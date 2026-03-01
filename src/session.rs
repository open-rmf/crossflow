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

use crate::{ScopedSessionBundle, SeriesSessionBundle, Seq};

#[cfg(feature = "trace")]
use crate::{SessionEvent, RequestId};

pub trait ManageSession {
    /// Spawn a session that will be used by a series
    fn spawn_series_session(&mut self) -> Entity;

    /// Spawn a session that will be used inside a scope
    fn spawn_scoped_session(
        &mut self,
        parent_session: Entity,
        scope: Entity,
        seq: Seq,
    ) -> Entity;

    /// Spawn a session that will be used for the buffer cleanup workflow of a
    /// scope.
    fn spawn_cleanup_session(
        &mut self,
        parent_session: Entity,
        begin_cleanup: Entity,
        seq: Seq,
    ) -> Entity;

    fn despawn_session(&mut self, entity: Entity);
}

impl ManageSession for World {
    fn spawn_series_session(&mut self) -> Entity {
        let series_session = self.spawn(SeriesSessionBundle::new()).id();
        #[cfg(feature = "trace")]
        {
            SessionEvent::spawned(None, series_session, self);
        }
        series_session
    }

    fn spawn_scoped_session(
        &mut self,
        parent_session: Entity,
        scope: Entity,
        seq: Seq,
    ) -> Entity {
        let scoped_session = self.spawn(ScopedSessionBundle::new(parent_session, scope)).id();
        #[cfg(feature = "trace")]
        {
            let scope_request = RequestId {
                session: parent_session,
                source: scope,
                seq,
            };
            SessionEvent::spawned(Some(scope_request), scoped_session, self);
        }

        scoped_session
    }

    fn spawn_cleanup_session(
        &mut self,
        parent_session: Entity,
        begin_cleanup: Entity,
        seq: Seq,
    ) -> Entity {
        let cleanup_session = self.spawn(ScopedSessionBundle::for_cleanup(parent_session, begin_cleanup)).id();
        #[cfg(feature = "trace")]
        {
            let scope_request = RequestId {
                session: parent_session,
                source: begin_cleanup,
                seq,
            };
            SessionEvent::spawned(Some(scope_request), cleanup_session, self);
        }

        cleanup_session
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
