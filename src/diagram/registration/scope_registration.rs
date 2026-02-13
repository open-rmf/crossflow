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

use bevy_ecs::prelude::{Commands, Entity};

use crate::{
    DynInputSlot, DynOutput, IncrementalScopeBuilder, IncrementalScopeRequestResult,
    IncrementalScopeResponseResult, NamedStream, StreamOf,
};

pub(crate) struct BuildScope {
    pub(crate) set_request:
        fn(&mut IncrementalScopeBuilder, &mut Commands) -> IncrementalScopeRequestResult,
    pub(crate) set_response:
        fn(&mut IncrementalScopeBuilder, &mut Commands) -> IncrementalScopeResponseResult,
    pub(crate) spawn_basic_scope_stream:
        fn(Entity, Entity, &mut Commands) -> (DynInputSlot, DynOutput),
}

impl BuildScope {
    pub(super) fn new<T: 'static + Send + Sync>() -> Self {
        Self {
            set_request: Self::impl_set_request::<T>,
            set_response: Self::impl_set_response::<T>,
            spawn_basic_scope_stream: Self::impl_spawn_basic_scope_stream::<T>,
        }
    }

    fn impl_set_request<T: 'static + Send + Sync>(
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> IncrementalScopeRequestResult {
        incremental.set_request::<T>(commands)
    }

    fn impl_set_response<T: 'static + Send + Sync>(
        incremental: &mut IncrementalScopeBuilder,
        commands: &mut Commands,
    ) -> IncrementalScopeResponseResult {
        incremental.set_response::<T>(commands)
    }

    fn impl_spawn_basic_scope_stream<T: 'static + Send + Sync>(
        in_scope: Entity,
        out_scope: Entity,
        commands: &mut Commands,
    ) -> (DynInputSlot, DynOutput) {
        let (stream_in, stream_out) =
            NamedStream::<StreamOf<T>>::spawn_scope_stream(in_scope, out_scope, commands);

        (stream_in.into(), stream_out.into())
    }
}
