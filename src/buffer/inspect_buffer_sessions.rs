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

use bevy_ecs::{
    prelude::Entity,
    world::{EntityRef, World},
};

use smallvec::SmallVec;

use crate::{
    Broken, BufferChangeBroadcasters, BufferInstanceId, BufferStorage, OperationError,
    OperationResult, OrBroken, Seq, UnhandledErrors,
};

pub trait InspectBufferSessions {
    fn buffered_count<T: 'static + Send + Sync>(
        &self,
        session: Entity,
    ) -> Result<usize, OperationError>;

    fn buffered_sessions<T: 'static + Send + Sync>(
        &self,
    ) -> Result<SmallVec<[Entity; 16]>, OperationError>;
}

impl<'w> InspectBufferSessions for EntityRef<'w> {
    fn buffered_count<T: 'static + Send + Sync>(
        &self,
        session: Entity,
    ) -> Result<usize, OperationError> {
        let buffer = self.get::<BufferStorage<T>>().or_broken()?;
        Ok(buffer.count(session))
    }

    fn buffered_sessions<T: 'static + Send + Sync>(
        &self,
    ) -> Result<SmallVec<[Entity; 16]>, OperationError> {
        let sessions = self
            .get::<BufferStorage<T>>()
            .or_broken()?
            .active_sessions();

        Ok(sessions)
    }
}

pub trait ManageBufferSessions {
    fn remove_buffer_session<T: 'static + Send + Sync>(
        &mut self,
        id: BufferInstanceId,
    ) -> OperationResult;
    fn ensure_buffer_session<T: 'static + Send + Sync>(
        &mut self,
        id: BufferInstanceId,
    ) -> OperationResult;
    fn get_buffer_seen(&mut self, id: BufferInstanceId) -> Seq;
}

impl ManageBufferSessions for World {
    fn remove_buffer_session<T: 'static + Send + Sync>(
        &mut self,
        BufferInstanceId { buffer, session }: BufferInstanceId,
    ) -> OperationResult {
        self.get_mut::<BufferStorage<T>>(buffer)
            .or_broken()?
            .remove_session(session);
        self.get_mut::<BufferChangeBroadcasters>(buffer)
            .or_broken()?
            .remove(&session);
        Ok(())
    }

    fn ensure_buffer_session<T: 'static + Send + Sync>(
        &mut self,
        BufferInstanceId { buffer, session }: BufferInstanceId,
    ) -> OperationResult {
        self.get_mut::<BufferStorage<T>>(buffer)
            .or_broken()?
            .ensure_session(session);
        Ok(())
    }

    fn get_buffer_seen(&mut self, BufferInstanceId { buffer, session }: BufferInstanceId) -> Seq {
        match get_buffer_seen(buffer, session, self) {
            Ok(seq) => seq,
            Err(err) => {
                let backtrace = match err {
                    OperationError::Broken(broken) => broken,
                    OperationError::NotReady => Some(backtrace::Backtrace::new()),
                };

                self.get_resource_or_init::<UnhandledErrors>()
                    .broken
                    .push(Broken {
                        node: buffer,
                        backtrace,
                    });

                // Default to a value of zero, effectively meaning that no update
                // has ever been seen
                0
            }
        }
    }
}

fn get_buffer_seen(
    buffer: Entity,
    session: Entity,
    buffer_mut: &mut World,
) -> OperationResult<Seq> {
    Ok(buffer_mut
        .get_mut::<BufferChangeBroadcasters>(buffer)
        .or_broken()?
        .get_seen(session))
}
