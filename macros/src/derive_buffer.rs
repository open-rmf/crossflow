use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Field, GenericParam, Generics, Ident, ImplGenerics, ItemStruct, Lifetime, LifetimeParam, Type,
    TypeGenerics, TypePath, Visibility, WhereClause, parse_quote, spanned::Spanned,
};

use crate::Result;

const JOINED_ATTR_TAG: &'static str = "joined";
const ACCESSOR_ATTR_TAG: &'static str = "accessor";

pub(crate) fn impl_joined_value(input_struct: &ItemStruct) -> Result<TokenStream> {
    let struct_ident = &input_struct.ident;
    let (impl_generics, ty_generics, where_clause) = input_struct.generics.split_for_impl();
    let CustomStructConfig {
        buffer_struct_ident,
        ..
    } = CustomStructConfig::for_joined(&input_struct);
    let buffer_struct_vis = &input_struct.vis;

    let (field_ident, _, field_config) =
        get_fields_map(&input_struct.fields, FieldSettings::for_joined())?;
    let buffer: Vec<&Type> = field_config.iter().map(|config| &config.buffer).collect();
    let noncopy = field_config.iter().any(|config| config.noncopy);

    let buffer_struct: ItemStruct = generate_buffer_struct(
        &buffer_struct_ident,
        buffer_struct_vis,
        &impl_generics,
        &where_clause,
        &field_ident,
        &buffer,
    );

    let impl_buffer_clone = impl_buffer_clone(
        &buffer_struct_ident,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &field_ident,
        noncopy,
    );

    let impl_select_buffers = impl_select_buffers(
        struct_ident,
        &buffer_struct_ident,
        buffer_struct_vis,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &field_ident,
        &buffer,
    );

    let impl_buffer_map_layout =
        impl_buffer_map_layout(&buffer_struct, &field_ident, &field_config)?;
    let impl_joining = impl_joining(
        &buffer_struct,
        &input_struct.ident,
        &input_struct.generics,
        &field_ident,
    )?;
    let impl_joined = impl_joined(
        &buffer_struct_ident,
        &input_struct.ident,
        &input_struct.generics,
    )?;

    let tokens = quote! {
        #impl_joined

        #buffer_struct

        #impl_buffer_clone

        #impl_select_buffers

        #impl_buffer_map_layout

        #impl_joining
    };

    Ok(tokens.into())
}

