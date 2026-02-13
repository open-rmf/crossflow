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

use variadics_please::all_tuples;

use std::{cell::RefCell, marker::PhantomData, sync::Arc};

use anyhow::Error as Anyhow;

pub use crate::dyn_node::*;
use crate::{Builder, Node, StreamAvailability, StreamEffect, StreamPack};

use super::*;

/// This manages how the common operations get registered, i.e. serialization,
/// deserialization, and cloning. By default all three of those operations are
/// registered for all message types since all plain data structures are compatible
/// with those operations.
///
/// This struct allows you to opt out of any of the common operations using:
/// - [`Self::no_serializing`]
/// - [`Self::no_deserializing`]
/// - [`Self::no_cloning`]
///
/// You cannot create this data structure directly. Instead use
/// [`DiagramElementRegistry::opt_out`] to obtain one.
#[must_use]
pub struct CommonOperations<'a, Deserialize, Serialize, Cloneable> {
    registry: &'a mut DiagramElementRegistry,
    _ignore: PhantomData<(Deserialize, Serialize, Cloneable)>,
}

impl<'a, DeserializeImpl, SerializeImpl, Cloneable>
    CommonOperations<'a, DeserializeImpl, SerializeImpl, Cloneable>
{
    /// Register a node builder with the specified common operations.
    ///
    /// This node builder always succeeds in building its node. If it is possible
    /// for your node builder to be unable to build its node, you should use
    /// [`Self::register_node_builder_fallible`] instead.
    ///
    /// # Arguments
    ///
    /// * `id` - Id of the builder, this must be unique.
    /// * `name` - Friendly name for the builder, this is only used for display purposes.
    /// * `f` - The node builder to register.
    pub fn register_node_builder<Config, Request, Response, Streams>(
        self,
        options: NodeBuilderOptions,
        mut f: impl FnMut(&mut Builder, Config) -> Node<Request, Response, Streams> + Send + 'static,
    ) -> NodeRegistrationBuilder<'a, Request, Response, Streams>
    where
        Config: JsonSchema + DeserializeOwned,
        Request: Send + Sync + 'static,
        Response: Send + Sync + 'static,
        Streams: StreamPack,
        DeserializeImpl: DeserializeMessage<Request>,
        DeserializeImpl: DeserializeMessage<Response>,
        SerializeImpl: SerializeMessage<Request>,
        SerializeImpl: SerializeMessage<Response>,
        Cloneable: RegisterClone<Request>,
        Cloneable: RegisterClone<Response>,
        JsonRegistration<SerializeImpl, DeserializeImpl>: RegisterJson<Request>,
        JsonRegistration<SerializeImpl, DeserializeImpl>: RegisterJson<Response>,
        Streams::StreamTypes: RegisterStreams<DeserializeImpl, SerializeImpl, Cloneable>,
    {
        self.register_node_builder_fallible(options, move |builder, config| Ok(f(builder, config)))
    }

    /// Register a node builder with the specified common operations.
    ///
    /// This node builder is able to fail while building. If it returns an [`Err`]
    /// instead of a node, then the entire diagram building procedure will be
    /// cancelled and an error will be provided to the user.
    ///
    /// If your node builder will always succeed, you can consider using
    /// [`Self::register_node_builder`] instead.
    ///
    /// # Arguments
    ///
    /// * `id` - Id of the builder, this must be unique.
    /// * `name` - Friendly name for the builder, this is only used for display purposes.
    /// * `f` - The node builder to register.
    pub fn register_node_builder_fallible<Config, Request, Response, Streams>(
        mut self,
        options: NodeBuilderOptions,
        mut f: impl FnMut(&mut Builder, Config) -> Result<Node<Request, Response, Streams>, Anyhow>
        + Send
        + 'static,
    ) -> NodeRegistrationBuilder<'a, Request, Response, Streams>
    where
        Config: JsonSchema + DeserializeOwned,
        Request: Send + Sync + 'static,
        Response: Send + Sync + 'static,
        Streams: StreamPack,
        DeserializeImpl: DeserializeMessage<Request>,
        DeserializeImpl: DeserializeMessage<Response>,
        SerializeImpl: SerializeMessage<Request>,
        SerializeImpl: SerializeMessage<Response>,
        Cloneable: RegisterClone<Request>,
        Cloneable: RegisterClone<Response>,
        JsonRegistration<SerializeImpl, DeserializeImpl>: RegisterJson<Request>,
        JsonRegistration<SerializeImpl, DeserializeImpl>: RegisterJson<Response>,
        Streams::StreamTypes: RegisterStreams<DeserializeImpl, SerializeImpl, Cloneable>,
    {
        let request = self.impl_register_message::<Request>();
        let response = self.impl_register_message::<Response>();
        Streams::StreamTypes::register_streams(&mut self);

        let node_builder_name = Arc::clone(&options.id);
        let mut availability = StreamAvailability::default();
        Streams::set_stream_availability(&mut availability);
        let streams = availability
            .named_streams()
            .into_iter()
            .map(|(k, v)|
                // SAFETY: We register all the streams earlier in this function
                (k, self.registry.messages.registration.get_index_dyn(&v).unwrap()))
            .collect();

        let registration = NodeRegistration {
            metadata: NodeMetadata {
                default_display_text: options.default_display_text.unwrap_or(options.id.clone()),
                request,
                response,
                streams,
                config_schema: self
                    .registry
                    .messages
                    .schema_generator
                    .subschema_for::<Config>(),
                description: options.description,
                config_examples: options.config_examples,
            },
            create_node_impl: RefCell::new(Box::new(move |builder, config| {
                let config = serde_json::from_value(config)
                    .map_err(|err| DiagramErrorCode::ConfigError(Arc::new(err)))?;
                let node =
                    f(builder, config).map_err(|error| DiagramErrorCode::NodeBuildingError {
                        builder: Arc::clone(&node_builder_name),
                        error: Arc::new(error),
                    })?;

                Ok(node.into())
            })),
        };
        self.registry.nodes.insert(options.id.clone(), registration);

        NodeRegistrationBuilder::<Request, Response, Streams>::new(self.registry)
    }

    /// Register a message with the specified common operations.
    pub fn register_message<Message>(mut self) -> MessageRegistrationBuilder<'a, Message>
    where
        Message: Send + Sync + 'static,
        DeserializeImpl: DeserializeMessage<Message>,
        SerializeImpl: SerializeMessage<Message>,
        Cloneable: RegisterClone<Message>,
        JsonRegistration<SerializeImpl, DeserializeImpl>: RegisterJson<Message>,
    {
        self.impl_register_message();
        MessageRegistrationBuilder::<Message>::new(&mut self.registry.messages)
    }

    fn impl_register_message<Message>(&mut self) -> usize
    where
        Message: Send + Sync + 'static,
        DeserializeImpl: DeserializeMessage<Message>,
        SerializeImpl: SerializeMessage<Message>,
        Cloneable: RegisterClone<Message>,
        JsonRegistration<SerializeImpl, DeserializeImpl>: RegisterJson<Message>,
    {
        let index = self
            .registry
            .messages
            .registration
            .get_index_or_insert::<Message>();

        self.registry
            .messages
            .register_deserialize::<Message, DeserializeImpl>();
        self.registry
            .messages
            .register_serialize::<Message, SerializeImpl>();
        self.registry
            .messages
            .register_clone::<Message, Cloneable>();

        register_json::<Message, SerializeImpl, DeserializeImpl>();
        index
    }

    /// Opt out of deserializing the input and output messages of the node.
    ///
    /// If you want to enable deserializing for only the input or only the output
    /// then use [`DiagramElementRegistry::register_message`] on the message type
    /// directly.
    ///
    /// Note that [`JsonBuffer`] is only enabled for message types that enable
    /// both serializing AND deserializing.
    pub fn no_deserializing(self) -> CommonOperations<'a, NotSupported, SerializeImpl, Cloneable> {
        CommonOperations {
            registry: self.registry,
            _ignore: Default::default(),
        }
    }

    /// Opt out of serializing the input and output messages of the node.
    ///
    /// If you want to enable serialization for only the input or only the output
    /// then use [`DiagramElementRegistry::register_message`] on the message type
    /// directly.
    ///
    /// Note that [`JsonBuffer`] is only enabled for message types that enable
    /// both serializing AND deserializing.
    pub fn no_serializing(self) -> CommonOperations<'a, DeserializeImpl, NotSupported, Cloneable> {
        CommonOperations {
            registry: self.registry,
            _ignore: Default::default(),
        }
    }

    /// Opt out of cloning the input and output messages of the node.
    ///
    /// If you want to enable cloning for only the input or only the output
    /// then use [`DiagramElementRegistry::register_message`] on the message type
    /// directly.
    pub fn no_cloning(self) -> CommonOperations<'a, DeserializeImpl, SerializeImpl, NotSupported> {
        CommonOperations {
            registry: self.registry,
            _ignore: Default::default(),
        }
    }

    /// Opt out of all the common operations.
    pub fn minimal(self) -> CommonOperations<'a, NotSupported, NotSupported, NotSupported> {
        CommonOperations {
            registry: self.registry,
            _ignore: Default::default(),
        }
    }

    pub(super) fn new(registry: &'a mut DiagramElementRegistry) -> Self {
        Self {
            registry,
            _ignore: Default::default(),
        }
    }
}

