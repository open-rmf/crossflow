/*
 * Copyright (C) 2024 Open Source Robotics Foundation
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

use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    prelude::{Component, Entity, Query, World},
    system::SystemState,
};

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Weak},
};

use smallvec::SmallVec;

use crate::{
    Accessing, BufferChangeBroadcasters, BufferKeyBuilder, BufferKeyTag, ChannelQueue, InScope,
    Input, InputBundle, ManageInput, MessageRoute, Operation, OperationCleanup, OperationError,
    OperationReachability, OperationRequest, OperationResult, OperationSetup, OrBroken,
    ReachabilityResult, RequestId, Seq, SingleInputStorage, SingleTargetStorage, output_port,
};

#[derive(Default, Component, Deref, DerefMut)]
struct AwaitingFetch(HashMap<Entity, HashMap<RequestId, Weak<AwaitingHandle>>>);

pub struct AwaitingHandle;

pub trait NotifyAwaitingBuffer {
    fn awaiting_buffer(
        &mut self,
        tag: &BufferKeyTag,
        req: RequestId,
    ) -> Option<Arc<AwaitingHandle>>;
}

impl NotifyAwaitingBuffer for World {
    fn awaiting_buffer(
        &mut self,
        tag: &BufferKeyTag,
        req: RequestId,
    ) -> Option<Arc<AwaitingHandle>> {
        let mut awaiting = self.get_mut::<AwaitingFetch>(tag.accessor)?;
        let map = awaiting.entry(tag.session).or_default();

        match map.entry(req) {
            Entry::Vacant(vacant) => {
                let handle = Arc::new(AwaitingHandle);
                vacant.insert(Arc::downgrade(&handle));
                return Some(handle);
            }
            Entry::Occupied(mut occupied) => {
                if let Some(handle) = occupied.get().upgrade() {
                    return Some(handle);
                }

                let handle = Arc::new(AwaitingHandle);
                occupied.insert(Arc::downgrade(&handle));
                return Some(handle);
            }
        }
    }
}

pub(crate) struct OperateBufferAccess<Input, Buffers, Output>
where
    Input: 'static + Send + Sync,
    Buffers: Accessing,
{
    buffers: Buffers,
    target: Entity,
    _ignore: std::marker::PhantomData<fn(Input, Buffers, Output)>,
}

impl<Input, Buffers, Output> OperateBufferAccess<Input, Buffers, Output>
where
    Input: 'static + Send + Sync,
    Buffers: Accessing,
{
    pub(crate) fn new(buffers: Buffers, target: Entity) -> Self {
        Self {
            buffers,
            target,
            _ignore: Default::default(),
        }
    }
}

#[derive(Component)]
pub struct BufferKeyUsage(pub(crate) fn(Entity, Entity, &World) -> ReachabilityResult);

#[derive(Component)]
pub(crate) struct BufferAccessStorage<B: Accessing> {
    pub(crate) buffers: B,
    pub(crate) keys: HashMap<Entity, B::Key>,
}

impl<B: Accessing> BufferAccessStorage<B> {
    pub(crate) fn new(buffers: B) -> Self {
        Self {
            buffers,
            keys: HashMap::new(),
        }
    }
}

impl<InputMessage, Buffers, Output> Operation for OperateBufferAccess<InputMessage, Buffers, Output>
where
    InputMessage: 'static + Send + Sync,
    Buffers: Accessing + 'static + Send + Sync,
    Buffers::Key: 'static + Send + Sync,
    Output: From<(InputMessage, Buffers::Key)> + 'static + Send + Sync,
{
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        world
            .get_entity_mut(self.target)
            .or_broken()?
            .insert(SingleInputStorage::new(source));

        self.buffers.add_accessor(source, world)?;
        world.entity_mut(source).insert((
            InputBundle::<InputMessage>::new(),
            BufferAccessStorage::new(self.buffers),
            SingleTargetStorage::new(self.target),
            BufferKeyUsage(buffer_key_usage::<Buffers>),
            AwaitingFetch::default(),
        ));

        Ok(())
    }

    fn execute(
        OperationRequest {
            source,
            world,
            roster,
        }: OperationRequest,
    ) -> OperationResult {
        let Input { session, data, seq } = world.take_input::<InputMessage>(source)?;
        let keys = get_access_keys::<Buffers>(source, session, seq, world)?;

        let target = world.get::<SingleTargetStorage>(source).or_broken()?.get();

        let port = output_port::next();
        let route = MessageRoute {
            session,
            source,
            seq,
            port: &port,
            target,
        };

        let output = Output::from((data, keys));
        world.give_input(route, output, roster)
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<InputMessage>()?;
        clean.cleanup_buffer_access::<Buffers>()?;

        let mut awaiting = clean
            .world
            .get_mut::<AwaitingFetch>(clean.source)
            .or_broken()?;
        awaiting.remove(&clean.cleanup.session);

        clean.notify_cleaned()
    }

    fn is_reachable(mut r: OperationReachability) -> ReachabilityResult {
        if r.has_input::<InputMessage>()? {
            return Ok(true);
        }

        SingleInputStorage::is_reachable(&mut r)
    }
}

pub(crate) fn get_access_keys<B>(
    source: Entity,
    session: Entity,
    seq: Seq,
    world: &mut World,
) -> Result<B::Key, OperationError>
where
    B: Accessing + 'static + Send + Sync,
    B::Key: 'static + Send + Sync,
{
    let scope = world.get::<InScope>(source).or_broken()?.scope();
    let sender = world
        .get_resource_or_insert_with(ChannelQueue::default)
        .sender
        .clone();

    let mut state: SystemState<(
        Query<&mut BufferAccessStorage<B>>,
        Query<&mut BufferChangeBroadcasters>,
    )> = SystemState::new(world);
    let (mut storages, mut broadcasters) = state.get_mut(world);

    let mut storage = storages.get_mut(source).or_broken()?;
    let s = storage.as_mut();
    let mut made_key = false;
    let keys = match s.keys.entry(session) {
        Entry::Occupied(occupied) => B::deep_clone_key(occupied.get()),
        Entry::Vacant(vacant) => {
            made_key = true;
            let mut builder = BufferKeyBuilder::with_tracking(
                scope,
                session,
                source,
                seq,
                sender,
                Arc::new(()),
                &mut broadcasters,
            );
            let new_key = vacant.insert(s.buffers.create_key(&mut builder)?);
            B::deep_clone_key(new_key)
        }
    };

    if made_key {
        // If we needed to make a new key for this session then we should
        // ensure that the session is active in the buffer before we send
        // off the keys.
        let buffers = s.buffers.clone();
        buffers.ensure_active_session(session, world)?;
    }

    Ok(keys)
}

pub(crate) fn buffer_key_usage<B>(
    accessor: Entity,
    session: Entity,
    world: &World,
) -> ReachabilityResult
where
    B: Accessing + 'static + Send + Sync,
    B::Key: 'static + Send + Sync,
{
    let key = world
        .get::<BufferAccessStorage<B>>(accessor)
        .or_broken()?
        .keys
        .get(&session);
    if let Some(key) = key {
        if B::is_key_in_use(key) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Buffer access nodes are siblings nodes in a workflow which can access the
/// buffer, potentially in a mutable way. Their outputs do not get fed to the
/// buffer, so they are not considered input nodes, but they may modify the
/// contents of the buffer, which includes pushing new data, so they affect
/// the reachability of the buffer.
#[derive(Component, Default)]
pub(crate) struct BufferAccessors(pub(crate) SmallVec<[Entity; 8]>);

impl BufferAccessors {
    pub(crate) fn add_accessor(&mut self, accessor: Entity) {
        self.0.push(accessor);
        self.0.sort();
        self.0.dedup();
    }

    pub(crate) fn is_reachable(r: &mut OperationReachability) -> ReachabilityResult {
        let Some(accessors) = r.world.get::<Self>(r.source) else {
            return Ok(false);
        };

        for accessor in &accessors.0 {
            if let Some(requested_by_accessor) = r.requested_by_accessor {
                // We calculate reachability a bit differently when a buffer
                // accessor is the one requesting it.

                if requested_by_accessor == *accessor {
                    // This is the accessor that is asking about reachability,
                    // so do not count it for reachability.
                    continue;
                }

                // Since an accessor is the one evaluating the reachability, we
                // will also account for other accessors that are awaiting a
                // fetch. This is to prevent a situation where multiple accessors
                // are awaiting the same buffer and none of them will ever place
                // a value in it.
                let awaiting = r.world.get::<AwaitingFetch>(*accessor).or_broken()?;
                if let Some(awaiting_session) = awaiting.get(&r.session) {
                    if awaiting_session.iter().any(|a| a.1.strong_count() > 0) {
                        // At least one key related to this accessor is being used
                        // to await a fetch. Out of an abundance of caution we will
                        // discount this accessor as providing reachability.
                        continue;
                    }
                }
            }

            let usage = r.world.get::<BufferKeyUsage>(*accessor).or_broken()?.0;
            if usage(*accessor, r.session, r.world)? {
                return Ok(true);
            }
        }

        for accessor in &accessors.0 {
            if r.check_upstream(*accessor)? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