pub(crate) fn impl_buffer_accessor(input_struct: &ItemStruct) -> Result<TokenStream> {
    let struct_ident = &input_struct.ident;
    let (impl_generics, ty_generics, where_clause) = input_struct.generics.split_for_impl();
    let CustomStructConfig {
        buffer_struct_ident,
        joined_struct_ident,
        use_as_joined,
        view_struct_name,
        access_struct_name,
    } = CustomStructConfig::for_accessor(&input_struct);
    let buffer_struct_vis = &input_struct.vis;

    let (field_ident, field_type, field_config) =
        get_fields_map(&input_struct.fields, FieldSettings::for_key())?;
    let buffer: Vec<&Type> = field_config.iter().map(|config| &config.buffer).collect();
    let noncopy = field_config.iter().any(|config| config.noncopy);

    let buffer_struct: ItemStruct = generate_buffer_struct(
        &buffer_struct_ident,
        buffer_struct_vis,
        &impl_generics,
        &where_clause,
        &field_ident,
        &buffer,
    );

    let impl_buffer_clone = impl_buffer_clone(
        &buffer_struct_ident,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &field_ident,
        noncopy,
    );

    let impl_select_buffers = impl_select_buffers(
        struct_ident,
        &buffer_struct_ident,
        buffer_struct_vis,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &field_ident,
        &buffer,
    );

    let impl_buffer_map_layout =
        impl_buffer_map_layout(&buffer_struct, &field_ident, &field_config)?;
    let impl_accessed = impl_accessing(&buffer_struct, &input_struct, &field_ident, &field_type)?;

    let mut view_generics = input_struct.generics.clone();
    let view_lifetime = Lifetime::new("'v", Span::call_site());
    let view_ltp = LifetimeParam::new(view_lifetime);
    view_generics.params.push(GenericParam::from(view_ltp));
    let (impl_generics_view, ty_generics_view, _) = view_generics.split_for_impl();

    let mut access_generics = input_struct.generics.clone();
    access_generics.params.extend([
        GenericParam::from(LifetimeParam::new(Lifetime::new("'w", Span::call_site()))),
        GenericParam::from(LifetimeParam::new(Lifetime::new("'s", Span::call_site()))),
        GenericParam::from(LifetimeParam::new(Lifetime::new("'a", Span::call_site()))),
    ]);

    let (impl_generics_access, ty_generics_access, where_clause_access) =
        access_generics.split_for_impl();

    let mut fn_access_generics = input_struct.generics.clone();
    fn_access_generics.params.extend([
        GenericParam::from(LifetimeParam::new(Lifetime::new("'_", Span::call_site()))),
        GenericParam::from(LifetimeParam::new(Lifetime::new("'_", Span::call_site()))),
        GenericParam::from(LifetimeParam::new(Lifetime::new("'_", Span::call_site()))),
    ]);
    let (_, ty_generics_fn_access, _) = fn_access_generics.split_for_impl();

    let buffer_state: Vec<_> = field_ident
        .iter()
        .map(|id| format_ident!("state_{id}"))
        .collect();
    let buffer_param: Vec<_> = field_ident
        .iter()
        .map(|id| format_ident!("access_{id}"))
        .collect();

    let joined_struct = if use_as_joined {
        quote! {}
    } else {
        quote! {
            #[allow(non_camel_case_types, unused)]
            #buffer_struct_vis struct #joined_struct_ident #impl_generics #where_clause {
                #(
                    #buffer_struct_vis #field_ident: <#field_type as ::crossflow::Accessor>::Joined,
                )*
            }
        }
    };

    let joining_impl = impl_joining(
        &buffer_struct,
        &joined_struct_ident,
        &input_struct.generics,
        &field_ident,
    )?;
    let joined_impl = impl_joined(
        &buffer_struct.ident,
        &joined_struct_ident,
        &input_struct.generics,
    )?;

    let wait_for_change_impl = if field_ident.len() == 0 {
        // Do nothing at all if there are no fields in the struct
        quote! {}
    } else if field_ident.len() <= 12 {
        // Use the tuple implementation of Race if there are few enough fields
        quote! {
            use ::crossflow::re_exports::Race;
            let futures = (#(
                <#field_type as ::crossflow::Accessor>::wait_for_change(&mut self. #field_ident),
            )*);

            Race::race(futures).await;
        }
    } else {
        // Use the vec implementation of Race if there are too many fields
        quote! {
            use ::crossflow::re_exports::Race;
            let mut futures = Vec::new();
            #(
                futures.push(<#field_type as ::crossflow::Accessor>::wait_for_change(&mut self. #field_ident));
            )*

            Race::race(futures).await;
        }
    };

    let tokens = quote! {
        impl #impl_generics ::crossflow::Accessor for #struct_ident #ty_generics #where_clause {
            type Buffers = #buffer_struct_ident #ty_generics;

            async fn wait_for_change(&mut self) {
                #wait_for_change_impl
            }

            type Seen = (
                #(
                    <#field_type as ::crossflow::Accessor>::Seen,
                )*
            );

            fn seen(&mut self, seen: Self::Seen) {
                let (
                    #(
                        #field_ident,
                    )*
                ) = seen;

                #(
                    <#field_type as ::crossflow::Accessor>::seen(&mut self. #field_ident, #field_ident);
                )*
            }

            fn make_seen(&self, world: &mut ::crossflow::re_exports::World) -> Self::Seen {
                #(
                    let #field_ident = <#field_type as ::crossflow::Accessor>::make_seen(&self. #field_ident, world);
                )*

                (
                    #(
                        #field_ident,
                    )*
                )
            }

            fn is_disjoint(&self) -> ::std::result::Result<(), ::crossflow::OverlapError> {
                let mut duplicates = ::std::collections::HashMap::new();
                let mut is_disjoint = true;

                #(
                    is_disjoint &= <#field_type as ::crossflow::AccessKey>::validate_disjoint(&self. #field_ident, &mut duplicates);
                )*

                if !is_disjoint {
                    duplicates.retain(|_, count| *count > 1);
                    return ::std::result::Result::Err(::crossflow::OverlapError { duplicates });
                }

                return ::std::result::Result::Ok(())
            }

            fn can_join(&self, world: &::crossflow::re_exports::World) -> Result<bool, ::crossflow::AccessError>{
                ::crossflow::Accessor::is_disjoint(self)?;

                #(
                    if !<#field_type as ::crossflow::Accessor>::can_join(&self. #field_ident, world)? {
                        return std::result::Result::Ok(false);
                    }
                )*

                ::std::result::Result::Ok(true)
            }

            type Joined = #joined_struct_ident #ty_generics;
            fn join(
                &self,
                req: ::crossflow::RequestId,
                world: &mut ::crossflow::re_exports::World,
            ) -> ::std::result::Result<::std::option::Option<Self::Joined>, ::crossflow::AccessError> {
                if !::crossflow::Accessor::can_join(self, world)? {
                    return ::std::result::Result::Ok(::std::option::Option::None);
                }

                // First fetch each value and collect any possible errors
                let mut errors = ::std::vec::Vec::new();
                #(
                    let #field_ident = match <#field_type as ::crossflow::Accessor>::join(&self. #field_ident, req, world) {
                        Ok(value) => value,
                        Err(err) => {
                            errors.push(err);
                            None
                        }
                    };
                )*

                // Check if any access errors occurred.
                ::crossflow::AccessError::from_list(errors)?;

                // Peel away the Option on the field values
                #(
                    let Some(#field_ident) = #field_ident else {
                        // Note: This can't happen unless there's a flaw in the
                        // implementation of can_join
                        return ::std::result::Result::Ok(::std::option::Option::None);
                    };
                )*

                ::std::result::Result::Ok(
                    ::std::option::Option::Some(
                        #joined_struct_ident {
                            #(
                                #field_ident,
                            )*
                        }
                    )
                )
            }

            fn distribute(
                &self,
                value: Self::Joined,
                req: ::crossflow::RequestId,
                world: &mut ::crossflow::re_exports::World,
            ) -> ::std::result::Result<(), ::crossflow::AccessError> {
                let Self::Joined {
                    #(
                        #field_ident,
                    )*
                } = value;

                let mut errors = ::std::vec::Vec::new();
                #(
                    if let ::std::result::Result::Err(err) = self. #field_ident .distribute(#field_ident, req, world) {
                        errors.push(err);
                    }
                )*

                ::crossflow::AccessError::from_list(errors)
            }

            type View<'v> = #view_struct_name #ty_generics_view;
            fn view<'v>(
                &self,
                req: ::crossflow::RequestId,
                world: &'v mut ::crossflow::re_exports::World,
            ) -> ::std::result::Result<Self::View<'v>, ::crossflow::BufferError> {
                let world_cell = world.as_unsafe_world_cell();
                #(
                    let #field_ident = ::crossflow::Accessor::view(
                        &self. #field_ident,
                        req,
                        unsafe {
                            // SAFETY: We require &mut World as input to this
                            // function, so we know that nothing else is
                            // interacting with the world right now. We only
                            // need mutability for the tracing to be performed,
                            // which doesn't affect any borrows that we're
                            // capturing. After that all access is read-only.
                            world_cell.world_mut()
                        }
                    )?;
                )*

                Ok(#view_struct_name {
                    #(
                        #field_ident,
                    )*
                })
            }

            fn view_untraced<'v>(&self, world: &'v ::crossflow::re_exports::World) -> ::std::result::Result<Self::View<'v>, ::crossflow::BufferError> {
                #(
                    let #field_ident = ::crossflow::Accessor::view_untraced(&self. #field_ident, world)?;
                )*

                Ok(#view_struct_name {
                    #(
                        #field_ident,
                    )*
                })
            }

            type Access<'w, 's, 'a> = #access_struct_name #ty_generics_access #where_clause_access;
            fn access<U>(
                &self,
                req: ::crossflow::RequestId,
                world: &mut ::crossflow::re_exports::World,
                f: impl FnOnce(#access_struct_name #ty_generics_fn_access) -> U,
            ) -> ::std::result::Result<U, ::crossflow::AccessError> {
                self.is_disjoint()?;

                let world_cell = world.as_unsafe_world_cell();
                #(
                    let mut #buffer_state = ::crossflow::AccessKey::get_state(
                        &self. #field_ident,
                        unsafe {
                            // SAFETY: We make sure the accessor is disjoint at
                            // the start of the function. After that there is no
                            // overlap in the mutable world access needed by the
                            // system states. Their commands will be flushed
                            // serially at the end of this function.
                            world_cell.world_mut()
                        }
                    );
                )*

                let r = {
                    #(
                        let mut #buffer_param = <#field_type as ::crossflow::AccessKey>::get_param(
                            &mut #buffer_state,
                            unsafe {
                                // SAFETY: Same rationale as earlier in this function
                                world_cell.world_mut()
                            }
                        );
                    )*

                    #(
                        let #field_ident = <#field_type as ::crossflow::AccessKey>::get_mut(
                            &self. #field_ident,
                            req,
                            &mut #buffer_param,
                        )?;
                    )*

                    let access = #access_struct_name {
                        #(
                            #field_ident,
                        )*
                    };

                    f(access)
                };

                #(
                    <#field_type as ::crossflow::AccessKey>::apply_state(
                        &mut #buffer_state,
                        unsafe {
                            // SAFETY: Same rationale as earlier in this function
                            world_cell.world_mut()
                        }
                    );
                )*

                ::std::result::Result::Ok(r)
            }
        }

        #joined_struct

        #joined_impl

        #joining_impl

        #[allow(non_camel_case_types, unused)]
        #buffer_struct_vis struct #view_struct_name #impl_generics_view #where_clause {
            #(
                #buffer_struct_vis #field_ident: <#field_type as ::crossflow::Accessor>::View<'v>,
            )*
        }

        impl #impl_generics_view ::std::clone::Clone for #view_struct_name #ty_generics_view #where_clause {
            fn clone(&self) -> Self {
                Self {
                    #(
                        #field_ident: ::std::clone::Clone::clone(&self.#field_ident),
                    )*
                }
            }
        }

        #[allow(non_camel_case_types, unused)]
        #buffer_struct_vis struct #access_struct_name #impl_generics_access #where_clause_access {
            #(
                #buffer_struct_vis #field_ident: <#field_type as ::crossflow::Accessor>::Access<'w, 's, 'a>,
            )*
        }

        #buffer_struct

        #impl_buffer_clone

        #impl_select_buffers

        #impl_buffer_map_layout

        #impl_accessed
    };

    Ok(tokens.into())
}

