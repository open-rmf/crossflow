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

use bevy_ecs::prelude::Entity;

use crate::{
    Input, InputBundle, ManageInput, MessageRoute, Operation, OperationCleanup,
    OperationReachability, OperationRequest, OperationResult, OperationSetup, OrBroken,
    ReachabilityResult, SingleInputStorage, SingleTargetStorage, output_port,
};

pub(crate) struct Noop<T> {
    target: Entity,
    _ignore: std::marker::PhantomData<fn(T)>,
}

impl<T> Noop<T> {
    pub(crate) fn new(target: Entity) -> Self {
        Noop {
            target,
            _ignore: Default::default(),
        }
    }
}

impl<T: 'static + Send + Sync> Operation for Noop<T> {
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        world
            .get_entity_mut(self.target)
            .or_broken()?
            .insert(SingleInputStorage::new(source));

        world.entity_mut(source).insert((
            InputBundle::<T>::new(),
            SingleTargetStorage::new(self.target),
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
        let Input {
            session,
            data: value,
            seq,
        } = world.take_input::<T>(source)?;

        let target = world.get::<SingleTargetStorage>(source).or_broken()?.get();
        let port = output_port::next();
        let route = MessageRoute {
            session,
            source,
            seq,
            port: &port,
            target,
        };
        world.give_input(route, value, roster)
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<T>()?;
        clean.notify_cleaned()
    }

    fn is_reachable(mut reachability: OperationReachability) -> ReachabilityResult {
        if reachability.has_input::<T>()? {
            return Ok(true);
        }

        SingleInputStorage::is_reachable(&mut reachability)
    }
}
