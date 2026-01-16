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

use bevy_ecs::prelude::Component;

use tokio::sync::oneshot;

use crate::{
    Cancellation,
    Executable, Input, InputBundle, ManageInput, OnTerminalCancelled, OperationCancel,
    OperationRequest, OperationResult, OperationSetup, OrBroken, SeriesLifecycleChannel,
    async_execution::spawn_task,
};

#[derive(Component)]
pub(crate) struct CaptureOutcome<T> {
    sender: oneshot::Sender<Result<T, Cancellation>>,
}

impl<T> CaptureOutcome<T> {
    pub(crate) fn new(sender: oneshot::Sender<Result<T, Cancellation>>) -> Self {
        Self { sender }
    }
}

impl<T: 'static + Send + Sync> Executable for CaptureOutcome<T> {
    fn setup(mut self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        let lifecycle_sender = world
            .get_resource_or_insert_with(SeriesLifecycleChannel::default)
            .sender
            .clone();

        let (inner_sender, inner_receiver) = oneshot::channel();

        let mut outer_sender = self.sender;
        self.sender = inner_sender;

        let bridge_channels = async move {
            tokio::select! {
                _ = outer_sender.closed() => {
                    lifecycle_sender.send(source).ok();
                }
                value = inner_receiver => {
                    if let Ok(value) = value {
                        let _ = outer_sender.send(value);
                    } else {
                        // Just let the sender drop. The receiver will get a
                        // RecvError.
                    }
                }
            }
        };

        spawn_task(bridge_channels, world).detach();

        world.entity_mut(source).insert((
            InputBundle::<T>::new(),
            OnTerminalCancelled(cancel_recv_target::<T>),
            self
        ));
        Ok(())
    }

    fn execute(OperationRequest { source, world, .. }: OperationRequest) -> OperationResult {
        let mut source_mut = world.get_entity_mut(source).or_broken()?;
        let Input { data, .. } = source_mut.take_input::<T>()?;
        let sender = source_mut.take::<CaptureOutcome<T>>().or_broken()?.sender;
        sender.send(Ok(data)).ok();
        source_mut.despawn();

        Ok(())
    }
}

fn cancel_recv_target<T>(OperationCancel { cancel, world, .. }: OperationCancel) -> OperationResult
where
    T: 'static + Send + Sync,
{
    let mut target_mut = world.get_entity_mut(cancel.target).or_broken()?;
    let sender = target_mut.take::<CaptureOutcome<T>>().or_broken()?.sender;
    let _ = sender.send(Err(cancel.cancellation));
    target_mut.despawn();

    Ok(())
}
