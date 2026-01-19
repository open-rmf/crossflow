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
    Cancellation, Executable, Input, InputBundle, ManageInput, OnTerminalCancelled,
    OperationCancel, OperationRequest, OperationResult, OperationSetup, OrBroken,
    SeriesLifecycleChannel, async_execution::spawn_task,
};

pub(crate) struct CaptureOutcome<T> {
    value: oneshot::Sender<Result<T, Cancellation>>,
    finished: oneshot::Receiver<()>,
}

#[derive(Component)]
struct OutcomeSenderStorage<T>(oneshot::Sender<Result<T, Cancellation>>);

impl<T> CaptureOutcome<T> {
    pub(crate) fn new(
        sender: oneshot::Sender<Result<T, Cancellation>>,
        finished: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            value: sender,
            finished,
        }
    }
}

impl<T: 'static + Send + Sync> Executable for CaptureOutcome<T> {
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        let lifecycle_sender = world
            .get_resource_or_insert_with(SeriesLifecycleChannel::default)
            .sender
            .clone();

        let finished = self.finished;
        let monitor_finish = async move {
            match finished.await {
                Ok(_) => {
                    // The outcome was successfully received, there is no action
                    // to take. We do nothing and let this future end.
                }
                Err(_) => {
                    // The Outcome instance was dropped before its result could
                    // be received. We alert the lifecycle manager so it can
                    // drop the series that this outcome depends on.
                    let _ = lifecycle_sender.send(source);
                }
            }
        };

        spawn_task(monitor_finish, world).detach();

        world.entity_mut(source).insert((
            InputBundle::<T>::new(),
            OnTerminalCancelled(cancel_recv_target::<T>),
            OutcomeSenderStorage(self.value),
        ));
        Ok(())
    }

    fn execute(OperationRequest { source, world, .. }: OperationRequest) -> OperationResult {
        let mut source_mut = world.get_entity_mut(source).or_broken()?;
        let Input { data, .. } = source_mut.take_input::<T>()?;
        let sender = source_mut.take::<OutcomeSenderStorage<T>>().or_broken()?.0;
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
    let sender = target_mut.take::<OutcomeSenderStorage<T>>().or_broken()?.0;
    let _ = sender.send(Err(cancel.cancellation));
    target_mut.despawn();

    Ok(())
}
