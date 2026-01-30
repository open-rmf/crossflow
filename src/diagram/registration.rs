/*
 * Copyright (C) 2025 Open Source Robotics Foundation
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

use std::{
    any::Any,
    borrow::{Borrow, Cow},
    cell::RefCell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    sync::Arc,
};

use bevy_ecs::prelude::{Commands, Entity};

use anyhow::Error as Anyhow;

pub use crate::dyn_node::*;
use crate::{
    Accessor, AnyBuffer, AsAnyBuffer, BufferIdentifier, BufferMap, BufferMapLayout, BufferSettings,
    Builder, DisplayText, IncompatibleLayout, IncrementalScopeBuilder, IncrementalScopeRequest,
    IncrementalScopeRequestResult, IncrementalScopeResponse, IncrementalScopeResponseResult,
    Joined, JsonBuffer, JsonMessage, MessageTypeHintMap, NamedStream, Node, StreamAvailability,
    StreamOf, StreamPack, StreamEffect, JoinRegistration, BufferMapLayoutHints,
};

#[cfg(feature = "trace")]
use crate::Trace;

use schemars::{JsonSchema, Schema, SchemaGenerator, generate::SchemaSettings, json_schema};
use serde::{Deserialize, Serialize, de::DeserializeOwned, ser::SerializeMap};

use super::{
    BuilderId, DeserializeMessage, DiagramErrorCode, DynForkClone, DynForkResult, DynSplit,
    DynType, JsonRegistration, RegisterJson, RegisterSplit, Section, SectionInterface,
    SectionInterfaceDescription, SerializeMessage, SplitSchema, TransformError, TypeInfo,
    buffer_schema::BufferAccessRequest, fork_clone_schema::RegisterClone,
    fork_result_schema::{RegisterForkResult, ForkResultRegistration}, register_json,
    supported::*,
    unzip_schema::{RegisterUnzip, UnzipRegistration},
};

pub struct NodeRegistration {
    metadata: NodeMetadata,

    /// Creates an instance of the registered node.
    create_node_impl: CreateNodeFn,
}

impl NodeRegistration {
    pub(super) fn create_node(
        &self,
        builder: &mut Builder,
        config: JsonMessage,
    ) -> Result<DynNode, DiagramErrorCode> {
        let mut create_node_impl = self.create_node_impl.borrow_mut();
        let n = create_node_impl(builder, config)?;
        Ok(n)
    }
}

type CreateNodeFn =
    RefCell<Box<dyn FnMut(&mut Builder, JsonMessage) -> Result<DynNode, DiagramErrorCode> + Send>>;
type DeserializeFn = fn(&mut Builder) -> Result<DynForkResult, DiagramErrorCode>;
type SerializeFn = fn(&mut Builder) -> Result<DynForkResult, DiagramErrorCode>;
type ForkCloneFn = fn(&mut Builder) -> Result<DynForkClone, DiagramErrorCode>;
type SplitFn = fn(&SplitSchema, &mut Builder) -> Result<DynSplit, DiagramErrorCode>;
type BufferAccessFn = fn(&BufferMap, &mut Builder) -> Result<DynNode, DiagramErrorCode>;
type ListenFn = fn(&BufferMap, &mut Builder) -> Result<DynOutput, DiagramErrorCode>;
type BufferLayoutTypeHintFn =
    fn(HashSet<BufferIdentifier<'static>>) -> Result<MessageTypeHintMap, IncompatibleLayout>;
type CreateBufferFn = fn(BufferSettings, &mut Builder) -> AnyBuffer;
type CreateTriggerFn = fn(&mut Builder) -> DynNode;
type CreateIntoFn = Arc<dyn Fn(&mut Builder) -> (DynInputSlot, DynOutput) + 'static + Send + Sync>;
type CreateTryIntoFn = Arc<dyn Fn(&mut Builder) -> DynForkResult + 'static + Send + Sync>;
type ToStringFn = fn(&mut Builder) -> DynNode;

#[cfg(feature = "trace")]
type EnableTraceSerializeFn = fn(&mut Trace);

struct BuildScope {
    set_request: fn(&mut IncrementalScopeBuilder, &mut Commands) -> IncrementalScopeRequestResult,
    set_response: fn(&mut IncrementalScopeBuilder, &mut Commands) -> IncrementalScopeResponseResult,
    spawn_basic_scope_stream: fn(Entity, Entity, &mut Commands) -> (DynInputSlot, DynOutput),
}

impl BuildScope {
    fn new<T: 'static + Send + Sync>() -> Self {
        Self {
            set_request: Self::impl_set_request::<T>,
            set_response: Self::impl_set_response::<T>,
            spawn_basic_scope_stream: Self::impl_spawn_basic_scope_stream::<T>,
        }
    }

    fn impl_set_request<T: 'static + Send + Sync>(
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> IncrementalScopeRequestResult {
        incremental.set_request::<T>(commands)
    }

    fn impl_set_response<T: 'static + Send + Sync>(
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> IncrementalScopeResponseResult {
        incremental.set_response::<T>(commands)
    }

    fn impl_spawn_basic_scope_stream<T: 'static + Send + Sync>(
        in_scope: Entity,
        out_scope: Entity,
        commands: &mut Commands,
    ) -> (DynInputSlot, DynOutput) {
        let (stream_in, stream_out) =
            NamedStream::<StreamOf<T>>::spawn_scope_stream(in_scope, out_scope, commands);

        (stream_in.into(), stream_out.into())
    }
}

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
                (k, self.registry.messages.registration.get_index_dyn(&v).unwrap())
            )
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
                let config =
                    serde_json::from_value(config).map_err(DiagramErrorCode::ConfigError)?;
                let node =
                    f(builder, config).map_err(|error| DiagramErrorCode::NodeBuildingError {
                        builder: Arc::clone(&node_builder_name),
                        error,
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
}

pub struct MessageRegistrationBuilder<'a, Message> {
    data: &'a mut MessageRegistry,
    _ignore: PhantomData<Message>,
}

impl<'a, Message> MessageRegistrationBuilder<'a, Message>
where
    Message: Send + Sync + 'static + Any,
{
    pub fn new(registry: &'a mut MessageRegistry) -> Self {
        // Any message type can be joined into a Vec
        registry.register_join::<Vec<Message>>();

        Self {
            data: registry,
            _ignore: Default::default(),
        }
    }

    /// Mark the message as having a unzippable response. This is required in order for the node
    /// to be able to be connected to a "Unzip" operation.
    pub fn with_unzip(&mut self) -> &mut Self
    where
        Supported<(Message, Supported, Supported)>: RegisterUnzip,
    {
        self.data.register_unzip::<Message, Supported, Supported>();
        self
    }

    /// Mark the message as having an unzippable response whose elements are not serializable.
    pub fn with_unzip_minimal(&mut self) -> &mut Self
    where
        Supported<(Message, NotSupported, NotSupported)>: RegisterUnzip,
    {
        self.data
            .register_unzip::<Message, NotSupported, NotSupported>();
        self
    }

    /// Mark the message as having a [`Result<_, _>`] response. This is required in order for the node
    /// to be able to be connected to a "Fork Result" operation.
    pub fn with_result(&mut self) -> &mut Self
    where
        Supported<(Message, Supported, Supported)>: RegisterForkResult,
    {
        self.data
            .register_result::<Supported<(Message, Supported, Supported)>>();
        self
    }

    /// Same as `Self::with_result` but it will not register serialization
    /// or cloning for the [`Ok`] or [`Err`] variants of the message.
    pub fn with_result_minimal(&mut self) -> &mut Self
    where
        Supported<(Message, NotSupported, NotSupported)>: RegisterForkResult,
    {
        self.data
            .register_result::<Supported<(Message, NotSupported, NotSupported)>>();
        self
    }

    /// Mark the message as having a splittable response. This is required in order
    /// for the node to be able to be connected to a "Split" operation.
    pub fn with_split(&mut self) -> &mut Self
    where
        Supported<(Message, Supported, Supported)>: RegisterSplit,
    {
        self.data.register_split::<Message, Supported, Supported>();
        self
    }

    /// Mark the message as having a splittable response but the items from the split
    /// are unserializable.
    pub fn with_split_minimal(&mut self) -> &mut Self
    where
        Supported<(Message, NotSupported, NotSupported)>: RegisterSplit,
    {
        self.data
            .register_split::<Message, NotSupported, NotSupported>();
        self
    }

    /// Mark the message as being joinable.
    pub fn with_join(&mut self) -> &mut Self
    where
        Message: Joined,
    {
        self.data.register_join::<Message>();
        self
    }

    /// Mark the message as being a buffer access.
    pub fn with_buffer_access(&mut self) -> &mut Self
    where
        Message: BufferAccessRequest,
    {
        self.data.register_buffer_access::<Message>();
        self
    }

    /// Mark the message as being listenable.
    pub fn with_listen(&mut self) -> &mut Self
    where
        Message: Accessor,
    {
        self.data.register_listen::<Message>();
        self
    }

    pub fn with_to_string(&mut self) -> &mut Self
    where
        Message: ToString,
    {
        self.data.register_to_string::<Message>();
        self
    }

    /// Register the [`Into`] implementation that maps this message into some
    /// other message type `U`.
    pub fn with_into<U>(&mut self) -> &mut Self
    where
        U: 'static + Send + Sync,
        Message: Into<U>,
    {
        self.with_mapping_into(Into::into)
    }

    /// Register a mapping from the message type into some other message type.
    /// This allows you to register a custom mapping.
    ///
    /// Registering this multiple times for the same message pair will override
    /// the previously registered mapping.
    pub fn with_mapping_into<U>(
        &mut self,
        f: impl Fn(Message) -> U + 'static + Send + Sync,
    ) -> &mut Self
    where
        U: 'static + Send + Sync,
    {
        let f = Arc::new(f);
        let mapping = move |builder: &mut Builder| -> (DynInputSlot, DynOutput) {
            let f = Arc::clone(&f);
            let node = builder.create_map_block(move |request| f(request));
            (node.input.into(), node.output.into())
        };

        let mapping = Arc::new(mapping) as CreateIntoFn;
        let u_index = self.data.registration.get_index_or_insert::<U>();
        self.data.registration.get_or_insert_operations::<Message>()
            .into_impls
            .insert(u_index, Arc::clone(&mapping));

        let message_index = self.data.registration.get_index_or_insert::<Message>();
        self.data.registration.get_or_insert_operations::<U>()
            .from_impls
            .insert(message_index, mapping);

        self
    }

    /// Register the [`From`] implementation that maps from some other message
    /// type `V` into this message type.
    pub fn with_from<V>(&mut self) -> &mut Self
    where
        V: 'static + Send + Sync + Into<Message>,
    {
        self.with_mapping_from::<V>(Into::into)
    }

    /// Register a mapping from some other message type `V` into this message.
    /// This allows you to register a custom mapping.
    ///
    /// Calling this multiple times for the same message pair will override any
    /// previously registered mapping for that pair.
    ///
    /// This is the same as `self.with_mapping_into<V>(f)` except `Message` and
    /// `V` are flipped.
    pub fn with_mapping_from<V>(
        &mut self,
        f: impl Fn(V) -> Message + 'static + Send + Sync,
    ) -> &mut Self
    where
        V: 'static + Send + Sync,
    {
        MessageRegistrationBuilder::<V>::new(self.data).with_mapping_into(f);
        self
    }

    /// Register the [`TryInto`] implementation that maps this message into some
    /// other message type `U`.
    pub fn with_try_into<U>(&mut self) -> &mut Self
    where
        U: 'static + Send + Sync,
        Message: TryInto<U>,
        Message::Error: 'static + Send + Sync + ToString,
    {
        self.with_mapping_try_into(TryInto::<U>::try_into)
    }

    /// Register a fallible mapping from the message type into some other
    /// message type. This allows you to register a custom mapping.
    ///
    /// Registering this multiple times for the same message pair will override
    /// the previously registered mapping.
    pub fn with_mapping_try_into<U, E>(
        &mut self,
        f: impl Fn(Message) -> Result<U, E> + 'static + Send + Sync,
    ) -> &mut Self
    where
        U: 'static + Send + Sync,
        E: 'static + Send + Sync + ToString,
    {
        let f = Arc::new(f);
        let mapping = move |builder: &mut Builder| -> DynForkResult {
            let f = Arc::clone(&f);
            let node = builder.create_map_block(move |request| {
                f(request)
                .map_err(|err| err.to_string())
            });
            let (fork_input, fork_result) = builder.create_fork_result();
            builder.connect(node.output, fork_input);

            DynForkResult {
                input: node.input.into(),
                ok: fork_result.ok.into(),
                err: fork_result.err.into(),
            }
        };

        let mapping = Arc::new(mapping) as CreateTryIntoFn;
        let u_index = self.data.registration.get_index_or_insert::<U>();
        self.data.registration.get_or_insert_operations::<Message>()
            .try_into_impls
            .insert(u_index, Arc::clone(&mapping));

        let message_index = self.data.registration.get_index_or_insert::<Message>();
        self.data.registration.get_or_insert_operations::<U>()
            .try_from_impls
            .insert(message_index, mapping);

        self
    }

    /// Register the [`TryFrom`] implementation that maps from some other
    /// message type `V` into this message type.
    pub fn with_try_from<V>(&mut self) -> &mut Self
    where
        V: 'static + Send + Sync + TryInto<Message>,
        <V as TryInto<Message>>::Error: 'static + Send + Sync + ToString,
    {
        self.with_mapping_try_from(V::try_into);
        self
    }

    /// Register a fallible mapping from some other message type `V` into this
    /// message. This allows you to register a custom mapping.
    ///
    /// Calling this multiple times for the same message pair will override any
    /// previous registered mapping for that pair.
    ///
    /// This is the same as `self.with_mapping_try_into<V>(f)` except `Message`
    /// and `V` are flipped.
    pub fn with_mapping_try_from<V, E>(
        &mut self,
        f: impl Fn(V) -> Result<Message, E> + 'static + Send + Sync,
    ) -> &mut Self
    where
        V: 'static + Send + Sync,
        E: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<V>::new(self.data).with_mapping_try_into(f);
        self
    }
}

pub struct NodeRegistrationBuilder<'a, Request, Response, Streams> {
    registry: &'a mut DiagramElementRegistry,
    _ignore: PhantomData<(Request, Response, Streams)>,
}

impl<'a, Request, Response, Streams> NodeRegistrationBuilder<'a, Request, Response, Streams>
where
    Request: Send + Sync + 'static + Any,
    Response: Send + Sync + 'static + Any,
{
    fn new(registry: &'a mut DiagramElementRegistry) -> Self {
        Self {
            registry,
            _ignore: Default::default(),
        }
    }

    /// If you opted out of any common operations in order to accommodate your
    /// response type, you can enable all common operations for your response
    /// type using this.
    pub fn with_common_request(&mut self) -> &mut Self
    where
        Request: DynType + DeserializeOwned + Serialize + Clone,
    {
        self.registry.register_message::<Request>();
        self
    }

    /// If you opted out of cloning, you can enable it specifically for the
    /// input message with this.
    pub fn with_clone_request(&mut self) -> &mut Self
    where
        Request: Clone,
    {
        self.registry
            .messages
            .register_clone::<Request, Supported>();
        self
    }

    /// If you opted out of deserialization, you can enable it specifically for
    /// the input message with this.
    pub fn with_deserialize_request(&mut self) -> &mut Self
    where
        Request: DeserializeOwned + DynType,
    {
        self.registry
            .messages
            .register_deserialize::<Request, Supported>();
        self
    }

    /// If you opted out of any common operations in order to accommodate your
    /// request type, you can enable all common operations for your response
    /// type using this.
    pub fn with_common_response(&mut self) -> &mut Self
    where
        Response: DynType + DeserializeOwned + Serialize + Clone,
    {
        self.registry.register_message::<Response>();
        self
    }

    /// If you opted out of cloning, you can enable it specifically for the
    /// output message with this.
    pub fn with_clone_response(&mut self) -> &mut Self
    where
        Response: Clone,
    {
        self.registry
            .messages
            .register_clone::<Response, Supported>();
        self
    }

    /// If you opted out of serialization, you can enable it specifically for
    /// the output message with this.
    pub fn with_serialize_response(&mut self) -> &mut Self
    where
        Response: Serialize + DynType,
    {
        self.registry
            .messages
            .register_serialize::<Response, Supported>();
        self
    }

    /// Mark the node as having a unzippable response. This is required in order for the node
    /// to be able to be connected to a "Unzip" operation.
    pub fn with_unzip(&mut self) -> &mut Self
    where
        Supported<(Response, Supported, Supported)>: RegisterUnzip,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_unzip();
        self
    }

    /// Mark the node as having an unzippable response whose elements are not serializable.
    pub fn with_unzip_unserializable(&mut self) -> &mut Self
    where
        Supported<(Response, NotSupported, NotSupported)>: RegisterUnzip,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_unzip_minimal();
        self
    }

    /// Mark the node as having a [`Result<_, _>`] response. This is required in order for the node
    /// to be able to be connected to a "Fork Result" operation.
    pub fn with_result(&mut self) -> &mut Self
    where
        Supported<(Response, Supported, Supported)>: RegisterForkResult,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_result();
        self
    }

    /// Same as `Self::with_result` but it will not register serialization
    /// or cloning for the [`Ok`] or [`Err`] variants of the message.
    pub fn with_result_minimal(&mut self) -> &mut Self
    where
        Supported<(Response, NotSupported, NotSupported)>: RegisterForkResult,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_result_minimal();
        self
    }

    /// Mark the node as having a splittable response. This is required in order
    /// for the node to be able to be connected to a "Split" operation.
    pub fn with_split(&mut self) -> &mut Self
    where
        Supported<(Response, Supported, Supported)>: RegisterSplit,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_split();
        self
    }

    /// Mark the node as having a splittable response but the items from the split
    /// are unserializable.
    pub fn with_split_unserializable(&mut self) -> &mut Self
    where
        Supported<(Response, NotSupported, NotSupported)>: RegisterSplit,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_split_minimal();
        self
    }

    /// Mark the node as having a joinable request.
    pub fn with_join(&mut self) -> &mut Self
    where
        Request: Joined,
    {
        self.registry.messages.register_join::<Request>();
        self
    }

    /// Mark the node as having a buffer access request.
    pub fn with_buffer_access(&mut self) -> &mut Self
    where
        Request: BufferAccessRequest,
    {
        self.registry.messages.register_buffer_access::<Request>();
        self
    }

    /// Mark the node as having a listen request.
    pub fn with_listen(&mut self) -> &mut Self
    where
        Request: Accessor,
    {
        self.registry.messages.register_listen::<Request>();
        self
    }

    pub fn with_request_to_string(&mut self) -> &mut Self
    where
        Request: ToString,
    {
        self.registry.messages.register_to_string::<Request>();
        self
    }

    pub fn with_response_to_string(&mut self) -> &mut Self
    where
        Response: ToString,
    {
        self.registry.messages.register_to_string::<Response>();
        self
    }

    /// Register the [`Into`] implementation that maps the request type of this
    /// node into some other message type `U`.
    pub fn with_into<U>(&mut self) -> &mut Self
    where
        U: 'static + Send + Sync,
        Response: Into<U>,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_into::<U>();
        self
    }

    /// Register a mapping from the request type of this node into some other
    /// message type. This allows you to register a custom mapping.
    ///
    /// Registering this multiple times for the same message pair will override
    /// the previously registered mapping.
    pub fn with_mapping_into<U>(
        &mut self,
        f: impl Fn(Response) -> U + 'static + Send + Sync,
    ) -> &mut Self
    where
        U: 'static + Send + Sync,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_mapping_into(f);
        self
    }

    /// Register the [`From`] implementation that maps from some other message
    /// type `V` into the request type of this node.
    pub fn with_from<V>(&mut self) -> &mut Self
    where
        V: 'static + Send + Sync + Into<Request>,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_from::<V>();
        self
    }

    /// Register a mapping from some other message type `V` into the request
    /// type of this node. This allows you to register a custom mapping.
    ///
    /// Calling this multiple times for the same message pair will override any
    /// previously registered mapping for that pair.
    pub fn with_mapping_from<V>(
        &mut self,
        f: impl Fn(V) -> Request + 'static + Send + Sync,
    ) -> &mut Self
    where
        V: 'static + Send + Sync,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_mapping_from(f);
        self
    }

    /// Register the [`TryInto`] implementation that maps the response type of
    /// this node into some other message type `U`.
    pub fn with_try_into<U>(&mut self) -> &mut Self
    where
        U: 'static + Send + Sync,
        Response: TryInto<U>,
        Response::Error: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_try_into::<U>();
        self
    }

    /// Register a fallible mapping from the response type of this node into
    /// some other message type. This allows you to register a custom mapping.
    ///
    /// Registering this multiple times for the same message pair will override
    /// the previously registered mapping.
    pub fn with_mapping_try_into<U, E>(
        &mut self,
        f: impl Fn(Response) -> Result<U, E> + 'static + Send + Sync,
    ) -> &mut Self
    where
        U: 'static + Send + Sync,
        E: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_mapping_try_into(f);
        self
    }

    /// Register the [`TryFrom`] implementation that maps from some other
    /// message type `V` into the request type of this node.
    pub fn with_try_from<V>(&mut self) -> &mut Self
    where
        V: 'static + Send + Sync + TryInto<Request>,
        <V as TryInto<Request>>::Error: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_try_from::<V>();
        self
    }

    /// Register a fallible mapping from some other message type `V` into the
    /// request type of this node. This allows you to register a custom mapping.
    ///
    /// Calling this multiple times for the same message pair will override any
    /// previous registered mapping for that pair.
    pub fn with_mapping_try_from<V, E>(
        &mut self,
        f: impl Fn(V) -> Result<Request, E> + 'static + Send + Sync,
    ) -> &mut Self
    where
        V: 'static + Send + Sync,
        E: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_mapping_try_from(f);
        self
    }
}

pub trait IntoNodeRegistration {
    fn into_node_registration(
        self,
        id: BuilderId,
        name: String,
        schema_generator: &mut SchemaGenerator,
    ) -> NodeRegistration;
}

type CreateSectionFn =
    dyn FnMut(&mut Builder, serde_json::Value) -> Result<Box<dyn Section>, DiagramErrorCode> + Send;

pub struct SectionRegistration {
    pub(crate) metadata: SectionMetadata,
    create_section_impl: RefCell<Box<CreateSectionFn>>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SectionMetadata {
    pub(super) default_display_text: DisplayText,
    pub(super) interface: SectionInterface,
    pub(super) config_schema: Schema,
    pub(super) description: Option<String>,
    pub(super) config_examples: Vec<ConfigExample>,
}

impl SectionRegistration {
    pub(super) fn create_section(
        &self,
        builder: &mut Builder,
        config: serde_json::Value,
    ) -> Result<Box<dyn Section>, DiagramErrorCode> {
        let mut create_section_impl = self.create_section_impl.borrow_mut();
        let section = create_section_impl(builder, config)?;
        Ok(section)
    }
}

pub struct DiagramElementRegistry {
    pub(super) nodes: HashMap<BuilderId, NodeRegistration>,
    pub(super) sections: HashMap<BuilderId, SectionRegistration>,
    pub(super) messages: MessageRegistry,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DiagramElementMetadata {
    nodes: HashMap<BuilderId, NodeMetadata>,
    sections: HashMap<BuilderId, SectionMetadata>,
    messages: Vec<MessageMetadata>,
    schemas: serde_json::Map<String, JsonMessage>,
    trace_supported: bool,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeMetadata {
    /// If the user does not specify a default display text, the node ID will
    /// be used here.
    pub(super) default_display_text: DisplayText,
    pub(super) request: usize,
    pub(super) response: usize,
    pub(super) streams: HashMap<Cow<'static, str>, usize>,
    pub(super) config_schema: Schema,
    pub(super) description: Option<String>,
    pub(super) config_examples: Vec<ConfigExample>,
}

pub struct MessageOperations {
    pub(super) deserialize_impl: Option<DeserializeFn>,
    pub(super) serialize_impl: Option<SerializeFn>,
    pub(super) fork_clone_impl: Option<ForkCloneFn>,
    pub(super) unzip_impl: Option<UnzipRegistration>,
    pub(super) fork_result: Option<ForkResultRegistration>,
    pub(super) split_impl: Option<SplitFn>,
    pub(super) join_impl: Option<JoinRegistration>,
    pub(super) buffer_access_impl: Option<BufferAccessFn>,
    pub(super) accessor_hints: Option<BufferLayoutTypeHintFn>,
    pub(super) listen_impl: Option<ListenFn>,
    pub(super) listen_hints: Option<BufferLayoutTypeHintFn>,
    pub(super) to_string_impl: Option<ToStringFn>,
    pub(super) create_buffer_impl: CreateBufferFn,
    pub(super) create_trigger_impl: CreateTriggerFn,
    pub(super) into_impls: HashMap<usize, CreateIntoFn>,
    pub(super) from_impls: HashMap<usize, CreateIntoFn>,
    pub(super) try_into_impls: HashMap<usize, CreateTryIntoFn>,
    pub(super) try_from_impls: HashMap<usize, CreateTryIntoFn>,
    build_scope: BuildScope,

    #[cfg(feature = "trace")]
    pub(super) enable_trace_serialization: Option<EnableTraceSerializeFn>,
}

impl MessageOperations {
    pub fn new<T>() -> Self
    where
        T: Send + Sync + 'static + Any,
    {
        Self {
            deserialize_impl: None,
            serialize_impl: None,
            fork_clone_impl: None,
            unzip_impl: None,
            fork_result: None,
            split_impl: None,
            join_impl: None,
            buffer_access_impl: None,
            accessor_hints: None,
            listen_impl: None,
            listen_hints: None,
            to_string_impl: None,
            create_buffer_impl: |settings, builder| {
                builder.create_buffer::<T>(settings).as_any_buffer()
            },
            create_trigger_impl: |builder| builder.create_map_block(|_: T| ()).into(),
            build_scope: BuildScope::new::<T>(),
            into_impls: Default::default(),
            try_into_impls: Default::default(),
            from_impls: Default::default(),
            try_from_impls: Default::default(),

            #[cfg(feature = "trace")]
            enable_trace_serialization: None,
        }
    }

    pub fn metadata(&self) -> MessageOperationsMetadata {
        MessageOperationsMetadata::new(self)
    }
}

/// Represents an empty js object.
///
/// ```json
/// { "type": "object" }
/// ```
#[derive(Clone)]
struct JsEmptyObject;

impl std::fmt::Debug for JsEmptyObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("empty").finish()
    }
}

impl Serialize for JsEmptyObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_map(Some(0))?.end()
    }
}

impl<'de> Deserialize<'de> for JsEmptyObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(serde::de::IgnoredAny)?;
        Ok(JsEmptyObject {})
    }
}

impl JsonSchema for JsEmptyObject {
    fn schema_name() -> Cow<'static, str> {
        "object".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "object" })
    }

    fn inline_schema() -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct MessageMetadata {
    type_name: Cow<'static, str>,
    schema: Option<Schema>,
    operations: Option<MessageOperationsMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct MessageOperationsMetadata {
    deserialize: Option<JsEmptyObject>,
    serialize: Option<JsEmptyObject>,
    fork_clone: Option<JsEmptyObject>,
    unzip: Option<Vec<usize>>,
    fork_result: Option<[usize; 2]>,
    split: Option<JsEmptyObject>,
    join: Option<BufferMapLayoutHints<usize>>,
    into: HashSet<usize>,
    try_into: HashSet<usize>,
    from: HashSet<usize>,
    try_from: HashSet<usize>,
}

impl MessageOperationsMetadata {
    fn new(ops: &MessageOperations) -> Self {
        Self {
            deserialize: ops.deserialize_impl.is_some().then(|| JsEmptyObject),
            serialize: ops.serialize_impl.is_some().then(|| JsEmptyObject),
            fork_clone: ops.fork_clone_impl.is_some().then(|| JsEmptyObject),
            unzip: ops.unzip_impl.as_ref().map(|unzip| unzip.output_types.clone()),
            fork_result: ops.fork_result.as_ref().map(|r| r.output_types),
            split: ops.split_impl.is_some().then(|| JsEmptyObject),
            join: ops.join_impl.as_ref().map(|op| op.layout.clone()),
            into: ops.into_impls.keys().copied().collect(),
            try_into: ops.try_into_impls.keys().copied().collect(),
            from: ops.from_impls.keys().copied().collect(),
            try_from: ops.try_from_impls.keys().copied().collect(),
        }
    }
}

pub struct MessageRegistration {
    pub(super) type_info: TypeInfo,
    pub(super) schema: Option<Schema>,
    /// We wrap operations in Option because there are some cases where we need
    /// to reference a message type via TypeInfo before it gets registered with
    /// its concrete type information. We can't register operations for a message
    /// type without its concrete type information, so instead we will allocate
    /// an index for it and leave its operations blank until later.
    pub(super) operations: Option<MessageOperations>,
}

impl MessageRegistration {
    pub(super) fn new<T>() -> Self
    where
        T: Send + Sync + 'static + Any,
    {
        Self {
            type_info: TypeInfo::of::<T>(),
            schema: None,
            operations: Some(MessageOperations::new::<T>()),
        }
    }

    fn placeholder(type_info: TypeInfo) -> Self {
        Self {
            type_info,
            schema: None,
            operations: None,
        }
    }

    pub fn get_operations(&self) -> Result<&MessageOperations, DiagramErrorCode> {
        self
        .operations
        .as_ref()
        .ok_or_else(|| DiagramErrorCode::UnregisteredTypes(vec![self.type_info]))
    }
}

pub struct MessageRegistry {
    pub registration: MessageRegistrations,
    pub schema_generator: SchemaGenerator,
}

impl MessageRegistry {
    fn new() -> Self {
        let mut settings = SchemaSettings::default();
        settings.definitions_path = "#/schemas/".into();

        Self {
            registration: Default::default(),
            schema_generator: SchemaGenerator::new(settings),
        }
    }

    pub(crate) fn get_dyn(&self, target_type: &TypeInfo) -> Option<&MessageRegistration> {
        self.registration.get_dyn(target_type)
    }

    pub fn deserialize(
        &self,
        target_type: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<DynForkResult, DiagramErrorCode> {
        self.try_deserialize(target_type, builder)?
            .ok_or_else(|| DiagramErrorCode::NotDeserializable(*target_type))
    }

    pub fn try_deserialize(
        &self,
        target_type: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<Option<DynForkResult>, DiagramErrorCode> {
        self.get_operations(target_type)?
            .deserialize_impl
            .map(|deserialize| deserialize(builder))
            .transpose()
    }

    /// Register a deserialize function if not already registered, returns true if the new
    /// function is registered.
    pub(super) fn register_deserialize<T, Deserializer>(&mut self)
    where
        T: Send + Sync + 'static + Any,
        Deserializer: DeserializeMessage<T>,
    {
        Deserializer::register_deserialize(&mut self.registration, &mut self.schema_generator);
    }

    pub fn serialize(
        &self,
        incoming_type: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<DynForkResult, DiagramErrorCode> {
        self.try_serialize(incoming_type, builder)?
            .ok_or_else(|| DiagramErrorCode::NotSerializable(*incoming_type))
    }

    pub fn try_serialize(
        &self,
        incoming_type: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<Option<DynForkResult>, DiagramErrorCode> {
        let ops = self.get_operations(incoming_type)?;
        ops.serialize_impl.map(|serialize| serialize(builder)).transpose()
    }

    pub fn try_to_string(
        &self,
        incoming_type: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<Option<DynNode>, DiagramErrorCode> {
        let ops = self.get_operations(incoming_type)?;
        Ok(ops.to_string_impl.map(|f| f(builder)))
    }

    /// Register a serialize function if not already registered, returns true if the new
    /// function is registered.
    pub(super) fn register_serialize<T, Serializer>(&mut self)
    where
        T: Send + Sync + 'static + Any,
        Serializer: SerializeMessage<T>,
    {
        Serializer::register_serialize(&mut self.registration, &mut self.schema_generator)
    }

    pub fn fork_clone(
        &self,
        message_info: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<DynForkClone, DiagramErrorCode> {
        let create = self.get_operations(message_info)?
            .fork_clone_impl
            .ok_or(DiagramErrorCode::NotCloneable(*message_info))?;

        create(builder)
    }

    /// Register a fork_clone function if not already registered, returns true if the new
    /// function is registered.
    pub(super) fn register_clone<T, F>(&mut self) -> bool
    where
        T: Send + Sync + 'static + Any,
        F: RegisterClone<T>,
    {
        let ops = &mut self
            .registration
            .get_or_insert_operations::<T>();
        if !F::CLONEABLE || ops.fork_clone_impl.is_some() {
            return false;
        }

        F::register_clone(ops);

        true
    }

    pub fn unzip<'a>(
        &'a self,
        message_info: &TypeInfo,
    ) -> Result<&'a UnzipRegistration, DiagramErrorCode> {
        self.get_operations(message_info)?
            .unzip_impl
            .as_ref()
            .ok_or(DiagramErrorCode::NotUnzippable(*message_info))
    }

    /// Register a unzip function if not already registered, returns true if the new
    /// function is registered.
    pub(super) fn register_unzip<T, Serializer, Cloneable>(&mut self) -> bool
    where
        T: Send + Sync + 'static + Any,
        Serializer: 'static,
        Cloneable: 'static,
        Supported<(T, Serializer, Cloneable)>: RegisterUnzip,
    {
        let unzip_impl = Supported::<(T, Serializer, Cloneable)>::register_unzip(self);

        let ops = self
            .registration
            .get_or_insert_operations::<T>();
        if ops.unzip_impl.is_some() {
            return false;
        }
        ops.unzip_impl = Some(unzip_impl);

        true
    }

    pub fn fork_result(
        &self,
        message_info: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<DynForkResult, DiagramErrorCode> {
        let create = self
            .get_operations(message_info)?
            .fork_result
            .as_ref()
            .ok_or(DiagramErrorCode::CannotForkResult(*message_info))?
            .create;

        create(builder)
    }

    /// Register a fork_result function if not already registered, returns true if the new
    /// function is registered.
    pub(super) fn register_result<R>(&mut self) -> bool
    where
        R: RegisterForkResult,
    {
        R::on_register(self)
    }

    pub fn split(
        &self,
        message_info: &TypeInfo,
        split_op: &SplitSchema,
        builder: &mut Builder,
    ) -> Result<DynSplit, DiagramErrorCode> {
        let create = self
            .get_operations(message_info)?
            .split_impl
            .ok_or(DiagramErrorCode::NotSplittable(*message_info))?;

        create(split_op, builder)
    }

    /// Register a split function if not already registered.
    pub(super) fn register_split<T, S, C>(&mut self)
    where
        T: Send + Sync + 'static + Any,
        Supported<(T, S, C)>: RegisterSplit,
    {
        Supported::<(T, S, C)>::on_register(self);
    }

    pub fn create_buffer(
        &self,
        message_info: &TypeInfo,
        settings: BufferSettings,
        builder: &mut Builder,
    ) -> Result<AnyBuffer, DiagramErrorCode> {
        let f = self
            .get_operations(message_info)?
            .create_buffer_impl;

        Ok(f(settings, builder))
    }

    pub(crate) fn set_scope_request(
        &self,
        message_info: &TypeInfo,
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> Result<IncrementalScopeRequest, DiagramErrorCode> {
        let f = self
            .get_operations(message_info)?
            .build_scope
            .set_request;

        f(incremental, commands).map_err(Into::into)
    }

    pub(crate) fn set_scope_response(
        &self,
        message_info: &TypeInfo,
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> Result<IncrementalScopeResponse, DiagramErrorCode> {
        let f = self
            .get_operations(message_info)?
            .build_scope
            .set_response;

        f(incremental, commands).map_err(Into::into)
    }

    pub(crate) fn spawn_basic_scope_stream(
        &self,
        message_info: &TypeInfo,
        in_scope: Entity,
        out_scope: Entity,
        commands: &mut Commands,
    ) -> Result<(DynInputSlot, DynOutput), DiagramErrorCode> {
        let f = self
            .get_operations(message_info)?
            .build_scope
            .spawn_basic_scope_stream;

        Ok(f(in_scope, out_scope, commands))
    }

    pub fn trigger(
        &self,
        message_info: &TypeInfo,
        builder: &mut Builder,
    ) -> Result<DynNode, DiagramErrorCode> {
        let f = self.get_operations(message_info)?
            .create_trigger_impl;

        Ok(f(builder))
    }

    pub fn join(
        &self,
        joinable: &TypeInfo,
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<DynOutput, DiagramErrorCode> {
        let create = self.get_operations(joinable)?
            .join_impl
            .as_ref()
            .ok_or_else(|| DiagramErrorCode::NotJoinable(*joinable))?
            .create;

        create(buffers, builder)
    }

    /// Register a join function if not already registered, returns true if the
    /// new function is registered.
    pub(super) fn register_join<T>(&mut self)
    where
        T: Send + Sync + 'static + Any + Joined,
    {
        let join = JoinRegistration::new::<T>(self);

        self
            .registration
            .get_or_insert_operations::<T>()
            .join_impl = Some(join);
    }

    pub fn with_buffer_access(
        &self,
        target_type: &TypeInfo,
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<DynNode, DiagramErrorCode> {
        let create = self.get_operations(target_type)?
            .buffer_access_impl
            .ok_or(DiagramErrorCode::CannotAccessBuffers(*target_type))?;

        create(buffers, builder)
    }

    pub fn accessor_hint(
        &self,
        message_info: &TypeInfo,
        identifiers: HashSet<BufferIdentifier<'static>>,
    ) -> Result<MessageTypeHintMap, DiagramErrorCode> {
        let hint = self.get_operations(message_info)?
            .accessor_hints
            .ok_or_else(|| DiagramErrorCode::CannotAccessBuffers(*message_info))?;

        hint(identifiers).map_err(Into::into)
    }

    pub(super) fn register_buffer_access<T>(&mut self) -> bool
    where
        T: Send + Sync + 'static + BufferAccessRequest,
    {
        let ops = &mut self
            .registration
            .get_or_insert_operations::<T>();
        if ops.buffer_access_impl.is_some() {
            return false;
        }

        ops.buffer_access_impl = Some(|buffers, builder| {
            let buffer_access =
                builder.try_create_buffer_access::<T::Message, T::BufferKeys>(buffers)?;
            Ok(buffer_access.into())
        });

        ops.accessor_hints = Some(<<T::BufferKeys as Accessor>::Buffers as BufferMapLayout>::get_buffer_message_type_hints);

        true
    }

    pub fn listen(
        &self,
        target_type: &TypeInfo,
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<DynOutput, DiagramErrorCode> {
        let create = self.get_operations(target_type)?
            .listen_impl
            .ok_or_else(|| DiagramErrorCode::CannotListen(*target_type))?;

        create(buffers, builder)
    }

    pub fn listen_hint(
        &self,
        message_info: &TypeInfo,
        identifiers: HashSet<BufferIdentifier<'static>>,
    ) -> Result<MessageTypeHintMap, DiagramErrorCode> {
        let hints = self.get_operations(message_info)?
            .listen_hints
            .ok_or_else(|| DiagramErrorCode::CannotListen(*message_info))?;

        hints(identifiers).map_err(Into::into)
    }

    pub(super) fn register_listen<T>(&mut self) -> bool
    where
        T: Send + Sync + 'static + Any + Accessor,
    {
        let ops = &mut self
            .registration
            .get_or_insert_operations::<T>();
        if ops.listen_impl.is_some() {
            return false;
        }

        ops.listen_impl =
            Some(|buffers, builder| Ok(builder.try_listen::<T>(buffers)?.output().into()));

        ops.listen_hints = Some(<T::Buffers as BufferMapLayout>::get_buffer_message_type_hints);

        true
    }

    pub(super) fn register_to_string<T>(&mut self)
    where
        T: 'static + Send + Sync + ToString,
    {
        let ops = &mut self
            .registration
            .get_or_insert_operations::<T>();

        ops.to_string_impl =
            Some(|builder| builder.create_map_block(|msg: T| msg.to_string()).into());
    }

    pub(crate) fn get_operations(
        &self,
        message_info: &TypeInfo,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        self.get_dyn(message_info)
            .map(|r| r.operations.as_ref())
            .flatten()
            .ok_or_else(|| DiagramErrorCode::UnregisteredTypes(vec![*message_info]))
    }
}

#[derive(Default)]
pub struct MessageRegistrations {
    messages: Vec<MessageRegistration>,

    /// Convert from type info to the index of a message wihtin the registry
    indices: HashMap<TypeInfo, usize>,

    /// Lookup message types that satisfy some constraint. This is used by
    /// message type inference.
    pub(crate) lookup: MessageLookup,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageLookup {
    /// Map from [T, E] output registrations to Result<T, E> registration.
    pub(crate) result: HashMap<[usize; 2], usize>,
    pub(crate) unzip: HashMap<Vec<usize>, usize>,
}

impl MessageRegistrations {
    pub fn iter(&self) -> std::slice::Iter<MessageRegistration> {
        self.messages.iter()
    }

    fn get<T>(&self) -> Option<&MessageRegistration>
    where
        T: Any,
    {
        self.get_dyn(&TypeInfo::of::<T>())
    }

    pub(crate) fn get_dyn(&self, target_type: &TypeInfo) -> Option<&MessageRegistration> {
        self
        .indices
        .get(target_type)
        .map(|index| self.messages.get(*index))
        .flatten()
    }

    pub(crate) fn get_by_index(
        &self,
        index: usize,
    ) -> Result<&MessageRegistration, DiagramErrorCode> {
        self.messages.get(index).ok_or_else(||
            DiagramErrorCode::UnknownMessageTypeIndex {
                index,
                limit: self.len(),
            })
    }

    pub(crate) fn len(&self) -> usize {
        self.messages.len()
    }

    pub(crate) fn get_or_insert<T>(&mut self) -> &mut MessageRegistration
    where
        T: 'static + Send + Sync,
    {
        let target_type = TypeInfo::of::<T>();
        if let Some(index) = self.indices.get(&target_type) {
            // SAFETY: self.message_indices and self.messages are kept in sync
            // by the implementation of MessageRegistry. If we find an entry in
            // self.message_indices then there must be a matching entry in
            // self.messages.
            self.messages.get_mut(*index).unwrap()
        } else {
            let index = self.messages.len();
            self.indices.insert(target_type, index);
            self.messages.push(MessageRegistration::new::<T>());

            // SAFETY: We just pushed an entry in the previous line, and now we
            // just want to retrieve a mutable borrow of it.
            self.messages.last_mut().unwrap()
        }
    }

    pub(crate) fn get_or_insert_operations<T>(&mut self) -> &mut MessageOperations
    where
        T: 'static + Send + Sync
    {
        self.get_or_insert::<T>()
            .operations
            .get_or_insert_with(|| MessageOperations::new::<T>())
    }

    pub(crate) fn get_index_dyn(&self, target_type: &TypeInfo) -> Option<usize> {
        self.indices.get(target_type).cloned()
    }

    // Used in testing
    #[allow(unused)]
    pub(crate) fn get_index<T>(&self) -> Option<usize>
    where
        T: 'static + Send + Sync,
    {
        self.indices.get(&TypeInfo::of::<T>()).cloned()
    }

    pub(crate) fn get_index_or_insert<T>(&mut self) -> usize
    where
        T: 'static + Send + Sync,
    {
        let target_type = TypeInfo::of::<T>();
        if let Some(index) = self.indices.get(&target_type) {
            *index
        } else {
            self.new_message_registration(
                target_type,
                MessageRegistration::new::<T>(),
            )
        }
    }

    pub(crate) fn get_index_or_insert_placeholder(&mut self, message_info: TypeInfo) -> usize {
        if let Some(index) = self.indices.get(&message_info) {
            *index
        } else {
            self.new_message_registration(
                message_info,
                MessageRegistration::placeholder(message_info),
            )
        }
    }

    fn new_message_registration(
        &mut self,
        message_info: TypeInfo,
        registration: MessageRegistration,
    ) -> usize {
        let index = self.messages.len();
        self.indices.insert(message_info, index);
        self.messages.push(registration);
        index
    }

    /// Get the metadata of the registered messages
    fn metadata(&self) -> Vec<MessageMetadata> {
        let mut metadata = Vec::new();
        for message in &self.messages {
            let operations = message.operations.as_ref().map(|ops| ops.metadata());

            metadata.push(MessageMetadata {
                type_name: Cow::Borrowed(message.type_info.type_name),
                schema: message.schema.clone(),
                operations,
            });
        }

        metadata
    }
}

impl<'a> IntoIterator for &'a MessageRegistrations {
    type IntoIter = std::slice::Iter<'a, MessageRegistration>;
    type Item = &'a MessageRegistration;
    fn into_iter(self) -> Self::IntoIter {
        self.messages.iter()
    }
}

impl Default for DiagramElementRegistry {
    fn default() -> Self {
        // Ensure buffer downcasting is automatically registered for all basic
        // serializable types.
        JsonBuffer::register_for::<()>();

        let mut registry = DiagramElementRegistry {
            nodes: Default::default(),
            sections: Default::default(),
            messages: MessageRegistry::new(),
        };

        registry.register_builtin_messages();
        registry
    }
}

impl DiagramElementRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new registry that does not automatically register any of the
    /// builtin types. Only advanced users who know what they are doing should
    /// use this.
    pub fn blank() -> Self {
        JsonBuffer::register_for::<()>();
        DiagramElementRegistry {
            nodes: Default::default(),
            sections: Default::default(),
            messages: MessageRegistry::new(),
        }
    }

    /// Get the metadata of this registry. The metadata can be serialized and
    /// deserialized.
    ///
    /// This metadata can be sent to remote clients so they know what node and
    /// section builders are available, as well as what message types are
    /// registered and what operations can be performed on them.
    pub fn metadata(&self) -> DiagramElementMetadata {
        let nodes = self
            .nodes
            .iter()
            .map(|(id, node)|
                (Arc::clone(id), node.metadata.clone())
            )
            .collect();

        let sections = self
            .sections
            .iter()
            .map(|(id, section)|
                (Arc::clone(id), section.metadata.clone())
            )
            .collect();

        let messages = self.messages.registration.metadata();
        let schemas = self.messages.schema_generator.definitions().clone();

        DiagramElementMetadata {
            nodes,
            sections,
            messages,
            schemas,
            trace_supported: crate::trace_supported(),
        }
    }

    /// Register a node builder with all the common operations (deserialize the
    /// request, serialize the response, and clone the response) enabled.
    ///
    /// You will receive a [`NodeRegistrationBuilder`] which you can then use to
    /// enable more operations around your node, such as fork result, split,
    /// or unzip. The data types of your node need to be suitable for those
    /// operations or else the compiler will not allow you to enable them.
    ///
    /// Node builders registered with this cannot fail to build their node when
    /// asked to. If your builder might be able to fail, use
    /// [`Self::register_node_builder_fallible`] instead.
    ///
    /// ```
    /// use crossflow::{NodeBuilderOptions, DiagramElementRegistry};
    ///
    /// let mut registry = DiagramElementRegistry::new();
    /// registry.register_node_builder(
    ///     NodeBuilderOptions::new("echo".to_string()),
    ///     |builder, _config: ()| builder.create_map_block(|msg: String| msg)
    /// );
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Id of the builder, this must be unique.
    /// * `name` - Friendly name for the builder, this is only used for display purposes.
    /// * `f` - The node builder to register.
    pub fn register_node_builder<Config, Request, Response, Streams: StreamPack>(
        &mut self,
        options: NodeBuilderOptions,
        builder: impl FnMut(&mut Builder, Config) -> Node<Request, Response, Streams> + Send + 'static,
    ) -> NodeRegistrationBuilder<'_, Request, Response, Streams>
    where
        Config: JsonSchema + DeserializeOwned,
        Request: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Response: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Streams::StreamTypes: RegisterStreams<Supported, Supported, Supported>,

    {
        self.opt_out().register_node_builder(options, builder)
    }

    /// Equivalent to [`Self::register_node_builder`] except the builder is allowed
    /// to fail building the node by returning [`Err`]. When [`Err`] is returned,
    /// building of the entire diagram will be cancelled and the user will receive
    /// an error.
    pub fn register_node_builder_fallible<Config, Request, Response, Streams: StreamPack>(
        &mut self,
        options: NodeBuilderOptions,
        builder: impl FnMut(&mut Builder, Config) -> Result<Node<Request, Response, Streams>, Anyhow>
        + Send
        + 'static,
    ) -> NodeRegistrationBuilder<'_, Request, Response, Streams>
    where
        Config: JsonSchema + DeserializeOwned,
        Request: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Response: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Streams::StreamTypes: RegisterStreams<Supported, Supported, Supported>,
    {
        self.opt_out()
            .register_node_builder_fallible(options, builder)
    }

    /// Register a single message for general use between nodes. This will
    /// include all common operations for the message (deserialize, serialize,
    /// and clone).
    ///
    /// You will receive a [`MessageRegistrationBuilder`] which you can then use
    /// to enable more operations for the message, such as forking, splitting,
    /// unzipping, and joining. The message type needs to be suitable for each
    /// operation that you register for it or else the compiler will not allow
    /// you to enable them.
    ///
    /// Use [`Self::opt_out`] to opt out of specified common operations before
    /// beginning to register the message. This allows you to register message
    /// types that do not support one or more of the common operations.
    pub fn register_message<Message>(&mut self) -> MessageRegistrationBuilder<'_, Message>
    where
        Message: Send + Sync + 'static + DynType + DeserializeOwned + Serialize + Clone,
    {
        self.opt_out().register_message()
    }

    /// Register a section builder with the specified common operations.
    ///
    /// # Arguments
    ///
    /// * `id` - Id of the builder, this must be unique.
    /// * `name` - Friendly name for the builder, this is only used for display purposes.
    /// * `f` - The section builder to register.
    pub fn register_section_builder<Config, SectionT>(
        &mut self,
        options: SectionBuilderOptions,
        mut section_builder: impl FnMut(&mut Builder, Config) -> SectionT + Send + 'static,
    ) where
        SectionT: Section + SectionInterfaceDescription + 'static,
        Config: DeserializeOwned + JsonSchema,
    {
        self.register_section_builder_fallible(options, move |builder, config| {
            Ok(section_builder(builder, config))
        });
    }

    /// Equivalent to [`Self::register_section_builder`] except the builder is
    /// allowed to fail building the section by returning [`Err`]. When [`Err`]
    /// is returned, building tof the entire diagram will be cancelled and the
    /// user will receive an error.
    pub fn register_section_builder_fallible<Config, SectionT>(
        &mut self,
        options: SectionBuilderOptions,
        mut section_builder: impl FnMut(&mut Builder, Config) -> Result<SectionT, Anyhow>
        + Send
        + 'static,
    ) where
        SectionT: Section + SectionInterfaceDescription + 'static,
        Config: DeserializeOwned + JsonSchema,
    {
        let builder_id = Arc::clone(&options.id);
        let registration = SectionRegistration {
            metadata: SectionMetadata {
                default_display_text: options
                    .default_display_text
                    .as_ref()
                    .unwrap_or(&options.id)
                    .clone(),
                interface: SectionT::interface_metadata(&mut self.messages.registration).clone(),
                config_schema: self.messages.schema_generator.subschema_for::<()>(),
                description: options.description.clone(),
                config_examples: options.config_examples.clone(),
            },
            create_section_impl: RefCell::new(Box::new(move |builder, config| {
                let section =
                    section_builder(builder, serde_json::from_value::<Config>(config).unwrap())
                        .map_err(|error| DiagramErrorCode::NodeBuildingError {
                            builder: Arc::clone(&builder_id),
                            error,
                        })?;
                Ok(Box::new(section))
            })),
        };

        self.sections.insert(options.id, registration);
        SectionT::on_register(self);
    }

    /// In some cases the common operations of deserialization, serialization,
    /// and cloning cannot be performed for the input or output message of a node.
    /// When that happens you can still register your node builder by calling
    /// this function and explicitly disabling the common operations that your
    /// node cannot support.
    ///
    /// In order for a message type to support all the common operations, it
    /// must implement [`schemars::JsonSchema`], [`serde::de::DeserializeOwned`],
    /// [`serde::Serialize`], and [`Clone`].
    ///
    /// ```
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(JsonSchema, Serialize, Deserialize, Clone)]
    /// struct MyCommonMessage {}
    /// ```
    ///
    /// If your node has an input or output message that is missing one of these
    /// traits, you can still register it by opting out of the relevant common
    /// operation(s):
    ///
    /// ```
    /// use crossflow::{NodeBuilderOptions, DiagramElementRegistry};
    ///
    /// struct NonSerializable {
    ///     data: String
    /// }
    ///
    /// let mut registry = DiagramElementRegistry::new();
    /// registry
    ///     .opt_out()
    ///     .no_deserializing()
    ///     .no_serializing()
    ///     .no_cloning()
    ///     .register_node_builder(
    ///         NodeBuilderOptions::new("echo"),
    ///         |builder, _config: ()| {
    ///             builder.create_map_block(|msg: NonSerializable| msg)
    ///         }
    ///     );
    /// ```
    ///
    /// Note that nodes registered without deserialization cannot be connected
    /// to the workflow start, and nodes registered without serialization cannot
    /// be connected to the workflow termination.
    pub fn opt_out(&mut self) -> CommonOperations<'_, Supported, Supported, Supported> {
        CommonOperations {
            registry: self,
            _ignore: Default::default(),
        }
    }

    pub fn get_node_registration<Q>(&self, id: &Q) -> Result<&NodeRegistration, DiagramErrorCode>
    where
        Q: Borrow<str> + ?Sized,
    {
        let k = id.borrow();
        self.nodes
            .get(k)
            .ok_or(DiagramErrorCode::BuilderNotFound(k.to_string().into()))
    }

    pub fn get_section_registration<Q>(
        &self,
        id: &Q,
    ) -> Result<&SectionRegistration, DiagramErrorCode>
    where
        Q: Borrow<str> + ?Sized,
    {
        self.sections
            .get(id.borrow())
            .ok_or_else(|| DiagramErrorCode::BuilderNotFound(id.borrow().to_string().into()))
    }

    pub fn get_message_registration<T>(&self) -> Option<&MessageRegistration>
    where
        T: Any,
    {
        self.messages.registration.get::<T>()
    }

    /// Register useful messages that are known to the crossflow library.
    /// This will be run automatically when you create using [`Self::default()`]
    /// or [`Self::new()`].
    pub fn register_builtin_messages(&mut self) {
        self.register_message::<JsonMessage>()
            .with_join()
            .with_split();

        self.opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_message::<TransformError>()
            .with_to_string();

        self.register_message::<String>();
        self.register_message::<u8>();
        self.register_message::<u16>();
        self.register_message::<u32>();
        self.register_message::<u64>();
        self.register_message::<usize>();
        self.register_message::<i8>();
        self.register_message::<i16>();
        self.register_message::<i32>();
        self.register_message::<i64>();
        self.register_message::<isize>();
        self.register_message::<f32>();
        self.register_message::<f64>();
        self.register_message::<bool>();
        self.register_message::<char>();
        self.register_message::<()>();
    }
}

#[derive(Clone)]
#[non_exhaustive]
pub struct NodeBuilderOptions {
    /// The unique identifier for this node builder. Diagrams will use this ID
    /// to refer to this node builder.
    pub id: BuilderId,
    /// If this is not specified, the id field will be used as the default
    /// display text.
    pub default_display_text: Option<BuilderId>,
    /// Optional text to describe the builder.
    pub description: Option<String>,
    /// Examples of configurations that can be used with this node builder.
    pub config_examples: Vec<ConfigExample>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigExample {
    /// A description of what this config is for
    pub description: String,
    /// The value of the config
    pub config: JsonMessage,
}

impl ConfigExample {
    /// Create a new config example.
    ///
    /// Note that this function will panic if the `config` argument fails to be
    /// serialized into a [`JsonMessage`], which happens if the data structure
    /// contains a map with non-string keys or its [`Serialize`] implementation
    /// produces an error. It's recommended to only use this during application
    /// startup to avoid runtime failures.
    ///
    /// To construct a [`ConfigExample`] with no risk of panicking, you can
    /// directly use normal structure initialization.
    pub fn new(description: impl ToString, config: impl Serialize) -> Self {
        Self {
            description: description.to_string(),
            config: serde_json::to_value(config).expect("failed to serialize example config"),
        }
    }
}

impl NodeBuilderOptions {
    pub fn new(id: impl Into<BuilderId>) -> Self {
        Self {
            id: id.into(),
            default_display_text: None,
            description: None,
            config_examples: Default::default(),
        }
    }

    pub fn with_default_display_text(mut self, text: impl Into<DisplayText>) -> Self {
        self.default_display_text = Some(text.into());
        self
    }

    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn with_config_examples(
        mut self,
        config_examples: impl IntoIterator<Item = ConfigExample>,
    ) -> Self {
        self.config_examples = config_examples.into_iter().collect();
        self
    }
}

#[non_exhaustive]
pub struct SectionBuilderOptions {
    /// The unique identifier for this section builder. Diagrams will use this
    /// ID to refer to this section builder.
    pub id: BuilderId,
    /// If this is not specified, the id field will be used as the default
    /// display text.
    pub default_display_text: Option<BuilderId>,
    /// Optional text to describe the builder.
    pub description: Option<String>,
    /// Examples of configurations that can be used with this section builder.
    pub config_examples: Vec<ConfigExample>,
}

impl SectionBuilderOptions {
    pub fn new(id: impl Into<BuilderId>) -> Self {
        Self {
            id: id.into(),
            default_display_text: None,
            description: None,
            config_examples: Default::default(),
        }
    }

    pub fn with_default_display_text(mut self, text: impl Into<DisplayText>) -> Self {
        self.default_display_text = Some(text.into());
        self
    }

    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn with_config_examples(
        mut self,
        config_examples: impl IntoIterator<Item = ConfigExample>,
    ) -> Self {
        self.config_examples = config_examples.into_iter().collect();
        self
    }
}

pub trait RegisterStreams<DeserializeImpl, SerializeImpl, Cloneable> {
    fn register_streams<'a>(
        registry: &mut CommonOperations<'a, DeserializeImpl, SerializeImpl, Cloneable>,
    );
}

impl<D, S, C> RegisterStreams<D, S, C> for () {
    fn register_streams(
        _: &mut CommonOperations<D, S, C>,
    ) {
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

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::Deserialize;

    use super::*;
    use crate::*;

    fn multiply3(i: i64) -> i64 {
        i * 3
    }

    #[derive(StreamPack)]
    struct TestStreamRegistration {
        foo_stream: i64,
        bar_stream: f64,
        baz_stream: String,
    }

    /// Some extra impl only used in tests (for now).
    /// If these impls are needed outside tests, then move them to the main impl.
    impl MessageOperations {
        fn deserializable(&self) -> bool {
            self.deserialize_impl.is_some()
        }

        fn serializable(&self) -> bool {
            self.serialize_impl.is_some()
        }

        fn cloneable(&self) -> bool {
            self.fork_clone_impl.is_some()
        }

        fn unzippable(&self) -> bool {
            self.unzip_impl.is_some()
        }

        fn can_fork_result(&self) -> bool {
            self.fork_result.is_some()
        }

        fn splittable(&self) -> bool {
            self.split_impl.is_some()
        }

        fn joinable(&self) -> bool {
            self.join_impl.is_some()
        }
    }

    #[test]
    fn test_register_node_builder() {
        let mut registry = DiagramElementRegistry::new();
        registry.opt_out().register_node_builder(
            NodeBuilderOptions::new("multiply3").with_default_display_text("Test Name"),
            |builder, _config: ()| builder.create_map_block(multiply3),
        );
        let req_ops = registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        let resp_ops = registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        assert!(req_ops.deserializable());
        assert!(resp_ops.serializable());
        assert!(resp_ops.cloneable());
        assert!(!resp_ops.unzippable());
        assert!(!resp_ops.can_fork_result());
        assert!(!resp_ops.splittable());
        assert!(!resp_ops.joinable());
    }

    #[test]
    fn test_register_cloneable_node() {
        let mut registry = DiagramElementRegistry::new();
        registry.register_node_builder(
            NodeBuilderOptions::new("multiply3").with_default_display_text("Test Name"),
            |builder, _config: ()| builder.create_map_block(multiply3),
        );
        let req_ops = &registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        let resp_ops = &registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        assert!(req_ops.deserializable());
        assert!(resp_ops.serializable());
        assert!(resp_ops.cloneable());
    }

    #[test]
    fn test_register_unzippable_node() {
        let mut registry = DiagramElementRegistry::new();
        let tuple_resp = |_: ()| -> (i64,) { (1,) };
        registry
            .opt_out()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("multiply3_uncloneable")
                    .with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| builder.create_map_block(tuple_resp),
            )
            .with_unzip();
        let req_ops = &registry.messages.registration.get::<()>().unwrap().operations.as_ref().unwrap();
        let resp_ops = &registry.messages.registration.get::<(i64,)>().unwrap().operations.as_ref().unwrap();
        assert!(req_ops.deserializable());
        assert!(resp_ops.serializable());
        assert!(resp_ops.unzippable());
    }

    #[test]
    fn test_register_splittable_node() {
        let mut registry = DiagramElementRegistry::new();
        let vec_resp = |_: ()| -> Vec<i64> { vec![1, 2] };

        registry
            .register_node_builder(
                NodeBuilderOptions::new("vec_resp").with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| builder.create_map_block(vec_resp),
            )
            .with_split();
        assert!(
            registry
                .messages
                .registration
                .get::<Vec<i64>>()
                .unwrap()
                .operations
                .as_ref()
                .unwrap()
                .splittable()
        );

        let map_resp = |_: ()| -> HashMap<String, i64> { HashMap::new() };
        registry
            .register_node_builder(
                NodeBuilderOptions::new("map_resp").with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| builder.create_map_block(map_resp),
            )
            .with_split();
        assert!(
            registry
                .messages
                .registration
                .get::<HashMap<String, i64>>()
                .unwrap()
                .operations
                .as_ref()
                .unwrap()
                .splittable()
        );

        registry.register_node_builder(
            NodeBuilderOptions::new("not_splittable").with_default_display_text("Test Name"),
            move |builder: &mut Builder, _config: ()| builder.create_map_block(map_resp),
        );
        // even though we didn't register with `with_split`, it is still splittable because we
        // previously registered another splittable node with the same response type.
        assert!(
            registry
                .messages
                .registration
                .get::<HashMap<String, i64>>()
                .unwrap()
                .operations
                .as_ref()
                .unwrap()
                .splittable()
        );
    }

    #[test]
    fn test_register_with_config() {
        let mut registry = DiagramElementRegistry::new();

        #[derive(Deserialize, JsonSchema)]
        struct TestConfig {
            by: i64,
        }

        registry.register_node_builder(
            NodeBuilderOptions::new("multiply").with_default_display_text("Test Name"),
            move |builder: &mut Builder, config: TestConfig| {
                builder.create_map_block(move |operand: i64| operand * config.by)
            },
        );
        assert!(registry.get_node_registration("multiply").is_ok());
    }

    struct NonSerializableRequest {}

    #[test]
    fn test_register_opaque_node() {
        let opaque_request_map = |_: NonSerializableRequest| {};

        let mut registry = DiagramElementRegistry::new();
        registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("opaque_request_map")
                    .with_default_display_text("Test Name"),
                move |builder, _config: ()| builder.create_map_block(opaque_request_map),
            )
            .with_serialize_response();
        assert!(registry.get_node_registration("opaque_request_map").is_ok());
        let req_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        let resp_ops = &registry.messages.registration.get::<()>().unwrap().operations.as_ref().unwrap();
        assert!(!req_ops.deserializable());
        assert!(resp_ops.serializable());

        let opaque_response_map = |_: ()| NonSerializableRequest {};
        registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("opaque_response_map")
                    .with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| {
                    builder.create_map_block(opaque_response_map)
                },
            )
            .with_deserialize_request();
        assert!(
            registry
                .get_node_registration("opaque_response_map")
                .is_ok()
        );
        let req_ops = &registry.messages.registration.get::<()>().unwrap().operations.as_ref().unwrap();
        let resp_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        assert!(req_ops.deserializable());
        assert!(!resp_ops.serializable());

        let opaque_req_resp_map = |_: NonSerializableRequest| NonSerializableRequest {};
        registry
            .opt_out()
            .no_deserializing()
            .no_serializing()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("opaque_req_resp_map")
                    .with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| {
                    builder.create_map_block(opaque_req_resp_map)
                },
            );
        assert!(
            registry
                .get_node_registration("opaque_req_resp_map")
                .is_ok()
        );

        let req_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();

        let resp_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        assert!(!req_ops.deserializable());
        assert!(!resp_ops.serializable());
    }

    #[test]
    fn test_register_message() {
        let mut registry = DiagramElementRegistry::new();

        #[derive(Deserialize, Serialize, JsonSchema, Clone)]
        struct TestMessage;

        registry.opt_out().register_message::<TestMessage>();

        let ops = &registry
            .get_message_registration::<TestMessage>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        assert!(ops.deserializable());
        assert!(ops.serializable());
        assert!(ops.cloneable());
        assert!(!ops.unzippable());
        assert!(!ops.can_fork_result());
        assert!(!ops.splittable());
        assert!(!ops.joinable());
    }

    #[test]
    fn test_serialize_registry() {
        let mut reg = DiagramElementRegistry::new();

        #[derive(Deserialize, Serialize, JsonSchema, Clone)]
        struct Foo {
            hello: String,
        }

        #[derive(Deserialize, Serialize, JsonSchema, Clone)]
        struct Bar {
            foo: Foo,
        }

        struct Opaque;

        reg.opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_node_builder(NodeBuilderOptions::new("test"), |builder, _config: ()| {
                builder.create_map_block(|_: Opaque| {
                    (
                        Foo {
                            hello: "hello".to_string(),
                        },
                        Bar {
                            foo: Foo {
                                hello: "world".to_string(),
                            },
                        },
                    )
                })
            })
            .with_unzip();

        reg.register_node_builder(NodeBuilderOptions::new("result_test"), |builder, _: ()| {
                builder.create_map_block(|value: f32| {
                    Ok::<f32, f32>(value)
                })
            })
            .with_result();

        reg.register_node_builder(
            NodeBuilderOptions::new("stream_test"),
            |builder, _config: ()| {
                builder.create_map(|input: BlockingMap<f64, TestStreamRegistration>| {
                    let value = input.request;
                    input.streams.foo_stream.send(value as i64);
                    input.streams.bar_stream.send(value);
                    input.streams.baz_stream.send(value.to_string());
                })
            },
        );

        // print out a pretty json for manual inspection
        println!("{}", serde_json::to_string_pretty(&reg.metadata()).unwrap());

        // test that schema refs are pointing to the correct path
        let value = serde_json::to_value(&reg.metadata()).unwrap();
        let messages = &value["messages"];
        let schemas = &value["schemas"];
        let bar_schema = &messages[reg.messages.registration.get_index::<Bar>().unwrap()]["schema"];
        assert_eq!(bar_schema["$ref"].as_str().unwrap(), "#/schemas/Bar");
        assert!(schemas.get("Bar").is_some());
        assert!(schemas.get("Foo").is_some());

        let nodes = &value["nodes"];
        let stream_test_schema = &nodes["stream_test"];
        let streams = &stream_test_schema["streams"];
        dbg!(&stream_test_schema);
        dbg!(&streams);
        assert_eq!(
            streams["foo_stream"].as_u64().unwrap() as usize,
            reg.messages.registration.get_index::<i64>().unwrap(),
        );
        assert_eq!(
            streams["bar_stream"].as_u64().unwrap() as usize,
            reg.messages.registration.get_index::<f64>().unwrap(),
        );
        assert_eq!(
            streams["baz_stream"].as_u64().unwrap() as usize,
            reg.messages.registration.get_index::<String>().unwrap(),
        );
    }

    #[test]
    fn test_serialize_js_empty_object() {
        let json = serde_json::to_string(&JsEmptyObject {}).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_deserialize_js_empty_object() {
        serde_json::from_str::<JsEmptyObject>("{}").unwrap();
        serde_json::from_str::<JsEmptyObject>(r#"{ "extra": "fields" }"#).unwrap();
        assert!(serde_json::from_str::<JsEmptyObject>(r#""some string""#).is_err());
        assert!(serde_json::from_str::<JsEmptyObject>("123").is_err());
        assert!(serde_json::from_str::<JsEmptyObject>("true").is_err());
        assert!(serde_json::from_str::<JsEmptyObject>("null").is_err());
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestFooBarBaz {
        foo: f32,
        bar: String,
        baz: u32,
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestFooBar {
        foo: f32,
        bar: String,
        baz: Option<u32>,
    }

    impl From<TestFooBarBaz> for TestFooBar {
        fn from(value: TestFooBarBaz) -> Self {
            TestFooBar {
                foo: value.foo,
                bar: value.bar,
                baz: Some(value.baz),
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestBarBaz {
        foo: Option<f32>,
        bar: String,
        baz: u32,
    }

    impl TryFrom<TestBarBaz> for TestFooBar {
        type Error = &'static str;
        fn try_from(value: TestBarBaz) -> Result<Self, Self::Error> {
            if let Some(foo) = value.foo {
                Ok(Self {
                    foo,
                    bar: value.bar,
                    baz: Some(value.baz),
                })
            } else {
                Err("missing foo")
            }
        }
    }

    impl TryFrom<TestFooBar> for TestBarBaz {
        type Error = &'static str;
        fn try_from(value: TestFooBar) -> Result<Self, Self::Error> {
            if let Some(baz) = value.baz {
                Ok(Self {
                    foo: Some(value.foo),
                    bar: value.bar,
                    baz,
                })
            } else {
                Err("missing baz")
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestMaybeFooBar {
        foo: Option<f32>,
        bar: Option<String>,
    }

    impl From<TestFooBar> for TestMaybeFooBar {
        fn from(value: TestFooBar) -> Self {
            TestMaybeFooBar {
                foo: Some(value.foo),
                bar: Some(value.bar),
            }
        }
    }

    impl TryFrom<TestMaybeFooBar> for TestFooBar {
        type Error = &'static str;
        fn try_from(value: TestMaybeFooBar) -> Result<Self, Self::Error> {
            if let (Some(foo), Some(bar)) = (value.foo, value.bar) {
                Ok(TestFooBar { foo, bar, baz: None })
            } else {
                Err("missing a field")
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestFoo {
        foo: f32,
    }

    impl From<TestFooBar> for TestFoo {
        fn from(value: TestFooBar) -> Self {
            TestFoo { foo: value.foo }
        }
    }

    impl TryFrom<TestMaybeFooBar> for TestFoo {
        type Error = &'static str;
        fn try_from(value: TestMaybeFooBar) -> Result<Self, Self::Error> {
            if let Some(foo) = value.foo {
                Ok(TestFoo { foo })
            } else {
                Err("missing foo")
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestBar {
        bar: String,
    }

    impl From<TestFooBar> for TestBar {
        fn from(value: TestFooBar) -> Self {
            TestBar { bar: value.bar }
        }
    }

    impl TryFrom<TestMaybeFooBar> for TestBar {
        type Error = &'static str;

        fn try_from(value: TestMaybeFooBar) -> Result<Self, Self::Error> {
            if let Some(bar) = value.bar {
                Ok(TestBar { bar })
            } else {
                Err("missing bar")
            }
        }
    }

    #[test]
    fn test_conversion_registration() {
        let mut registry = DiagramElementRegistry::new();
        registry
            .register_message::<TestFooBar>()
            .with_from::<TestFooBarBaz>()
            .with_try_from::<TestMaybeFooBar>()
            .with_try_from::<TestBarBaz>()
            .with_try_into::<TestBarBaz>()
            .with_into::<TestMaybeFooBar>()
            .with_into::<TestFoo>()
            .with_into::<TestBar>();

        let index_foo_bar = registry
            .messages
            .registration
            .get_index_or_insert::<TestFooBar>();
        let index_bar_baz = registry
            .messages
            .registration
            .get_index_or_insert::<TestBarBaz>();
        let index_foo_bar_baz = registry
            .messages
            .registration
            .get_index_or_insert::<TestFooBarBaz>();
        let index_maybe_foo_bar = registry
            .messages
            .registration
            .get_index_or_insert::<TestMaybeFooBar>();
        let index_foo = registry
            .messages
            .registration
            .get_index_or_insert::<TestFoo>();
        let index_bar = registry
            .messages
            .registration
            .get_index_or_insert::<TestBar>();

        let ops = registry.messages.registration.get_or_insert::<TestFooBar>().operations.as_ref().unwrap();
        assert!(ops.from_impls.contains_key(&index_foo_bar_baz));
        assert!(ops.try_from_impls.contains_key(&index_maybe_foo_bar));
        assert!(ops.try_from_impls.contains_key(&index_bar_baz));
        assert!(ops.try_into_impls.contains_key(&index_bar_baz));
        assert!(ops.into_impls.contains_key(&index_maybe_foo_bar));
        assert!(ops.into_impls.contains_key(&index_foo));
        assert!(ops.into_impls.contains_key(&index_bar));
        assert_eq!(ops.into_impls.len(), 3);

        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestFooBarBaz>()
            .operations
            .as_ref()
            .unwrap()
            .into_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestMaybeFooBar>()
            .operations
            .as_ref()
            .unwrap()
            .try_into_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestBarBaz>()
            .operations
            .as_ref()
            .unwrap()
            .try_into_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestBarBaz>()
            .operations
            .as_ref()
            .unwrap()
            .try_from_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestMaybeFooBar>()
            .operations
            .as_ref()
            .unwrap()
            .from_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestFoo>()
            .operations
            .as_ref()
            .unwrap()
            .from_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestBar>()
            .operations
            .as_ref()
            .unwrap()
            .from_impls
            .contains_key(&index_foo_bar)
        );

        let metadata = registry.metadata();
        let ops = metadata.messages.get(index_foo_bar).unwrap().operations.as_ref().unwrap();
        assert!(ops.from.contains(&index_foo_bar_baz));
        assert!(ops.try_from.contains(&index_maybe_foo_bar));
        assert!(ops.try_from.contains(&index_bar_baz));
        assert!(ops.try_into.contains(&index_bar_baz));
        assert!(ops.into.contains(&index_maybe_foo_bar));
        assert!(ops.into.contains(&index_foo));
        assert!(ops.into.contains(&index_bar));
        assert_eq!(ops.into.len(), 3);

        assert!(
            metadata
            .messages
            .get(index_foo_bar_baz)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .into
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages
            .get(index_maybe_foo_bar)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .try_into
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages
            .get(index_bar_baz)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .try_into
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages
            .get(index_bar_baz)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .try_from
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages
            .get(index_maybe_foo_bar)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .from
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages
            .get(index_foo)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .from
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages
            .get(index_bar)
            .unwrap()
            .operations
            .as_ref()
            .unwrap()
            .from
            .contains(&index_foo_bar)
        );
    }
}