pub trait RegisterStreams<DeserializeImpl, SerializeImpl, Cloneable> {
    fn register_streams<'a>(
        registry: &mut CommonOperations<'a, DeserializeImpl, SerializeImpl, Cloneable>,
    );
}

impl<D, S, C> RegisterStreams<D, S, C> for () {
    fn register_streams(_: &mut CommonOperations<D, S, C>) {
        // Do nothing
    }
}

macro_rules! impl_register_streams_for_tuple {
    ($($T:ident),*) => {
        #[allow(non_snake_case)]
        impl<D, S, C, $($T),*> RegisterStreams<D, S, C> for ($($T,)*)
        where
            $($T: StreamEffect,)*
            D: $(DeserializeMessage<$T::Input> + DeserializeMessage<$T::Output> + )*,
            S: $(SerializeMessage<$T::Input> + SerializeMessage<$T::Output> + )*,
            C: $(RegisterClone<$T::Input> + RegisterClone<$T::Output> + )*,
            JsonRegistration<S, D>: $(RegisterJson<$T::Input> + RegisterJson<$T::Output> + )*,
        {
            fn register_streams(
                registry: &mut CommonOperations<D, S, C>,
            ) {
                $(
                    registry.impl_register_message::<$T::Input>();
                    registry.impl_register_message::<$T::Output>();
                )*
            }
        }
    }
}

all_tuples!(impl_register_streams_for_tuple, 1, 12, T);
