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
    prelude::{Component, Entity, Mut, Commands, Query},
    system::SystemParam,
    query::QueryEntityError,
};

use smallvec::{Drain, SmallVec};

use std::collections::HashMap;

use std::{
    iter::{Rev, Map},
    ops::RangeBounds,
    slice::{Iter, IterMut},
};

use crate::{BufferSettings, RetentionPolicy, InputStorage, Seq, RequestId, BufferKeyTag, BufferView};

#[cfg(feature = "trace")]
use crate::Trace;

#[derive(SystemParam)]
pub(crate) struct BufferMutQuery<'w, 's, T: 'static + Send + Sync> {
    query: Query<'w, 's, (
        &'static mut BufferStorage<T>,
        &'static mut InputStorage<T>,
    )>,
    commands: Commands<'w, 's>,
    #[cfg(feature = "trace")]
    trace: Query<'w, 's, &'static Trace>,
}

impl<'w, 's, T: 'static + Send + Sync> BufferMutQuery<'w, 's, T> {
    pub(crate) fn get_manager<'a>(
        &'a mut self,
        req: RequestId,
        key: &BufferKeyTag,
    ) -> Result<BufferManager<'w, 's, 'a, T>, QueryEntityError> {
        let (storage, input) = self.query.get_mut(key.buffer)?;

        #[cfg(feature = "trace")]
        let trace = self.trace.get(key.buffer).ok();

        Ok(BufferManager {
            storage,
            input,
            req,
            session: key.session,
            commands: &mut self.commands,
            #[cfg(feature = "trace")]
            trace,
        })
    }

    pub(crate) fn get_view<'a>(
        &'a self,
        key: &BufferKeyTag,
    ) -> Result<BufferView<'a, T>, QueryEntityError> {
        let session = key.session;
        self.query
            .get(key.buffer)
            .map(|(storage, _)| BufferView { storage, session })
    }
}

pub(crate) struct BufferManager<'w, 's, 'a, T> {
    storage: Mut<'a, BufferStorage<T>>,
    input: Mut<'a, InputStorage<T>>,
    pub(crate) req: RequestId,
    pub(crate) session: Entity,
    pub(crate) commands: &'a mut Commands<'w, 's>,
    #[cfg(feature = "trace")]
    #[allow(unused)]
    trace: Option<&'a Trace>,
}

impl<'w, 's, 'a, T> BufferManager<'w, 's, 'a, T> {
    pub(crate) fn len(&self) -> usize {
        self.storage.count(self.session)
    }

    pub(crate) fn iter(&self) -> IterBufferView<'_, T>
    where
        T: 'static + Send + Sync,
    {
        self.storage.iter(self.session)
    }

    pub(crate) fn oldest(&self) -> Option<&T> {
        self.storage.oldest(self.session)
    }

