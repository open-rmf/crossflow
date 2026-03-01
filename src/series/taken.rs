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

use bevy_ecs::prelude::Component;

use tokio::sync::mpsc::UnboundedSender as Sender;

use crate::{
    Executable, Input, InputBundle, ManageInput, Cancellable,
    OperationRequest, OperationResult, OperationSetup, OrBroken, SeriesLifecycleChannel,
    promise::private::Sender as PromiseSender, ManageSession, Cancel,
};

#[derive(Component)]
pub(crate) struct TakenResponse<T> {
    sender: PromiseSender<T>,
}

impl<T> TakenResponse<T> {
    pub(crate) fn new(sender: PromiseSender<T>) -> Self {
        Self { sender }
    }
}

impl<T: 'static + Send + Sync> Executable for TakenResponse<T> {
    fn setup(mut self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        let lifecycle_sender = world
            .get_resource_or_insert_with(SeriesLifecycleChannel::default)
            .sender
            .clone();
        self.sender.on_promise_drop(move || {
            lifecycle_sender.send(source).ok();
        });

        world.entity_mut(source).insert((
            InputBundle::<T>::new(),
            Cancellable::new(cancel_taken_target::<T>),
            self,
        ));
        Ok(())
    }

    fn execute(OperationRequest { source, world, .. }: OperationRequest) -> OperationResult {
        let Input { data, session, .. } = world.take_input::<T>(source)?;
        let sender = world
            .get_entity_mut(source)
            .or_broken()?
            .take::<TakenResponse<T>>()
            .or_broken()?
            .sender;
        sender.send(data).ok();

        world.despawn_session(session);
        Ok(())
    }
}

#[derive(Component)]
pub(crate) struct TakenStream<T> {
    sender: Sender<T>,
}

impl<T> TakenStream<T> {
    pub fn new(sender: Sender<T>) -> Self {
        Self { sender }
    }
}

impl<T: 'static + Send + Sync> Executable for TakenStream<T> {
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        world
            .entity_mut(source)
            .insert((InputBundle::<T>::new(), self));
        Ok(())
    }

    fn execute(OperationRequest { source, world, .. }: OperationRequest) -> OperationResult {
        let Input { data, .. } = world.take_input::<T>(source)?;
        let source_ref = world.get_entity(source).or_broken()?;
        let stream = source_ref.get::<TakenStream<T>>().or_broken()?;
        stream.sender.send(data).ok();
        Ok(())
    }
}

fn cancel_taken_target<T>(Cancel { target, cancellation, world, .. }: Cancel) -> OperationResult
where
    T: 'static + Send + Sync,
{
    let mut target_mut = world.get_entity_mut(target).or_broken()?;
    let taken = target_mut.take::<TakenResponse<T>>().or_broken()?;
    taken.sender.cancel(cancellation).ok();

    Ok(())
}