/// Code that are currently unused but could be used in the future, move them out of this mod if
/// they are ever used.
#[allow(unused)]
mod _unused {
    use super::*;

    /// Converts a list of generics to a [`PhantomData`] TypePath.
    /// e.g. `::std::marker::PhantomData<fn(T,)>`
    fn to_phantom_data(generics: &Generics) -> TypePath {
        let lifetimes: Vec<Type> = generics
            .lifetimes()
            .map(|lt| {
                let lt = &lt.lifetime;
                let ty: Type = parse_quote! { & #lt () };
                ty
            })
            .collect();
        let ty_params: Vec<&Ident> = generics.type_params().map(|ty| &ty.ident).collect();
        parse_quote! { ::std::marker::PhantomData<fn(#(#lifetimes,)* #(#ty_params,)*)> }
    }
}

struct CustomStructConfig {
    buffer_struct_ident: Ident,
    joined_struct_ident: Ident,
    use_as_joined: bool,
    view_struct_name: Ident,
    access_struct_name: Ident,
}

struct CustomStructAttrs {
    joined_struct_name: bool,
    view_struct_name: bool,
    access_struct_name: bool,
}

const BUFFERS_STRUCT_NAME: &'static str = "buffers_struct_name";
const JOINED_STRUCT_NAME: &'static str = "joined_struct_name";
const VIEW_STRUCT_NAME: &'static str = "view_struct_name";
const ACCESS_STRUCT_NAME: &'static str = "access_struct_name";
const USE_AS_JOINED: &'static str = "use_as_joined";

impl std::fmt::Display for CustomStructAttrs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{BUFFERS_STRUCT_NAME}")?;
        if self.joined_struct_name {
            write!(f, ", {JOINED_STRUCT_NAME}, {USE_AS_JOINED}")?;
        }
        Ok(())
    }
}

impl CustomStructConfig {
    fn from_data_struct(
        data_struct: &ItemStruct,
        attr_tag: &str,
        attrs: CustomStructAttrs,
    ) -> Self {
        let mut config = CustomStructConfig {
            buffer_struct_ident: format_ident!("__crossflow_{}_Buffers", data_struct.ident),
            joined_struct_ident: format_ident!("__crossflow_{}_Joined", data_struct.ident),
            use_as_joined: false,
            view_struct_name: format_ident!("__crossflow_{}_View", data_struct.ident),
            access_struct_name: format_ident!("__crossflow_{}_Access", data_struct.ident),
        };

        for attr in data_struct
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident(attr_tag))
        {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(BUFFERS_STRUCT_NAME) {
                    config.buffer_struct_ident = meta.value()?.parse()?;
                } else if attrs.joined_struct_name && meta.path.is_ident(JOINED_STRUCT_NAME) {
                    config.joined_struct_ident = meta.value()?.parse()?;
                } else if attrs.joined_struct_name && meta.path.is_ident(USE_AS_JOINED) {
                    config.joined_struct_ident = meta.value()?.parse()?;
                    config.use_as_joined = true;
                } else if attrs.view_struct_name && meta.path.is_ident(VIEW_STRUCT_NAME) {
                    config.view_struct_name = meta.value()?.parse()?;
                } else if attrs.access_struct_name && meta.path.is_ident(ACCESS_STRUCT_NAME) {
                    config.access_struct_name = meta.value()?.parse()?;
                } else {
                    return Err(syn::Error::new(
                        meta.path.span(),
                        format!(
                            "Unrecognized attribute. This derive only supports: {}",
                            attrs,
                        ),
                    ));
                }

                Ok(())
            })
            // panic if attribute is malformed, this will result in a compile error which is intended.
            .unwrap();
        }

        config
    }

    fn for_joined(data_struct: &ItemStruct) -> Self {
        Self::from_data_struct(
            data_struct,
            &JOINED_ATTR_TAG,
            CustomStructAttrs {
                joined_struct_name: false,
                view_struct_name: false,
                access_struct_name: false,
            },
        )
    }

    fn for_accessor(data_struct: &ItemStruct) -> Self {
        Self::from_data_struct(
            data_struct,
            &ACCESSOR_ATTR_TAG,
            CustomStructAttrs {
                joined_struct_name: true,
                view_struct_name: true,
                access_struct_name: true,
            },
        )
    }
}