    pub(crate) fn newest(&self) -> Option<&T> {
        self.storage.newest(self.session)
    }

    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.storage.get(self.session, index)
    }

    pub(crate) fn force_push(&mut self, value: T) -> Option<T> {
        let seq = self.input.increment_seq();
        let retention = self.storage.settings.retention();
        let removed = Self::impl_push(
            self.storage.reverse_queues.entry(self.session).or_default(),
            retention,
            seq,
            value,
        );

        removed.map(|e| e.message)
    }

    pub(crate) fn push(&mut self, message: T) -> Option<T> {
        let retention = self.storage.settings.retention();
        let Some(reverse_queue) = self.storage.reverse_queues.get_mut(&self.session) else {
            return Some(message);
        };

        let seq = self.input.increment_seq();
        let removed = Self::impl_push(reverse_queue, retention, seq, message);

        removed.map(|e| e.message)
    }

    pub(crate) fn push_as_oldest(&mut self, message: T) -> Option<T> {
        let retention = self.storage.settings.retention();
        let Some(reverse_queue) = self.storage.reverse_queues.get_mut(&self.session) else {
            return Some(message);
        };

        let seq = self.input.increment_seq();
        let entry = BufferEntry { message, seq };
        let replaced = match retention {
            RetentionPolicy::KeepFirst(n) => {
                if n > 0 && reverse_queue.len() >= n {
                    Some(reverse_queue.remove(0))
                } else {
                    None
                }
            }
            RetentionPolicy::KeepLast(n) => {
                if reverse_queue.len() >= n {
                    return Some(entry.message);
                }

                None
            }
            RetentionPolicy::KeepAll => None,
        };

        reverse_queue.push(entry);
        replaced.map(|e| e.message)
    }

    pub(crate) fn pull(&mut self) -> Option<T> {
        self.storage.reverse_queues.get_mut(&self.session)?.pop().map(|e| e.message)
    }

    pub(crate) fn pull_newest(&mut self) -> Option<T> {
        let reverse_queue = self.storage.reverse_queues.get_mut(&self.session)?;
        if reverse_queue.is_empty() {
            return None;
        }

        Some(reverse_queue.remove(0).message)
    }

    pub(crate) fn iter_mut(&mut self) -> IterBufferMut<'_, T>
    where
        T: 'static + Send + Sync,
    {
        let get_msg = get_message_mut::<T> as fn(&mut BufferEntry<T>) -> &mut T;
        IterBufferMut {
            iter: self
                .storage
                .reverse_queues
                .get_mut(&self.session)
                .map(|q| q.iter_mut().map(get_msg).rev()),
        }
    }

    pub(crate) fn oldest_mut(&mut self) -> Option<&mut T> {
        self.storage
            .reverse_queues
            .get_mut(&self.session)
            .and_then(|q| q.last_mut())
            .map(|e| &mut e.message)
    }

    pub(crate) fn newest_mut(&mut self) -> Option<&mut T> {
        self.storage
            .reverse_queues
            .get_mut(&self.session)
            .and_then(|q| q.first_mut())
            .map(|e| &mut e.message)
    }

    pub(crate) fn newest_mut_or_else(
        &mut self,
        f: impl FnOnce() -> T,
    ) -> Option<&mut T> {
        let f = || {
            let seq = self.input.increment_seq();
            (seq, f())
        };

        let retention = self.storage.settings.retention();
        self.storage.reverse_queues.get_mut(&self.session).and_then(|q| {
            if q.is_empty() {
                let (seq, message) = f();
                Self::impl_push(q, retention, seq, message);
            }

            q.first_mut().map(|e| &mut e.message)
        })
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let reverse_queue = self.storage.reverse_queues.get_mut(&self.session)?;
        let len = reverse_queue.len();
        if len <= index {
            return None;
        }

        reverse_queue.get_mut(len - index - 1).map(|e| &mut e.message)
    }

    pub(crate) fn drain<R>(&mut self, range: R) -> DrainBuffer<'_, T>
    where
        T: 'static + Send + Sync,
        R: RangeBounds<usize>,
    {
        let f = entry_into_message::<T> as fn(BufferEntry<T>) -> T;
        DrainBuffer {
            drain: self
                .storage
                .reverse_queues
                .get_mut(&self.session)
                .map(|q| q.drain(range).map(f).rev()),
        }
    }

    fn impl_push(
        reverse_queue: &mut SmallVec<[BufferEntry<T>; 16]>,
        retention: RetentionPolicy,
        seq: Seq,
        message: T,
    ) -> Option<BufferEntry<T>> {
        let entry = BufferEntry { message, seq };
        let replaced = match retention {
            RetentionPolicy::KeepFirst(n) => {
                if reverse_queue.len() >= n {
                    // We're at the limit for inputs in this queue so just send
                    // this back
                    return Some(entry);
                }

                None
            }
            RetentionPolicy::KeepLast(n) => {
                if n > 0 && reverse_queue.len() >= n {
                    reverse_queue.pop()
                } else if n == 0 {
                    // This can never store any number of entries
                    return Some(entry);
                } else {
                    None
                }
            }
            RetentionPolicy::KeepAll => None,
        };

        reverse_queue.insert(0, entry);
        replaced
    }

}

