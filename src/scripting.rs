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

use crate::{
    JsonMessage, IdentifierRef, AnyBuffer, Joined, Accessor,
    Accessing, BufferKeyBuilder, OperationResult, BufferMap, BufferMapStruct,
    BufferMapLayout, IncompatibleLayout, MessageTypeHintMap, BufferMapLayoutHints,
    BufferKeyMap, AccessError, RequestId, BufferError, AwaitingHandle,
};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use std::{
    collections::HashSet,
    sync::Arc,
};
use bevy_ecs::prelude::{Entity, World};

/// This is a message type designed to be passed in and out of scripting
/// environments, such as Python bindings or CEL operations.
#[derive(Debug, Default, Clone, Joined, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ScriptMessage {
    pub data: JsonMessage,
    #[serde(skip)]
    pub accessors: BufferKeyMap,
}

impl From<JsonMessage> for ScriptMessage {
    fn from(data: JsonMessage) -> Self {
        Self {
            data,
            accessors: Default::default(),
        }
    }
}

impl From<BufferKeyMap> for ScriptMessage {
    fn from(accessors: BufferKeyMap) -> Self {
        Self {
            accessors,
            data: Default::default(),
        }
    }
}

impl From<(JsonMessage, BufferKeyMap)> for ScriptMessage {
    fn from((data, accessors): (JsonMessage, BufferKeyMap)) -> Self {
        Self { data, accessors }
    }
}

impl Accessor for ScriptMessage {
    type Buffers = ScriptBuffers;

    fn to_any_keys(&self) -> BufferKeyMap {
        self.accessors.clone()
    }

    fn try_from_any_keys(keys: &BufferKeyMap) -> Result<Self, IncompatibleLayout> {
        Ok(keys.clone().into())
    }

    async fn wait_for_change(&mut self) {
        self.accessors.wait_for_change().await
    }

    type Seen = <BufferKeyMap as Accessor>::Seen;
    fn seen(&mut self, seen: Self::Seen) {
        self.accessors.seen(seen);
    }

    fn make_seen(&self, world: &mut World) -> Self::Seen {
        self.accessors.make_seen(world)
    }

    fn is_disjoint(&self) -> Result<(), crate::OverlapError> {
        self.accessors.is_disjoint()
    }

    fn can_join(&self, world: &World) -> Result<bool, crate::AccessError> {
        self.accessors.can_join(world)
    }

    fn notify_awaiting(
        &self,
        req: RequestId,
        handles: &mut Vec<Arc<AwaitingHandle>>,
        world: &mut World,
    ) {
        self.accessors.notify_awaiting(req, handles, world);
    }

    type Joined = <BufferKeyMap as Accessor>::Joined;
    fn join(&self, req: RequestId, world: &mut World) -> Result<Option<Self::Joined>, AccessError> {
        self.accessors.join(req, world)
    }

    fn distribute(
        &self,
        value: Self::Joined,
        req: RequestId,
        world: &mut World,
    ) -> Result<(), AccessError>
    {
        self.accessors.distribute(value, req, world)
    }

    type View<'a> = <BufferKeyMap as Accessor>::View<'a>;
    fn view<'a>(
        &self,
        req: RequestId,
        world: &'a mut World,
    ) -> Result<Self::View<'a>, crate::BufferError> {
        self.accessors.view(req, world)
    }

    fn view_untraced<'a>(&self, world: &'a World) -> Result<Self::View<'a>, BufferError> {
        self.accessors.view_untraced(world)
    }

    type Access<'w, 's, 'a> = <BufferKeyMap as Accessor>::Access<'w, 's, 'a>;
    fn access<U>(
        &self,
        req: RequestId,
        world: &mut World,
        f: impl FnOnce(Self::Access<'_, '_, '_>) -> U,
    ) -> Result<U, AccessError>
    {
        self.accessors.access(req, world, f)
    }
}

/// This is a minimal wrapper around [`BufferMap`] that allows it to generate
/// a ScriptMessage via access or listening instead of a HashMap of keys.
#[derive(Debug, Clone)]
pub struct ScriptBuffers(pub BufferMap);

impl BufferMapStruct for ScriptBuffers {
    fn buffer_list(&self) -> smallvec::SmallVec<[AnyBuffer; 8]> {
        self.0.buffer_list()
    }
}

impl BufferMapLayout for ScriptBuffers {
    fn try_from_buffer_map(buffers: &BufferMap) -> Result<Self, IncompatibleLayout> {
        Ok(Self(buffers.clone()))
    }

    fn get_buffer_message_type_hints(
        identifiers: HashSet<IdentifierRef<'static>>,
    ) -> Result<MessageTypeHintMap, IncompatibleLayout> {
        <BufferMap as BufferMapLayout>::get_buffer_message_type_hints(identifiers)
    }

    fn get_layout_hints() -> BufferMapLayoutHints {
        <BufferMap as BufferMapLayout>::get_layout_hints()
    }
}

impl Accessing for ScriptBuffers {
    type Key = ScriptMessage;

    fn create_key(&self, builder: &mut BufferKeyBuilder) -> OperationResult<Self::Key> {
        <BufferMap as Accessing>::create_key(&self.0, builder)
            .map(|accessors| accessors.into())
    }

    fn add_accessor(&self, accessor: Entity, world: &mut World) -> OperationResult {
        <BufferMap as Accessing>::add_accessor(&self.0, accessor, world)
    }

    fn deep_clone_key(key: &Self::Key) -> Self::Key {
        <BufferMap as Accessing>::deep_clone_key(&key.accessors).into()
    }

    fn is_key_in_use(key: &Self::Key) -> bool {
        <BufferMap as Accessing>::is_key_in_use(&key.accessors)
    }
}
