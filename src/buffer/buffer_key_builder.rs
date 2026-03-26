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

use bevy_ecs::prelude::{Entity, Query};

use std::sync::Arc;

use crate::{
    BufferAccessLifecycle, BufferChangeBroadcasters, BufferKeyBody, BufferKeyTag, ChannelSender,
    OperationResult, OrBroken, Seq,
};

pub struct BufferKeyBuilder<'w, 's, 'a> {
    scope: Entity,
    session: Entity,
    accessor: Entity,
    lifecycle: Option<(Seq, ChannelSender, Arc<()>)>,
    broadcasters: &'a mut Query<'w, 's, &'static mut BufferChangeBroadcasters>,
}

impl<'w, 's, 'a> BufferKeyBuilder<'w, 's, 'a> {
    /// Make a [`BufferKeyTag`] that can be given to a [`crate::BufferKey`]-like struct.
    pub fn make_body(&mut self, buffer: Entity) -> OperationResult<BufferKeyBody> {
        let receiver = self
            .broadcasters
            .get_mut(buffer)
            .or_broken()?
            .get_receiver(self.session);
        let body = BufferKeyBody {
            tag: BufferKeyTag {
                buffer,
                session: self.session,
                accessor: self.accessor,
            },
            lifecycle: self.lifecycle.as_ref().map(|(seq, sender, tracker)| {
                Arc::new(BufferAccessLifecycle::new(
                    self.scope,
                    buffer,
                    self.session,
                    *seq,
                    self.accessor,
                    sender.clone(),
                    tracker.clone(),
                ))
            }),
            receiver,
        };

        Ok(body)
    }

    pub(crate) fn with_tracking(
        scope: Entity,
        session: Entity,
        accessor: Entity,
        seq: Seq,
        sender: ChannelSender,
        tracker: Arc<()>,
        broadcasters: &'a mut Query<'w, 's, &'static mut BufferChangeBroadcasters>,
    ) -> Self {
        Self {
            scope,
            session,
            accessor,
            lifecycle: Some((seq, sender, tracker)),
            broadcasters,
        }
    }

    pub(crate) fn without_tracking(
        scope: Entity,
        session: Entity,
        accessor: Entity,
        broadcasters: &'a mut Query<'w, 's, &'static mut BufferChangeBroadcasters>,
    ) -> Self {
        Self {
            scope,
            session,
            accessor,
            lifecycle: None,
            broadcasters,
        }
    }
}