struct FieldSettings {
    default_buffer: fn(&Type) -> Type,
    attr_tag: &'static str,
}

impl FieldSettings {
    fn for_joined() -> Self {
        Self {
            default_buffer: Self::default_field_for_joined,
            attr_tag: JOINED_ATTR_TAG,
        }
    }

    fn for_key() -> Self {
        Self {
            default_buffer: Self::default_field_for_key,
            attr_tag: ACCESSOR_ATTR_TAG,
        }
    }

    fn default_field_for_joined(ty: &Type) -> Type {
        parse_quote! { ::crossflow::FetchFromBuffer<#ty> }
    }

    fn default_field_for_key(ty: &Type) -> Type {
        parse_quote! { <#ty as ::crossflow::BufferKeyLifecycle>::TargetBuffer }
    }
}

struct FieldConfig {
    buffer: Type,
    noncopy: bool,
}

impl FieldConfig {
    fn from_field(field: &Field, settings: &FieldSettings) -> Self {
        let ty = &field.ty;
        let mut config = Self {
            buffer: (settings.default_buffer)(ty),
            noncopy: false,
        };

        for attr in field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident(settings.attr_tag))
        {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("buffer") {
                    config.buffer = meta.value()?.parse()?;
                }
                if meta.path.is_ident("noncopy_buffer") {
                    config.noncopy = true;
                }
                Ok(())
            })
            // panic if attribute is malformed, this will result in a compile error which is intended.
            .unwrap();
        }

