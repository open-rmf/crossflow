/*
 * Copyright (C) 2026 Open Source Robotics Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
*/

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DeriveInput, Fields, Type};

use crate::Result;

pub(crate) fn impl_fork_enum(input: &DeriveInput) -> Result<TokenStream> {
    let enum_ident = &input.ident;
    let enum_data = match &input.data {
        Data::Enum(data) => data,
        _ => return Err("Fork derive can only be used with enums".to_string()),
    };

    if enum_data.variants.is_empty() {
        return Err("Fork derive requires an enum with at least one variant".to_string());
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    // One output slot per enum variant.
    let unzipped_outputs = make_unzipped_outputs(enum_data);
    // Create concrete output targets during workflow construction.
    let output_creation = make_output_creation(enum_data);
    // Route the active variant and dispose inactive branches at runtime.
    let match_arms = make_match_arms(enum_data);

    Ok(quote! {
        impl #impl_generics ::crossflow::Unzippable for #enum_ident #ty_generics #where_clause {
            type Unzipped = (#(#unzipped_outputs,)*);

            fn unzip_output(
                output: ::crossflow::Output<Self>,
                builder: &mut ::crossflow::Builder,
            ) -> Self::Unzipped {
                assert_eq!(output.scope(), builder.scope());
                let mut targets = ::crossflow::re_exports::SmallVec::new();
                let result = (
                    #(#output_creation,)*
                );

                builder.commands().queue(::crossflow::AddOperation::new(
                    Some(output.scope()),
                    output.id(),
                    ::crossflow::ForkUnzip::<Self>::new(::crossflow::ForkTargetStorage(targets)),
                ));

                result
            }

            fn distribute_values(
                ::crossflow::OperationRequest {
                    source,
                    world,
                    roster,
                }: ::crossflow::OperationRequest,
            ) -> ::crossflow::OperationResult {
                use ::crossflow::{ManageDisposal, ManageInput, OrBroken};

                let ::crossflow::Input {
                    session,
                    data: input,
                    seq,
                } = world.take_input::<Self>(source)?;

                let targets = world.get::<::crossflow::ForkTargetStorage>(source).or_broken()?;
                let request = ::crossflow::RequestId {
                    session,
                    source,
                    seq,
                };

                match input {
                    #(#match_arms,)*
                }

                Ok(())
            }
        }
    })
}

fn make_unzipped_outputs(enum_data: &DataEnum) -> Vec<TokenStream> {
    enum_data
        .variants
        .iter()
        .map(|variant| {
            let variant_type = variant_output_type(&variant.fields);
            quote! { ::crossflow::Output<#variant_type> }
        })
        .collect()
}

fn make_output_creation(enum_data: &DataEnum) -> Vec<TokenStream> {
    enum_data
        .variants
        .iter()
        .map(|variant| {
            let output_ty = variant_output_type(&variant.fields);
            quote! {
                {
                    let target = builder.commands().spawn(::crossflow::UnusedTarget).id();
                    targets.push(target);
                    ::crossflow::Output::<#output_ty>::new(builder.scope(), target)
                }
            }
        })
        .collect()
}

fn make_match_arms(enum_data: &DataEnum) -> Vec<TokenStream> {
    enum_data
        .variants
        .iter()
        .enumerate()
        .map(|(active_index, variant)| {
            let variant_ident = &variant.ident;
            let (pattern, active_value) = variant_pattern_and_value(&variant.fields, variant_ident);
            let active_port = active_index;
            let active_target = format_ident!("__crossflow_target_{}", active_index);

            let bind_targets: Vec<TokenStream> = (0..enum_data.variants.len())
                .map(|index| {
                    let target_ident = format_ident!("__crossflow_target_{}", index);
                    quote! {
                        let #target_ident = *targets.0.get(#index).or_broken()?;
                    }
                })
                .collect();

            let emit_disposals: Vec<TokenStream> = (0..enum_data.variants.len())
                .filter(|index| *index != active_index)
                .map(|index| {
                    let target_ident = format_ident!("__crossflow_target_{}", index);
                    quote! {
                        {
                            let port = ::crossflow::output_port::next_index(#index);
                            let route = request.to_route_source(&port);
                            let disposal = ::crossflow::Disposal::branching(
                                source,
                                #target_ident,
                                None,
                            );
                            world.emit_disposal(route, disposal, roster);
                        }
                    }
                })
                .collect();

            quote! {
                Self::#variant_ident #pattern => {
                    #(#bind_targets)*

                    {
                        let port = ::crossflow::output_port::next_index(#active_port);
                        let route = request.to_message_route(&port, #active_target);
                        world.give_input(route, #active_value, roster)?;
                    }

                    #(#emit_disposals)*
                }
            }
        })
        .collect()
}

fn variant_output_type(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Unit => quote! { () },
        Fields::Unnamed(fields) => {
            let types: Vec<&Type> = fields.unnamed.iter().map(|field| &field.ty).collect();
            if types.len() == 1 {
                // Keep single-field variants ergonomic: Output<T> instead of Output<(T,)>.
                let ty = types[0];
                quote! { #ty }
            } else {
                quote! { (#(#types,)*) }
            }
        }
        Fields::Named(fields) => {
            let types: Vec<&Type> = fields.named.iter().map(|field| &field.ty).collect();
            if types.len() == 1 {
                // Keep single-field named variants ergonomic as well.
                let ty = types[0];
                quote! { #ty }
            } else {
                quote! { (#(#types,)*) }
            }
        }
    }
}

fn variant_pattern_and_value(fields: &Fields, variant_ident: &syn::Ident) -> (TokenStream, TokenStream) {
    match fields {
        Fields::Unit => (quote! {}, quote! { () }),
        Fields::Unnamed(fields) => {
            let variant_name = variant_ident.to_string().to_lowercase();
            let bindings: Vec<_> = (0..fields.unnamed.len())
                .map(|index| format_ident!("__crossflow_{}_{}", variant_name, index))
                .collect();

            if bindings.len() == 1 {
                let value = &bindings[0];
                (quote! { (#value) }, quote! { #value })
            } else {
                (quote! { (#(#bindings,)*) }, quote! { (#(#bindings,)*) })
            }
        }
        Fields::Named(fields) => {
            let names: Vec<_> = fields
                .named
                .iter()
                .map(|field| field.ident.clone().expect("named field must have identifier"))
                .collect();

            if names.len() == 1 {
                let value = &names[0];
                (quote! { { #value } }, quote! { #value })
            } else {
                (quote! { { #(#names,)* } }, quote! { (#(#names,)*) })
            }
        }
    }
}
