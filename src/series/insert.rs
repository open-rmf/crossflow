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

use bevy_ecs::prelude::{Bundle, Component, Entity};

use crate::{
    Executable, Input, InputBundle, ManageInput, ManageSession, OperationRequest, OperationResult,
    OperationSetup, OrBroken, SeriesLifecycle,
};

#[derive(Component)]
pub(crate) struct Insert<T> {
    target: Entity,
    _ignore: std::marker::PhantomData<fn(T)>,
}

impl<T> Insert<T> {
    pub(crate) fn new(target: Entity) -> Self {
        Self {
            target,
            _ignore: Default::default(),
        }
    }
}

impl<T: 'static + Send + Sync + Bundle> Executable for Insert<T> {
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        let lifecycle = SeriesLifecycle::new(source, world);
        world
            .entity_mut(source)
            .insert((InputBundle::<T>::new(), self, lifecycle));
        Ok(())
    }

    fn execute(OperationRequest { source, world, .. }: OperationRequest) -> OperationResult {
        let Input { data, session, .. } = world.take_input::<T>(source)?;
        let target = world.get::<Insert<T>>(source).or_broken()?.target;
        if let Ok(mut target_mut) = world.get_entity_mut(target) {
            target_mut.insert(data);
        }

        world.despawn_session(session);
        Ok(())
    }
}
