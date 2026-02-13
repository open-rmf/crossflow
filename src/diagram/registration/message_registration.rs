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

use std::{any::Any, borrow::Cow, collections::HashMap, marker::PhantomData, sync::Arc};

use bevy_ecs::prelude::{Commands, Entity};

pub use crate::dyn_node::*;
use crate::{
    Accessor, AnyBuffer, BufferAccessRegistration, BufferMap, BufferSettings, Builder,
    IncrementalScopeBuilder, IncrementalScopeRequest, IncrementalScopeResponse, JoinRegistration,
    Joined, JsonMessage, ListenRegistration,
};

use serde_with::serde_as;

use super::*;

pub struct MessageRegistrationBuilder<'a, Message> {
    pub(super) data: &'a mut MessageRegistry,
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
        self.data
            .registration
            .get_or_insert_operations::<Message>()
            .into_impls
            .insert(u_index, Arc::clone(&mapping));

        let message_index = self.data.registration.get_index_or_insert::<Message>();
        self.data
            .registration
            .get_or_insert_operations::<U>()
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
            let node =
                builder.create_map_block(move |request| f(request).map_err(|err| err.to_string()));
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
        self.data
            .registration
            .get_or_insert_operations::<Message>()
            .try_into_impls
            .insert(u_index, Arc::clone(&mapping));

        let message_index = self.data.registration.get_index_or_insert::<Message>();
        self.data
            .registration
            .get_or_insert_operations::<U>()
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

pub struct MessageRegistration {
    pub(crate) type_info: TypeInfo,
    pub(crate) schema: Option<Schema>,
    /// We wrap operations in Option because there are some cases where we need
    /// to reference a message type via TypeInfo before it gets registered with
    /// its concrete type information. We can't register operations for a message
    /// type without its concrete type information, so instead we will allocate
    /// an index for it and leave its operations blank until later.
    pub(crate) operations: Option<MessageOperations>,
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
        self.operations.as_ref().ok_or_else(|| {
            DiagramErrorCode::UnregisteredTypes(vec![Cow::Borrowed(self.type_info.type_name)])
        })
    }
}

pub struct MessageRegistry {
    pub registration: MessageRegistrations,
    pub schema_generator: SchemaGenerator,
}

impl MessageRegistry {
    pub(super) fn new() -> Self {
        let mut settings = SchemaSettings::default();
        settings.definitions_path = "#/schemas/".into();

        Self {
            registration: Default::default(),
            schema_generator: SchemaGenerator::new(settings),
        }
    }

    pub(crate) fn get_dyn(
        &self,
        target_type: &TypeInfo,
    ) -> Result<&MessageRegistration, DiagramErrorCode> {
        self.registration.get_dyn(target_type).ok_or_else(|| {
            DiagramErrorCode::UnregisteredTypes(vec![Cow::Borrowed(target_type.type_name)])
        })
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
            .deserialize
            .map(|deserialize| deserialize(builder))
            .transpose()
    }

    /// Register a deserialize function if not already registered, returns true if the new
    /// function is registered.
    pub(crate) fn register_deserialize<T, Deserializer>(&mut self)
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
        ops.serialize
            .map(|serialize| serialize(builder))
            .transpose()
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
    pub(crate) fn register_serialize<T, Serializer>(&mut self)
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
        let create =
            self.get_operations(message_info)?
                .fork_clone
                .ok_or(DiagramErrorCode::NotCloneable(Cow::Borrowed(
                    message_info.type_name,
                )))?;

