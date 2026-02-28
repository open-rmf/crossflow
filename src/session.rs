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

use bevy_ecs::prelude::{World, Entity};

use crate::{RequestId};

#[cfg(feature = "trace")]
use crate::SessionEvent;

pub trait ManageSession {
    fn spawn_series_session(&mut self) -> Entity;

    fn spawn_scoped_session(&mut self, scope_request: RequestId) -> Entity;

    fn despawn_session(&mut self, entity: Entity);
}

impl ManageSession for World {
    fn spawn_series_session(&mut self) -> Entity {

    }

    fn spawn_scoped_session(&mut self, scope_request: RequestId) -> Entity {

    }

    fn despawn_session(&mut self, session: Entity) {
        if !self.get_entity(session).is_ok() {
            // This session has somehow been despawned already.
            return;
        }

        #[cfg(feature = "trace")]
        {
            SessionEvent::despawned(session, self);
        }

        self.despawn(session);
    }
}