        config
    }
}

fn get_fields_map(
    fields: &syn::Fields,
    settings: FieldSettings,
) -> Result<(Vec<&Ident>, Vec<&Type>, Vec<FieldConfig>)> {
    match fields {
        syn::Fields::Named(data) => {
            let mut idents = Vec::new();
            let mut types = Vec::new();
            let mut configs = Vec::new();
            for field in &data.named {
                let ident = field
                    .ident
                    .as_ref()
                    .ok_or("expected named fields".to_string())?;
                idents.push(ident);
                types.push(&field.ty);
                configs.push(FieldConfig::from_field(field, &settings));
            }
            Ok((idents, types, configs))
        }
        _ => return Err("expected named fields".to_string()),
    }
}

fn generate_buffer_struct(
    buffer_struct_ident: &Ident,
    buffer_struct_vis: &Visibility,
    impl_generics: &ImplGenerics,
    where_clause: &Option<&WhereClause>,
    field_ident: &Vec<&Ident>,
    buffer: &Vec<&Type>,
) -> ItemStruct {
    parse_quote! {
        #[allow(non_camel_case_types, unused)]
        #buffer_struct_vis struct #buffer_struct_ident #impl_generics #where_clause {
            #(
                #buffer_struct_vis #field_ident: #buffer,
            )*
        }
    }
}

