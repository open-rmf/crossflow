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

use crate::{
    Accessing, BufferAccessStorage, CleanInputsOf, InScope, InSeries, ManageDisposal, ManageInput,
    MiscellaneousFailure, OperationError, OperationResult, OperationRoster, OrBroken, RequestId,
    UnhandledErrors,
};

use bevy_ecs::prelude::{Component, Entity, World};

use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;

use smallvec::SmallVec;

pub struct OperationCleanup<'a> {
    pub source: Entity,
    pub cleanup: Cleanup,
    pub world: &'a mut World,
    pub roster: &'a mut OperationRoster,
}

impl<'a> OperationCleanup<'a> {
    pub fn new(
        cleaner: Entity,
        node: Entity,
        session: Entity,
        cleanup_id: RequestId,
        world: &'a mut World,
        roster: &'a mut OperationRoster,
    ) -> Self {
        let cleanup = Cleanup {
            cleaner,
            node,
            session,
            cleanup_id,
        };
        Self {
            source: node,
            cleanup,
            world,
            roster,
        }
    }

    /// Instruct the operation `node` to clean itself for `session`.
    ///
    /// Returns true/false based on whether the operation has any cleanup
    /// capabilities.
    pub fn clean(&mut self) -> bool {
        let Some(cleanup) = self.world.get::<OnCleanup>(self.source) else {
            return false;
        };

        let cleanup = cleanup.0;
        if let Err(error) = cleanup(OperationCleanup {
            source: self.source,
            cleanup: self.cleanup,
            world: self.world,
            roster: self.roster,
        }) {
            self.world
                .get_resource_or_insert_with(UnhandledErrors::default)
                .operations
                .push(error);
        }

        true
    }

    pub fn cleanup_inputs<T: 'static + Send + Sync>(&mut self) -> OperationResult {
        self.world.cleanup_inputs::<T>(CleanInputsOf {
            session: self.cleanup.session,
            source: self.source,
        });
        Ok(())
    }

    pub fn cleanup_disposals(&mut self) -> OperationResult {
        if self.world.get::<InSeries>(self.source).is_some() {
            // Ignore disposal cleanup for operations that are in a series rather
            // than in a scope. These operations will be dropped as soon as the
            // series is finished.
            return Ok(());
        }

        let scope = self.world.get::<InScope>(self.source).or_broken()?.scope();
        if self.cleanup.cleaner == scope {
            // Only erase disposals if the cleanup is being triggered by the scope
            self.world
                .clear_disposals(self.cleanup.session, self.source);
        }
        Ok(())
    }

    pub fn cleanup_buffer_access<B>(&mut self) -> OperationResult
    where
        B: Accessing + 'static + Send + Sync,
        B::Key: 'static + Send + Sync,
    {
        let scope = self.world.get::<InScope>(self.source).or_broken()?.scope();
        if self.cleanup.cleaner == scope {
            // If the scope is telling us to clean up, then we should fully
            // remove the key for this session. Otherwise we should not remove
            // it because it's important that we can continue to track the keys.
            self.world
                .get_mut::<BufferAccessStorage<B>>(self.source)
                .or_broken()?
                .keys
                .remove(&self.cleanup.session);
        }
        Ok(())
    }

    pub fn notify_cleaned(&mut self) -> OperationResult {
        self.cleanup.notify_cleaned(self.world, self.roster)
    }

    /// Use this to pass the responsibility of cleaning up this node to another
    /// operation node. This is used by async providers to hand off cleanup
    /// responsibilities to their active tasks.
    pub fn delegate_to(mut self, source: Entity) -> Self {
        self.source = source;
        self
    }
}

/// The contents that an operation is willing to clean.
#[derive(Debug, Default, Component)]
pub struct CleanupContents {
    awaiting_cleanup: HashMap<RequestId, SmallVec<[Entity; 16]>>,
}

impl CleanupContents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_cleanup(&mut self, cleanup_id: RequestId, nodes: SmallVec<[Entity; 16]>) {
        self.awaiting_cleanup.insert(cleanup_id, nodes);
    }

    pub fn register_cleanup_of_node(&mut self, cleanup_id: RequestId, node: Entity) -> bool {
        let Some(nodes) = self.awaiting_cleanup.get_mut(&cleanup_id) else {
            return false;
        };
        nodes.retain(|n| *n != node);
        nodes.is_empty()
    }
}

pub struct FinalizeCleanupRequest<'a> {
    pub cleanup: Cleanup,
    pub world: &'a mut World,
    pub roster: &'a mut OperationRoster,
}

#[derive(Component)]
pub(crate) struct OnCleanup(pub(super) fn(OperationCleanup) -> OperationResult);

#[derive(Component, Clone, Copy)]
pub struct FinalizeCleanup(pub(crate) fn(FinalizeCleanupRequest) -> OperationResult);

impl FinalizeCleanup {
    pub fn new(f: fn(FinalizeCleanupRequest) -> OperationResult) -> Self {
        Self(f)
    }
}

/// Notify the scope manager that the request may be finished with cleanup
#[derive(Clone, Copy, Debug)]
pub struct Cleanup {
    /// This is the ID of the scope operation that initiated the cleanup. This
    /// will typically be the trim or terminate operation.
    pub cleaner: Entity,
    /// This is the operation node that the Cleanup request was sent to. The
    /// request might need to move across other operation nodes while it is
    /// being carried out, so we keep track of the original target node here so
    /// that the cleaner can be correctly notified about which node finished
    /// cleaning up.
    pub node: Entity,
    /// This is the session that the node shoudl clean.
    pub session: Entity,
    // A unique ID for this cleanup operation.
    pub cleanup_id: RequestId,
}

impl Cleanup {
    pub(crate) fn notify_cleaned(
        &self,
        world: &mut World,
        roster: &mut OperationRoster,
    ) -> OperationResult {
        let mut cleaner_mut = world.get_entity_mut(self.cleaner).or_broken()?;
        let mut scope_contents = cleaner_mut.get_mut::<CleanupContents>().or_broken()?;
        if scope_contents.register_cleanup_of_node(self.cleanup_id, self.node) {
            roster.cleanup_finished(*self);
            scope_contents.awaiting_cleanup.remove(&self.cleanup_id);
        }
        Ok(())
    }

    pub(crate) fn trigger(self, world: &mut World, roster: &mut OperationRoster) {
        // Clear this cleanup_id so we're not leaking memory
        match world.get_mut::<CleanupContents>(self.cleaner) {
            Some(mut contents) => {
                contents.awaiting_cleanup.remove(&self.cleanup_id);
            }
            None => {
                world
                    .get_resource_or_insert_with(UnhandledErrors::default)
                    .miscellaneous
                    .push(MiscellaneousFailure {
                        error: Arc::new(anyhow!("Failed to clear cleanup tracker: {self:?}")),
                        backtrace: Some(backtrace::Backtrace::new()),
                    });
            }
        }

        let Some(FinalizeCleanup(f)) = world.get::<FinalizeCleanup>(self.cleaner).copied() else {
            return;
        };
        if let Err(OperationError::Broken(backtrace)) = (f)(FinalizeCleanupRequest {
            cleanup: self,
            world,
            roster,
        }) {
            world
                .get_resource_or_insert_with(UnhandledErrors::default)
                .miscellaneous
                .push(MiscellaneousFailure {
                    error: Arc::new(anyhow!("Failed to finalize cleanup: {self:?}")),
                    backtrace,
                })
        }
    }
}