#[derive(Component)]
pub(crate) struct BufferStorage<T> {
    /// Settings that determine how this buffer will behave.
    settings: BufferSettings,
    /// Map from session ID to a queue of data that has arrived for it. This
    /// is used by nodes that feed into joiner nodes to store input so that it's
    /// readily available when needed.
    ///
    /// The main reason we use this as a reverse queue instead of a forward queue
    /// is because SmallVec doesn't have a version of pop that we can use on the
    /// front. We should reconsider whether this is really a sensible choice.
    reverse_queues: HashMap<Entity, SmallVec<[BufferEntry<T>; 16]>>,
}

pub(crate) struct BufferEntry<T> {
    #[allow(unused)]
    pub(crate) seq: Seq,
    pub(crate) message: T,
}

impl<T> BufferStorage<T> {
    pub(crate) fn count(&self, session: Entity) -> usize {
        self.reverse_queues
            .get(&session)
            .map(|q| q.len())
            .unwrap_or(0)
    }

    pub(crate) fn active_sessions(&self) -> SmallVec<[Entity; 16]> {
        self.reverse_queues.keys().copied().collect()
    }

    pub(crate) fn iter(&self, session: Entity) -> IterBufferView<'_, T>
    where
        T: 'static + Send + Sync,
    {
        let f = get_message_ref::<T> as fn(&BufferEntry<T>) -> &T;
        IterBufferView {
            iter: self.reverse_queues.get(&session).map(|q| q.iter().map(f).rev()),
        }
    }

    pub(crate) fn oldest(&self, session: Entity) -> Option<&T> {
        self.reverse_queues.get(&session).and_then(|q| q.last()).map(|e| &e.message)
    }

    pub(crate) fn newest(&self, session: Entity) -> Option<&T> {
        self.reverse_queues.get(&session).and_then(|q| q.first()).map(|e| &e.message)
    }

    pub(crate) fn get(&self, session: Entity, index: usize) -> Option<&T> {
        let reverse_queue = self.reverse_queues.get(&session)?;
        let len = reverse_queue.len();
        if len <= index {
            return None;
        }

        reverse_queue.get(len - index - 1).map(|e| &e.message)
    }

    pub(crate) fn new(settings: BufferSettings) -> Self {
        Self {
            settings,
            reverse_queues: Default::default(),
        }
    }

    pub(crate) fn ensure_session(&mut self, session: Entity) {
        self.reverse_queues.entry(session).or_default();
    }

    pub(crate) fn remove_session(&mut self, session: Entity) {
        self.reverse_queues.remove(&session);
    }
}

fn get_message_ref<T>(entry: &BufferEntry<T>) -> &T {
    &entry.message
}

fn get_message_mut<T>(entry: &mut BufferEntry<T>) -> &mut T {
    &mut entry.message
}

fn entry_into_message<T>(entry: BufferEntry<T>) -> T {
    entry.message
}

pub struct IterBufferView<'b, T>
where
    T: 'static + Send + Sync,
{
    iter: Option<Rev<Map<Iter<'b, BufferEntry<T>>, fn(&BufferEntry<T>) -> &T>>>,
}

impl<'b, T> Iterator for IterBufferView<'b, T>
where
    T: 'static + Send + Sync,
{
    type Item = &'b T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = &mut self.iter {
            iter.next()
        } else {
            None
        }
    }
}

pub struct IterBufferMut<'b, T>
where
    T: 'static + Send + Sync,
{
    iter: Option<Rev<Map<IterMut<'b, BufferEntry<T>>, fn(&mut BufferEntry<T>) -> &mut T>>>,
}

impl<'b, T> Iterator for IterBufferMut<'b, T>
where
    T: 'static + Send + Sync,
{
    type Item = &'b mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = &mut self.iter {
            iter.next()
        } else {
            None
        }
    }
}

pub struct DrainBuffer<'b, T>
where
    T: 'static + Send + Sync,
{
    drain: Option<Rev<Map<Drain<'b, [BufferEntry<T>; 16]>, fn(BufferEntry<T>) -> T>>>,
}

impl<'b, T> Iterator for DrainBuffer<'b, T>
where
    T: 'static + Send + Sync,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(drain) = &mut self.drain {
            drain.next()
        } else {
            None
        }
    }
}