fn impl_select_buffers(
    struct_ident: &Ident,
    buffer_struct_ident: &Ident,
    buffer_struct_vis: &Visibility,
    impl_generics: &ImplGenerics,
    ty_generics: &TypeGenerics,
    where_clause: &Option<&WhereClause>,
    field_ident: &Vec<&Ident>,
    buffer: &Vec<&Type>,
) -> TokenStream {
    quote! {
        impl #impl_generics #struct_ident #ty_generics #where_clause {
            #buffer_struct_vis fn select_buffers(
                #(
                    #field_ident: impl Into< #buffer >,
                )*
            ) -> #buffer_struct_ident #ty_generics {
                #buffer_struct_ident {
                    #(
                        #field_ident: #field_ident .into(),
                    )*
                }
            }
        }
    }
    .into()
}

fn impl_buffer_clone(
    buffer_struct_ident: &Ident,
    impl_generics: &ImplGenerics,
    ty_generics: &TypeGenerics,
    where_clause: &Option<&WhereClause>,
    field_ident: &Vec<&Ident>,
    noncopy: bool,
) -> TokenStream {
    if noncopy {
        // Clone impl for structs with a buffer that is not copyable
        quote! {
            impl #impl_generics ::crossflow::re_exports::Clone for #buffer_struct_ident #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    Self {
                        #(
                            #field_ident: ::crossflow::re_exports::Clone::clone(&self.#field_ident),
                        )*
                    }
                }
            }
        }
    } else {
        // Clone and copy impl for structs with buffers that are all copyable
        quote! {
            impl #impl_generics ::crossflow::re_exports::Clone for #buffer_struct_ident #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    *self
                }
            }

            impl #impl_generics ::crossflow::re_exports::Copy for #buffer_struct_ident #ty_generics #where_clause {}
        }
    }
}