        create(builder)
    }

    /// Register a fork_clone function if not already registered, returns true if the new
    /// function is registered.
    pub(crate) fn register_clone<T, F>(&mut self) -> bool
    where
        T: Send + Sync + 'static + Any,
        F: RegisterClone<T>,
    {
        let ops = &mut self.registration.get_or_insert_operations::<T>();
        if !F::CLONEABLE || ops.fork_clone.is_some() {
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
            .unzip
            .as_ref()
            .ok_or(DiagramErrorCode::NotUnzippable(Cow::Borrowed(
                message_info.type_name,
            )))
    }

    /// Register a unzip function if not already registered, returns true if the new
    /// function is registered.
    pub(crate) fn register_unzip<T, Serializer, Cloneable>(&mut self) -> bool
    where
        T: Send + Sync + 'static + Any,
        Serializer: 'static,
        Cloneable: 'static,
        Supported<(T, Serializer, Cloneable)>: RegisterUnzip,
    {
        let unzip_impl = Supported::<(T, Serializer, Cloneable)>::register_unzip(self);

        let ops = self.registration.get_or_insert_operations::<T>();
        if ops.unzip.is_some() {
            return false;
        }
        ops.unzip = Some(unzip_impl);

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
            .ok_or(DiagramErrorCode::CannotForkResult(Cow::Borrowed(
                message_info.type_name,
            )))?
            .create;

        create(builder)
    }

    /// Register a fork_result function if not already registered, returns true if the new
    /// function is registered.
    pub(crate) fn register_result<R>(&mut self)
    where
        R: RegisterForkResult,
    {
        R::register_fork_result(self)
    }

    pub fn split(
        &self,
        message_info: &TypeInfo,
        split_op: &SplitSchema,
        builder: &mut Builder,
    ) -> Result<DynSplit, DiagramErrorCode> {
        let create = self
            .get_operations(message_info)?
            .split
            .ok_or(DiagramErrorCode::NotSplittable(Cow::Borrowed(
                message_info.type_name,
            )))?
            .create;

        create(split_op, builder)
    }

    /// Register a split function if not already registered.
    pub(crate) fn register_split<T, S, C>(&mut self)
    where
        T: Send + Sync + 'static + Any,
        Supported<(T, S, C)>: RegisterSplit,
    {
        Supported::<(T, S, C)>::register_split(self);
    }

    pub fn create_buffer(
        &self,
        message_info: &TypeInfo,
        settings: BufferSettings,
        builder: &mut Builder,
    ) -> Result<AnyBuffer, DiagramErrorCode> {
        let f = self.get_operations(message_info)?.create_buffer_impl;

        Ok(f(settings, builder))
    }

    pub(crate) fn set_scope_request(
        &self,
        message_info: &TypeInfo,
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> Result<IncrementalScopeRequest, DiagramErrorCode> {
        let f = self.get_operations(message_info)?.build_scope.set_request;

        f(incremental, commands).map_err(Into::into)
    }

    pub(crate) fn set_scope_response(
        &self,
        message_info: &TypeInfo,
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> Result<IncrementalScopeResponse, DiagramErrorCode> {
        let f = self.get_operations(message_info)?.build_scope.set_response;

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
        let f = self.get_operations(message_info)?.create_trigger_impl;

        Ok(f(builder))
    }

    pub fn join(
        &self,
        joinable: &TypeInfo,
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<DynOutput, DiagramErrorCode> {
        let create = self
            .get_operations(joinable)?
            .join
            .as_ref()
            .ok_or_else(|| DiagramErrorCode::NotJoinable(Cow::Borrowed(joinable.type_name)))?
            .create;

        create(buffers, builder)
    }

    /// Register a join function if not already registered, returns true if the
    /// new function is registered.
    pub(crate) fn register_join<T>(&mut self)
    where
        T: Send + Sync + 'static + Any + Joined,
    {
        let join = JoinRegistration::new::<T>(self);

        self.registration.get_or_insert_operations::<T>().join = Some(join);
    }

    pub fn with_buffer_access(
        &self,
        target_type: &TypeInfo,
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<DynNode, DiagramErrorCode> {
        let create = self
            .get_operations(target_type)?
            .buffer_access
            .as_ref()
            .ok_or(DiagramErrorCode::CannotAccessBuffers(Cow::Borrowed(
                target_type.type_name,
            )))?
            .create;

        create(buffers, builder)
    }

    pub(crate) fn register_buffer_access<T>(&mut self)
    where
        T: Send + Sync + 'static + BufferAccessRequest,
    {
        let buffer_access = BufferAccessRegistration::new::<T>(self);

        self.registration
            .get_or_insert_operations::<T>()
            .buffer_access = Some(buffer_access);
    }

    pub fn listen(
        &self,
        target_type: &TypeInfo,
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<DynOutput, DiagramErrorCode> {
        let create = self
            .get_operations(target_type)?
            .listen
            .as_ref()
            .ok_or_else(|| DiagramErrorCode::CannotListen(Cow::Borrowed(target_type.type_name)))?
            .create;

        create(buffers, builder)
    }

    pub fn get_type_info_for(&self, index: usize) -> Result<TypeInfo, DiagramErrorCode> {
        Ok(self.registration.get_by_index(index)?.type_info)
    }

    pub(crate) fn register_listen<T>(&mut self)
    where
        T: Send + Sync + 'static + Any + Accessor,
    {
        let listen = ListenRegistration::new::<T>(self);

        self.registration.get_or_insert_operations::<T>().listen = Some(listen);
    }

    pub(crate) fn register_to_string<T>(&mut self)
    where
        T: 'static + Send + Sync + ToString,
    {
        let ops = &mut self.registration.get_or_insert_operations::<T>();

        ops.to_string_impl =
            Some(|builder| builder.create_map_block(|msg: T| msg.to_string()).into());
    }

    pub(crate) fn get_operations(
        &self,
        message_info: &TypeInfo,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        self.get_dyn(message_info)?
            .operations
            .as_ref()
            .ok_or_else(|| {
                DiagramErrorCode::UnregisteredTypes(vec![Cow::Borrowed(message_info.type_name)])
            })
    }
}

#[derive(Default)]
pub struct MessageRegistrations {
    messages: Vec<MessageRegistration>,

    /// Convert from type info to the index of a message wihtin the registry
    indices: HashMap<TypeInfo, usize>,

    /// Lookup message types that satisfy some constraint. This is used by
    /// message type inference.
    pub(crate) reverse_lookup: ReverseMessageLookup,
}

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReverseMessageLookup {
    /// Map from [T, E] output registrations to Result<T, E> registration.
    #[serde_as(as = "Vec<(_, _)>")]
    #[schemars(with = "Vec<([usize; 2], usize)>")]
    pub(crate) result: HashMap<[usize; 2], usize>,

    /// Map from the unzipped types to the original zipped type.
    #[serde_as(as = "Vec<(_, _)>")]
    #[schemars(with = "Vec<(Vec<usize>, usize)>")]
    pub(crate) unzip: HashMap<Vec<usize>, usize>,

    /// Map from the message type of the item that comes out of a split to all
    /// message types that can be split into it.
    #[serde_as(as = "Vec<(_, _)>")]
    #[schemars(with = "Vec<(usize, Vec<usize>)>")]
    pub(crate) split: HashMap<usize, Vec<usize>>,

    /// The index where the [`JsonMessage`] type is registered.
    #[serde_as(as = "_")]
    #[schemars(with = "Option<usize>")]
    pub(crate) json_message: Option<usize>,
}

impl MessageRegistrations {
    pub fn iter(&self) -> std::slice::Iter<'_, MessageRegistration> {
        self.messages.iter()
    }

    pub(crate) fn get<T>(&self) -> Option<&MessageRegistration>
    where
        T: Any,
    {
        self.get_dyn(&TypeInfo::of::<T>())
    }

    pub(crate) fn get_dyn(&self, target_type: &TypeInfo) -> Option<&MessageRegistration> {
        self.indices
            .get(target_type)
            .map(|index| self.messages.get(*index))
            .flatten()
    }

    pub(crate) fn get_by_index(
        &self,
        index: usize,
    ) -> Result<&MessageRegistration, DiagramErrorCode> {
        self.messages
            .get(index)
            .ok_or_else(|| DiagramErrorCode::UnknownMessageTypeIndex {
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
            self.new_message_registration(TypeInfo::of::<T>(), MessageRegistration::new::<T>());

            // SAFETY: We just pushed an entry in the previous line, and now we
            // simply want to retrieve a mutable borrow of it.
            self.messages.last_mut().unwrap()
        }
    }

    pub(crate) fn get_or_insert_operations<T>(&mut self) -> &mut MessageOperations
    where
        T: 'static + Send + Sync,
    {
        self.get_or_insert::<T>()
            .operations
            .get_or_insert_with(|| MessageOperations::new::<T>())
    }

    pub(crate) fn get_index_dyn(&self, target_type: &TypeInfo) -> Result<usize, DiagramErrorCode> {
        self.indices.get(target_type).cloned().ok_or_else(|| {
            DiagramErrorCode::UnregisteredTypes(vec![Cow::Borrowed(target_type.type_name)])
        })
    }

    // Used in testing
    #[allow(unused)]
    pub(crate) fn get_index<T>(&self) -> Result<usize, DiagramErrorCode>
    where
        T: 'static + Send + Sync,
    {
        let type_info = TypeInfo::of::<T>();
        self.indices.get(&type_info).cloned().ok_or_else(|| {
            DiagramErrorCode::UnregisteredTypes(vec![Cow::Borrowed(type_info.type_name)])
        })
    }

    pub(crate) fn get_index_or_insert<T>(&mut self) -> usize
    where
        T: 'static + Send + Sync,
    {
        let target_type = TypeInfo::of::<T>();
        if let Some(index) = self.indices.get(&target_type) {
            *index
        } else {
            self.new_message_registration(target_type, MessageRegistration::new::<T>())
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

        if message_info == TypeInfo::of::<JsonMessage>() {
            self.reverse_lookup.json_message = Some(index);
        }

        index
    }

    /// Get the metadata of the registered messages
    pub fn metadata(&self) -> Vec<MessageMetadata> {
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageMetadata {
    type_name: Cow<'static, str>,
    schema: Option<Schema>,
    operations: Option<MessageOperationsMetadata>,
}

impl MessageMetadata {
    pub fn type_name(&self) -> &Cow<'static, str> {
        &self.type_name
    }

    pub fn schema(&self) -> &Option<Schema> {
        &self.schema
    }

    pub fn operations(&self) -> &Option<MessageOperationsMetadata> {
        &self.operations
    }
}
