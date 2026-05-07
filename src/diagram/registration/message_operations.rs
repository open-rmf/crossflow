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

use std::{
    any::Any,
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub use crate::dyn_node::*;
use crate::{
    AnyBuffer, AsAnyBuffer, BufferAccessMetadata, BufferAccessRegistration, BufferMapLayoutHints,
    BufferSettings, Builder, JoinRegistration, ListenRegistration, SplitRegistration,
};

use super::*;

pub(crate) type DeserializeNodeFn = fn(&mut Builder) -> Result<DynForkResult, DiagramErrorCode>;
pub(crate) type DeserializeFn<T> = fn(&JsonMessage) -> Result<T, String>;
pub(crate) type SerializeNodeFn = fn(&mut Builder) -> Result<DynForkResult, DiagramErrorCode>;
pub(crate) type SerializeFn<T> = fn(T) -> Result<JsonMessage, String>;
pub(crate) type ForkCloneFn = fn(&mut Builder) -> Result<DynForkClone, DiagramErrorCode>;
pub(crate) type CreateBufferFn = fn(BufferSettings, &mut Builder) -> AnyBuffer;
pub(crate) type CreateTriggerFn = fn(&mut Builder) -> DynNode;
pub(crate) type CreateIntoFn =
    Arc<dyn Fn(&mut Builder) -> (DynInputSlot, DynOutput) + 'static + Send + Sync>;
pub(crate) type CreateTryIntoFn =
    Arc<dyn Fn(&mut Builder) -> DynForkResult + 'static + Send + Sync>;
pub(crate) type ToStringFn = fn(&mut Builder) -> DynNode;
pub(crate) type ArcAny = Arc<dyn Any + 'static + Send + Sync>;

pub(crate) struct Serialization {
    pub(crate) into_json_message: SerializeNodeFn,
    pub(crate) serialize: ArcAny,
}

pub(crate) struct Deserialization {
    pub(crate) create_node: DeserializeNodeFn,
    pub(crate) deserialize: ArcAny,
}

pub struct MessageOperations {
    pub(crate) deserialize: Option<Deserialization>,
    pub(crate) serialize: Option<Serialization>,
    pub(crate) fork_clone: Option<ForkCloneFn>,
    pub(crate) unzip: Option<UnzipRegistration>,
    pub(crate) fork_result: Option<ForkResultRegistration>,
    pub(crate) split: Option<SplitRegistration>,
    pub(crate) join: Option<JoinRegistration>,
    pub(crate) buffer_access: Option<BufferAccessRegistration>,
    pub(crate) listen: Option<ListenRegistration>,
    pub(crate) to_string: Option<ToStringFn>,
    pub(crate) create_buffer_impl: CreateBufferFn,
    pub(crate) create_trigger_impl: CreateTriggerFn,
    pub(crate) into_impls: HashMap<usize, CreateIntoFn>,
    pub(crate) from_impls: HashMap<usize, CreateIntoFn>,
    pub(crate) try_into_impls: HashMap<usize, CreateTryIntoFn>,
    pub(crate) try_from_impls: HashMap<usize, CreateTryIntoFn>,
    pub(crate) build_scope: BuildScope,

    #[cfg(feature = "trace")]
    pub(crate) enable_trace_serialization: Option<EnableTraceSerializeFn>,
}

impl MessageOperations {
    pub fn new<T>() -> Self
    where
        T: Send + Sync + 'static + Any,
    {
        Self {
            deserialize: None,
            serialize: None,
            fork_clone: None,
            unzip: None,
            fork_result: None,
            split: None,
            join: None,
            buffer_access: None,
            listen: None,
            to_string: None,
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

    pub fn metadata(&self, registrations: &MessageRegistrations) -> MessageOperationsMetadata {
        MessageOperationsMetadata::new(self, registrations)
    }

    pub fn supports_into_script_message(&self, registrations: &MessageRegistrations) -> bool {
        if let Ok(script_idx) = registrations.get_index_dyn(&TypeInfo::of::<ScriptMessage>()) {
            if self.into_impls.contains_key(&script_idx) {
                return true;
            }

            if self.try_into_impls.contains_key(&script_idx) {
                return true;
            }
        }

        if let Some(access) = &self.buffer_access {
            let req = access.metadata.request_message;
            if let Ok(req) = registrations.get_by_index(req) {
                if let Some(req_ops) = &req.operations {
                    if req_ops.serialize.is_some() {
                        // This is a buffer accessing message and its incoming
                        // request message type is serializable, so we can turn
                        // this message into a script message.
                        return true;
                    }
                }
            }
        }

        if self.listen.is_some() {
            // If this is a purely listening type then that means it's made
            // entirely of buffer keys, so it can be turned into a script message.
            return true;
        }

        if self.serialize.is_some() {
            // If this message is serializable, then it can be turned into a
            // script message.
            return true;
        }

        false
    }

    pub fn supports_from_script_message(&self, registrations: &MessageRegistrations) -> bool {
        if let Ok(script_idx) = registrations.get_index_dyn(&TypeInfo::of::<ScriptMessage>()) {
            if self.from_impls.contains_key(&script_idx) {
                return true;
            }

            if self.try_from_impls.contains_key(&script_idx) {
                return true;
            }
        }

        if let Some(access) = &self.buffer_access {
            let req = access.metadata.request_message;
            if let Ok(req) = registrations.get_by_index(req) {
                if let Some(req_ops) = &req.operations {
                    if req_ops.deserialize.is_some() {
                        // This is a buffer accessing message and its incoming
                        // request message type is serializable, so we can turn
                        // this message into a script message.
                        return true;
                    }
                }
            }
        }

        if self.listen.is_some() {
            // If this is a purely listening type then that means it's made
            // entirely of buffer keys, so we can attempt to create it from a
            // script message.
            return true;
        }

        if self.deserialize.is_some() {
            // If this message is deserializable, then we can attempt to create
            // it from a script message.
            return true;
        }

        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageOperationsMetadata {
    deserialize: Option<JsEmptyObject>,
    serialize: Option<JsEmptyObject>,
    into_script_message: Option<JsEmptyObject>,
    from_script_message: Option<JsEmptyObject>,
    fork_clone: Option<JsEmptyObject>,
    unzip: Option<Vec<usize>>,
    fork_result: Option<[usize; 2]>,
    split: Option<usize>,
    join: Option<BufferMapLayoutHints<usize>>,
    buffer_access: Option<BufferAccessMetadata>,
    listen: Option<BufferMapLayoutHints<usize>>,
    into: HashSet<usize>,
    try_into: HashSet<usize>,
    from: HashSet<usize>,
    try_from: HashSet<usize>,
}

impl MessageOperationsMetadata {
    fn new(ops: &MessageOperations, registrations: &MessageRegistrations) -> Self {
        let into_script_message = ops.supports_into_script_message(registrations).then(|| JsEmptyObject);
        let from_script_message = ops.supports_from_script_message(registrations).then(|| JsEmptyObject);

        Self {
            deserialize: ops.deserialize.is_some().then(|| JsEmptyObject),
            serialize: ops.serialize.is_some().then(|| JsEmptyObject),
            into_script_message,
            from_script_message,
            fork_clone: ops.fork_clone.is_some().then(|| JsEmptyObject),
            unzip: ops.unzip.as_ref().map(|unzip| unzip.output_types.clone()),
            fork_result: ops.fork_result.as_ref().map(|r| r.output_types),
            split: ops.split.as_ref().map(|op| op.output_type),
            join: ops.join.as_ref().map(|op| op.layout.clone()),
            buffer_access: ops.buffer_access.as_ref().map(|op| op.metadata.clone()),
            listen: ops.listen.as_ref().map(|op| op.layout.clone()),
            into: ops.into_impls.keys().copied().collect(),
            try_into: ops.try_into_impls.keys().copied().collect(),
            from: ops.from_impls.keys().copied().collect(),
            try_from: ops.try_from_impls.keys().copied().collect(),
        }
    }
}

impl MessageOperationsMetadata {
    pub fn can_deserialize(&self) -> bool {
        self.deserialize.is_some()
    }

    pub fn can_serialize(&self) -> bool {
        self.serialize.is_some()
    }

    pub fn into_script_message(&self) -> bool {
        self.into_script_message.is_some()
    }

    pub fn from_script_message(&self) -> bool {
        self.from_script_message.is_some()
    }

    pub fn can_fork_clone(&self) -> bool {
        self.fork_clone.is_some()
    }

    pub fn unzip(&self) -> &Option<Vec<usize>> {
        &self.unzip
    }

    pub fn fork_result(&self) -> &Option<[usize; 2]> {
        &self.fork_result
    }

    pub fn split_output(&self) -> &Option<usize> {
        &self.split
    }

    pub fn join(&self) -> &Option<BufferMapLayoutHints<usize>> {
        &self.join
    }

    pub fn buffer_access(&self) -> &Option<BufferAccessMetadata> {
        &self.buffer_access
    }

    pub fn listen(&self) -> &Option<BufferMapLayoutHints<usize>> {
        &self.listen
    }

    pub fn into_messages(&self) -> &HashSet<usize> {
        &self.into
    }

    pub fn try_into_messages(&self) -> &HashSet<usize> {
        &self.try_into
    }

    pub fn from_messages(&self) -> &HashSet<usize> {
        &self.from
    }

    pub fn try_from_messages(&self) -> &HashSet<usize> {
        &self.try_from
    }
}

/// Represents an empty js object.
///
/// ```json
/// { "type": "object" }
/// ```
#[derive(Clone)]
pub struct JsEmptyObject;

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