/// Params:
///   buffer_struct: The struct to implement `BufferMapLayout`.
///   item_struct: The struct which `buffer_struct` is derived from.
///   settings: [`FieldSettings`] to use when parsing the field attributes
fn impl_buffer_map_layout(
    buffer_struct: &ItemStruct,
    field_ident: &Vec<&Ident>,
    field_config: &Vec<FieldConfig>,
) -> Result<proc_macro2::TokenStream> {
    let struct_ident = &buffer_struct.ident;
    let (impl_generics, ty_generics, where_clause) = buffer_struct.generics.split_for_impl();
    let buffer: Vec<&Type> = field_config.iter().map(|config| &config.buffer).collect();
    let map_key: Vec<String> = field_ident.iter().map(|v| v.to_string()).collect();

    Ok(quote! {
        impl #impl_generics ::crossflow::BufferMapLayout for #struct_ident #ty_generics #where_clause {
            fn try_from_buffer_map(buffers: &::crossflow::BufferMap) -> Result<Self, ::crossflow::IncompatibleLayout> {
                let mut compatibility = ::crossflow::IncompatibleLayout::default();
                #(
                    let #field_ident = compatibility.require_buffer_for_identifier::<#buffer>(#map_key, buffers);
                )*

                // Unwrap the Ok after inspecting every field so that the
                // IncompatibleLayout error can include all information about
                // which fields were incompatible.
                #(
                    let Ok(#field_ident) = #field_ident else {
                        return Err(compatibility);
                    };
                )*

                Ok(Self {
                    #(
                        #field_ident,
                    )*
                })
            }

            fn get_buffer_message_type_hints(
                identifiers: ::std::collections::HashSet<::crossflow::IdentifierRef<'static>>,
            ) -> ::std::result::Result<::crossflow::MessageTypeHintMap, ::crossflow::IncompatibleLayout> {
                let mut evaluation = ::crossflow::MessageTypeHintEvaluation::new(identifiers);
                #(
                    evaluation.set_hint(#map_key, <#buffer as ::crossflow::AsAnyBuffer>::message_type_hint());
                )*

                evaluation.evaluate()
            }

            fn get_layout_hints() -> ::crossflow::BufferMapLayoutHints {
                let mut hints = ::crossflow::MessageTypeHintMap::new();
                #(
                    hints.insert(#map_key .into(), <#buffer as ::crossflow::AsAnyBuffer>::message_type_hint());
                )*

                ::crossflow::BufferMapLayoutHints::Static(hints)
            }
        }

        impl #impl_generics ::crossflow::BufferMapStruct for #struct_ident #ty_generics #where_clause {
            fn buffer_list(&self) -> ::crossflow::re_exports::SmallVec<[::crossflow::AnyBuffer; 8]> {
                ::crossflow::re_exports::smallvec![#(
                    ::crossflow::AsAnyBuffer::as_any_buffer(&self.#field_ident),
                )*]
            }
        }
    }
    .into())
}

fn impl_joined(
    buffers_struct_ident: &Ident,
    joined_struct_ident: &Ident,
    generics: &Generics,
) -> Result<proc_macro2::TokenStream> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(
        quote! {
            impl #impl_generics ::crossflow::Joined for #joined_struct_ident #ty_generics #where_clause {
                type Buffers = #buffers_struct_ident #ty_generics;
                fn from_item(item: <Self::Buffers as ::crossflow::Joining>::Item) -> Self {
                    item
                }
            }
        }
        .into()
    )
}

fn impl_joining(
    buffers_struct: &ItemStruct,
    joined_struct_ident: &Ident,
    generics: &Generics,
    field_ident: &Vec<&Ident>,
) -> Result<proc_macro2::TokenStream> {
    let struct_ident = &buffers_struct.ident;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::crossflow::Joining for #struct_ident #ty_generics #where_clause {
            type Item = #joined_struct_ident #ty_generics;

            fn fetch_for_join(
                &self,
                req: ::crossflow::RequestId,
                session: ::crossflow::re_exports::Entity,
                world: &mut ::crossflow::re_exports::World,
            ) -> ::std::result::Result<Self::Item, ::crossflow::OperationError> {
                #(
                    let #field_ident = self.#field_ident.fetch_for_join(req, session, world)?;
                )*

                Ok(Self::Item {#(
                    #field_ident,
                )*})
            }
        }
    }
    .into())
}

fn impl_accessing(
    accessed_struct: &ItemStruct,
    key_struct: &ItemStruct,
    field_ident: &Vec<&Ident>,
    field_type: &Vec<&Type>,
) -> Result<proc_macro2::TokenStream> {
    let struct_ident = &accessed_struct.ident;
    let key_struct_ident = &key_struct.ident;
    let (impl_generics, ty_generics, where_clause) = key_struct.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::crossflow::Accessing for #struct_ident #ty_generics #where_clause {
            type Key = #key_struct_ident #ty_generics;

            fn add_accessor(
                &self,
                accessor: ::crossflow::re_exports::Entity,
                world: &mut ::crossflow::re_exports::World,
            ) -> ::crossflow::OperationResult {
                #(
                    ::crossflow::Accessing::add_accessor(&self.#field_ident, accessor, world)?;
                )*
                Ok(())
            }

            fn create_key(&self, builder: &mut ::crossflow::BufferKeyBuilder) -> ::crossflow::OperationResult<Self::Key> {
                ::std::result::Result::Ok(
                    Self::Key {#(
                        // TODO(@mxgrey): This currently does not have good support for the user
                        // substituting in a different key type than what the BufferKeyLifecycle expects.
                        // We could consider adding a .clone().into() to help support that use case, but
                        // this would be such a niche use case that I think we can ignore it for now.
                        #field_ident: <#field_type as ::crossflow::BufferKeyLifecycle>::create_key(&self.#field_ident, builder)?,
                    )*}
                )
            }

            fn deep_clone_key(key: &Self::Key) -> Self::Key {
                Self::Key {#(
                    #field_ident: ::crossflow::BufferKeyLifecycle::deep_clone(&key.#field_ident),
                )*}
            }

            fn is_key_in_use(key: &Self::Key) -> bool {
                false
                    #(
                        || ::crossflow::BufferKeyLifecycle::is_in_use(&key.#field_ident)
                    )*
            }
        }
    }.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attrs() {
        let input_struct: ItemStruct = parse_quote! {
            #[derive(Clone, Accessor)]
            #[accessor(
                buffers_struct_name = TestKeysBuffers,
                joined_struct_name = TestKeysJoined,
            )]
            struct TestKeys<T: 'static + Send + Sync + Clone> {
                integer: BufferKey<i64>,
                float: BufferKey<f64>,
                string: BufferKey<String>,
                generic: BufferKey<T>,
                any: AnyBufferKey,
            }
        };

        let CustomStructConfig {
            buffer_struct_ident,
            joined_struct_ident,
            use_as_joined,
            view_struct_name,
            access_struct_name,
        } = CustomStructConfig::for_accessor(&input_struct);
        assert_eq!(buffer_struct_ident, "TestKeysBuffers");
        assert_eq!(joined_struct_ident, "TestKeysJoined");
        assert!(!use_as_joined);
        assert_eq!(view_struct_name, "__crossflow_TestKeys_View");
        assert_eq!(access_struct_name, "__crossflow_TestKeys_Access");
    }
}
