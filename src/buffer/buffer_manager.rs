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
    prelude::{Commands, Component, Entity, Mut, Query},
    query::QueryEntityError,
    system::SystemParam,
};

use smallvec::{Drain, SmallVec};

use std::collections::HashMap;

use std::{
    iter::{Map, Rev},
    ops::{Deref, DerefMut, RangeBounds},
    slice::{Iter, IterMut},
};

use crate::{
    BufferKeyTag, BufferSettings, BufferView, InputStorage, RequestId, RetentionPolicy, Seq,
};

#[cfg(feature = "trace")]
use crate::{
    BufferAccessRecord, BufferEvent, BufferModification, BufferPush, BufferRemoval, BufferTracer,
    MessageTracer, TraceBuffer, TraceTarget, TracedEvent, TracedMessage, WriteToTraceLog,
};

/// A wrapper type that allows the tracing feature to track changes to buffer
/// values. If the tracing feature is disabled, this will just provide regular
/// mutable access to an entry in the buffer.
pub struct BMut<'a, T> {
    pub(crate) entry: &'a mut BufferEntry<T>,
    pub(crate) tracer: BMutTracer<'a>,
}

pub(crate) struct BMutTracer<'a> {
    #[cfg(feature = "trace")]
    trace: MessageTracer<'a>,
    _ignore: std::marker::PhantomData<fn(&'a ())>,
}

impl<'a> BMutTracer<'a> {
    pub(crate) fn trace_mut<T: 'static + Send + Sync>(
        &self,
        #[allow(unused)] entry: &mut BufferEntry<T>,
    ) {
        #[cfg(feature = "trace")]
        {
            if entry.original.is_none() && self.trace.is_on() {
                let msg = self.trace.trace_message(&entry.message);
                entry.original = Some(msg);
            }
        }
    }
}

impl<'a, T: 'static + Send + Sync> BMut<'a, T> {
    /// View the value in the buffer without modifying it. Using this does not
    /// cause any tracing event.
    pub fn get(&self) -> &T {
        &self.entry.message
    }

    /// Get a mutable borrow of the value in the buffer. If tracing is enabled,
    /// the original value of this buffer entry will be noted, and a buffer
    /// modification event will be reported.
    pub fn get_mut(&mut self) -> &mut T {
        self.tracer.trace_mut(self.entry);
        &mut self.entry.message
    }
}

impl<T: 'static + Send + Sync> Deref for BMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: 'static + Send + Sync> DerefMut for BMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

#[derive(SystemParam)]
pub(crate) struct BufferMutQuery<'w, 's, T: 'static + Send + Sync> {
    #[cfg(feature = "trace")]
    tracer: BufferTracer<'w, 's>,
    query: Query<'w, 's, (&'static mut BufferStorage<T>, &'static mut InputStorage<T>)>,
    commands: Commands<'w, 's>,
}

impl<'w, 's, T: 'static + Send + Sync> BufferMutQuery<'w, 's, T> {
    pub(crate) fn get_manager<'a>(
        &'a mut self,
        req: RequestId,
        key: &BufferKeyTag,
    ) -> Result<BufferManager<'w, 's, 'a, T>, QueryEntityError> {
        #[cfg(feature = "trace")]
        {
            self.tracer
                .trace(req.into(), key, BufferAccessRecord::Viewed);
        }

        let (storage, input) = self.query.get_mut(key.buffer)?;

        Ok(BufferManager {
            storage,
            input,
            req,
            commands: &mut self.commands as *mut _,
            bmut: BMutBuilder {
                key: key.clone(),
                #[cfg(feature = "trace")]
                tracer: &self.tracer as *const _,
                _ignore: Default::default(),
            },
        })
    }

    pub(crate) fn get_view<'a>(
        &'a mut self,
        _req: RequestId,
        key: &BufferKeyTag,
    ) -> Result<BufferView<'a, T>, QueryEntityError> {
        #[cfg(feature = "trace")]
        {
            self.tracer
                .trace(_req.into(), key, BufferAccessRecord::Viewed);
        }
        self.get_view_untraced(key)
    }

    pub(crate) fn get_view_untraced<'a>(
        &'a self,
        key: &BufferKeyTag,
    ) -> Result<BufferView<'a, T>, QueryEntityError> {
        let session = key.session;
        self.query
            .get(key.buffer)
            .map(|(storage, _)| BufferView { storage, session })
    }
}

pub(crate) struct BufferManager<'w, 's, 'a, T: 'static + Send + Sync> {
    storage: Mut<'a, BufferStorage<T>>,
    input: Mut<'a, InputStorage<T>>,
    pub(crate) req: RequestId,
    // TODO(@mxgrey): We use a raw pointer here to escape an HRTB bug in the
    // Rust compiler: https://github.com/rust-lang/rust/issues/100013
    // When that issue is resolved we should try to revert this to a regular
    // safe borrow.
    pub(crate) commands: *mut Commands<'w, 's>,
    bmut: BMutBuilder<'w, 's, 'a>,
}

/// This provides an easy way to create BMut objects based on whether the trace
/// feature is on
#[derive(Clone)]
struct BMutBuilder<'w, 's, 'a> {
    key: BufferKeyTag,
    // TODO(@mxgrey): We use a raw pointer here to escape an HRTB bug in the
    // Rust compiler: https://github.com/rust-lang/rust/issues/100013
    // When that issue is resolved we should try to revert this to a regular
    // safe borrow.
    #[cfg(feature = "trace")]
    tracer: *const BufferTracer<'w, 's>,
    _ignore: std::marker::PhantomData<fn(&'w (), &'s (), &'a ())>,
}

impl<'w, 's, 'a> BMutBuilder<'w, 's, 'a> {
    fn build<'b, T>(&'b self, entry: &'b mut BufferEntry<T>) -> BMut<'b, T> {
        BMut {
            entry,
            tracer: BMutTracer {
                #[cfg(feature = "trace")]
                // SAFETY: The tracer pointer comes from a valid reference that
                // outlives this BufferManager, so it is safe to dereference.
                trace: unsafe { &*self.tracer }.get_message_tracer(&self.key),
                _ignore: Default::default(),
            },
        }
    }
}

impl<'w, 's, 'a, T: 'static + Send + Sync> BufferManager<'w, 's, 'a, T> {
    pub(crate) fn key_session(&self) -> Entity {
        self.bmut.key.session
    }

    pub(crate) fn len(&self) -> usize {
        self.storage.count(self.bmut.key.session)
    }

    pub(crate) fn iter(&self) -> IterBufferView<'_, T>
    where
        T: 'static + Send + Sync,
    {
        self.storage.iter(self.bmut.key.session)
    }

    pub(crate) fn oldest(&self) -> Option<&T> {
        self.storage.oldest(self.bmut.key.session)
    }

    pub(crate) fn newest(&self) -> Option<&T> {
        self.storage.newest(self.bmut.key.session)
    }

    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.storage.get(self.bmut.key.session, index)
    }

    pub(crate) fn force_push(&mut self, value: T) -> Option<T> {
        let seq = self.input.increment_seq();
        let retention = self.storage.settings.retention();
        let removed = Self::impl_push(
            self.storage
                .reverse_queues
                .entry(self.bmut.key.session)
                .or_default(),
            retention,
            seq,
            value,
            &self.req,
            &self.bmut.key,
            self.commands,
            #[cfg(feature = "trace")]
            self.bmut.tracer,
        );

        removed.map(|e| e.message)
    }

    pub(crate) fn push(&mut self, message: T) -> Option<T> {
        let retention = self.storage.settings.retention();
        let Some(reverse_queue) = self.storage.reverse_queues.get_mut(&self.bmut.key.session)
        else {
            return Some(message);
        };

        let seq = self.input.increment_seq();
        let removed = Self::impl_push(
            reverse_queue,
            retention,
            seq,
            message,
            &self.req,
            &self.bmut.key,
            self.commands,
            #[cfg(feature = "trace")]
            self.bmut.tracer,
        );

        removed.map(|e| e.message)
    }

    pub(crate) fn push_as_oldest(&mut self, message: T) -> Option<T> {
        let retention = self.storage.settings.retention();
        let Some(reverse_queue) = self.storage.reverse_queues.get_mut(&self.bmut.key.session)
        else {
            return Some(message);
        };

        let seq = self.input.increment_seq();
        let entry = BufferEntry::new(seq, message);
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

        #[cfg(feature = "trace")]
        Self::trace_message_replacement(
            &replaced,
            &entry,
            reverse_queue.len(),
            &self.req,
            &self.bmut.key,
            self.commands,
            self.bmut.tracer,
        );

        reverse_queue.push(entry);
        replaced.map(|e| e.message)
    }

    pub(crate) fn pull(&mut self) -> Option<T> {
        let entry = self
            .storage
            .reverse_queues
            .get_mut(&self.bmut.key.session)?
            .pop();

        #[cfg(feature = "trace")]
        if let Some(entry) = &entry {
            self.trace_removal(entry.seq);
        }

        entry.map(|e| e.message)
    }

    pub(crate) fn pull_newest(&mut self) -> Option<T> {
        let reverse_queue = self
            .storage
            .reverse_queues
            .get_mut(&self.bmut.key.session)?;
        if reverse_queue.is_empty() {
            return None;
        }

        let entry = reverse_queue.remove(0);
        #[cfg(feature = "trace")]
        self.trace_removal(entry.seq);

        Some(entry.message)
    }

    pub(crate) fn iter_mut(&mut self) -> IterBufferMut<'_, T>
    where
        T: 'static + Send + Sync,
    {
        IterBufferMut {
            iter: self
                .storage
                .reverse_queues
                .get_mut(&self.bmut.key.session)
                .map(|q| q.iter_mut().rev()),
            #[cfg(feature = "trace")]
            // SAFETY: The tracer pointer comes from a valid reference that
            // outlives this BufferManager, so it is safe to dereference.
            trace: unsafe { &*self.bmut.tracer }.get_message_tracer(&self.bmut.key),
        }
    }

    pub(crate) fn oldest_mut(&mut self) -> Option<BMut<'_, T>> {
        self.storage
            .reverse_queues
            .get_mut(&self.bmut.key.session)
            .and_then(|q| q.last_mut())
            .map(|e| self.bmut.build(e))
    }

    pub(crate) fn newest_mut(&mut self) -> Option<BMut<'_, T>> {
        self.storage
            .reverse_queues
            .get_mut(&self.bmut.key.session)
            .and_then(|q| q.first_mut())
            .map(|e| self.bmut.build(e))
    }

    pub(crate) fn newest_mut_or_else(&mut self, f: impl FnOnce() -> T) -> Option<BMut<'_, T>> {
        let f = || {
            let seq = self.input.increment_seq();
            (seq, f())
        };

        let retention = self.storage.settings.retention();
        self.storage
            .reverse_queues
            .get_mut(&self.bmut.key.session)
            .and_then(|q| {
                if q.is_empty() {
                    let (seq, message) = f();
                    Self::impl_push(
                        q,
                        retention,
                        seq,
                        message,
                        &self.req,
                        &self.bmut.key,
                        self.commands,
                        #[cfg(feature = "trace")]
                        self.bmut.tracer,
                    );
                }

                q.first_mut().map(|e| self.bmut.build(e))
            })
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<BMut<'_, T>> {
        let reverse_queue = self
            .storage
            .reverse_queues
            .get_mut(&self.bmut.key.session)?;
        let len = reverse_queue.len();
        if len <= index {
            return None;
        }

        reverse_queue
            .get_mut(len - index - 1)
            .map(|e| self.bmut.build(e))
    }

    pub(crate) fn drain<R>(&mut self, range: R) -> DrainBuffer<'w, 's, '_, T>
    where
        T: 'static + Send + Sync,
        R: RangeBounds<usize>,
    {
        DrainBuffer {
            drain: self
                .storage
                .reverse_queues
                .get_mut(&self.bmut.key.session)
                .map(|q| q.drain(range).rev()),
            _commands: self.commands,
            #[cfg(feature = "trace")]
            // SAFETY: The tracer pointer comes from a valid reference that
            // outlives this BufferManager, so it is safe to dereference.
            tracer: unsafe { &*self.bmut.tracer }.get_message_tracer(&self.bmut.key),
            #[cfg(feature = "trace")]
            accessor: unsafe { &*self.bmut.tracer }.get_trace_target(self.req),
            #[cfg(feature = "trace")]
            buffer: unsafe { &*self.bmut.tracer }.get_trace_buffer(&self.bmut.key),
        }
    }

    fn impl_push(
        reverse_queue: &mut SmallVec<[BufferEntry<T>; 16]>,
        retention: RetentionPolicy,
        seq: Seq,
        message: T,
        _req: &RequestId,
        _key: &BufferKeyTag,
        _cmds: *mut Commands,
        #[cfg(feature = "trace")] tracer: *const BufferTracer,
    ) -> Option<BufferEntry<T>> {
        let entry = BufferEntry::new(seq, message);
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

        #[cfg(feature = "trace")]
        Self::trace_message_replacement(&replaced, &entry, 0, _req, _key, _cmds, tracer);

        reverse_queue.insert(0, entry);
        replaced
    }

    #[cfg(feature = "trace")]
    fn trace_modifications(&mut self) {
        // SAFETY: Both pointers come from valid references that outlive this
        // BufferManager, so they are safe to dereference.
        let (commands, tracer) = unsafe { (&mut *self.commands, &*self.bmut.tracer) };
        let toggle = tracer.get_trace_toggle(&self.bmut.key);
        if !toggle.is_on() {
            return;
        }

        let instant = std::time::Instant::now();
        let time = std::time::SystemTime::now();
        let accessor = tracer.get_trace_target(self.req);
        let buffer = tracer.get_trace_buffer(&self.bmut.key);
        let trace = tracer.get_message_tracer(&self.bmut.key);

        if let Some(reverse_queue) = self.storage.reverse_queues.get_mut(&self.bmut.key.session) {
            for BufferEntry {
                seq,
                message,
                original,
            } in reverse_queue.iter_mut().rev()
            {
                if let Some(original) = original.take() {
                    let event = BufferEvent {
                        accessor: accessor.clone(),
                        buffer: buffer.clone(),
                        access: BufferAccessRecord::Modified(BufferModification {
                            seq: *seq,
                            original,
                            modified: trace.trace_message(message),
                        }),
                    };

                    commands.write_trace(TracedEvent {
                        event: event.into(),
                        instant,
                        time,
                    });
                }
            }
        }
    }

    #[cfg(feature = "trace")]
    fn trace_message_replacement(
        replaced: &Option<BufferEntry<T>>,
        pushed: &BufferEntry<T>,
        position: usize,
        req: &RequestId,
        key: &BufferKeyTag,
        cmds: *mut Commands,
        tracer: *const BufferTracer,
    ) {
        // SAFETY: Both pointers come from valid references that outlive the
        // BufferManager, so they are safe to dereference.
        let (cmds, tracer) = unsafe { (&mut *cmds, &*tracer) };
        let toggle = tracer.get_trace_toggle(key);
        if toggle.is_on() {
            let trace = tracer.get_message_tracer(key);
            let instant = std::time::Instant::now();
            let time = std::time::SystemTime::now();
            let accessor = tracer.get_trace_target(*req);
            let buffer = tracer.get_trace_buffer(key);
            if let Some(replaced) = replaced {
                let seq = replaced.seq;
                let access = BufferAccessRecord::Removed(BufferRemoval { seq });
                let event = BufferEvent {
                    accessor: accessor.clone(),
                    buffer: buffer.clone(),
                    access,
                };
                cmds.write_trace(TracedEvent {
                    event: event.into(),
                    instant,
                    time,
                });
            }

            let seq = pushed.seq;
            let message = trace.trace_message(&pushed.message);
            let access = BufferAccessRecord::Pushed(BufferPush {
                seq,
                position,
                message,
            });
            let event = BufferEvent {
                accessor,
                buffer,
                access,
            };
            cmds.write_trace(TracedEvent {
                event: event.into(),
                instant,
                time,
            });
        }
    }

    #[cfg(feature = "trace")]
    fn trace_removal(&mut self, seq: Seq) {
        // SAFETY: Both pointers come from valid references that outlive this
        // BufferManager, so they are safe to dereference.
        let (commands, tracer) = unsafe { (&mut *self.commands, &*self.bmut.tracer) };
        let toggle = tracer.get_trace_toggle(&self.bmut.key);
        if toggle.is_on() {
            let instant = std::time::Instant::now();
            let time = std::time::SystemTime::now();
            let accessor = tracer.get_trace_target(self.req);
            let buffer = tracer.get_trace_buffer(&self.bmut.key);
            let access = BufferAccessRecord::Removed(BufferRemoval { seq });
            let event = BufferEvent {
                accessor,
                buffer,
                access,
            };
            commands.write_trace(TracedEvent {
                event: event.into(),
                instant,
                time,
            });
        }
    }
}

impl<'w, 's, 'a, T: 'static + Send + Sync> Drop for BufferManager<'w, 's, 'a, T> {
    fn drop(&mut self) {
        #[cfg(feature = "trace")]
        self.trace_modifications();
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
    /// When tracing is enabled, this field is used to track whether a buffer
    /// has changed during a mutable access, and if so this will contain its
    /// original value. This should be cleared out with each Drop of the
    /// BufferManager.
    #[cfg(feature = "trace")]
    pub(crate) original: Option<TracedMessage>,
}

impl<T> BufferEntry<T> {
    pub(crate) fn new(seq: Seq, message: T) -> Self {
        Self {
            seq,
            message,
            #[cfg(feature = "trace")]
            original: None,
        }
    }
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
            iter: self
                .reverse_queues
                .get(&session)
                .map(|q| q.iter().map(f).rev()),
        }
    }

    pub(crate) fn oldest<'a>(&'a self, session: Entity) -> Option<&'a T> {
        self.reverse_queues
            .get(&session)
            .and_then(|q| q.last())
            .map(|e| &e.message)
    }

    pub(crate) fn newest(&self, session: Entity) -> Option<&T> {
        self.reverse_queues
            .get(&session)
            .and_then(|q| q.first())
            .map(|e| &e.message)
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
    iter: Option<Rev<IterMut<'b, BufferEntry<T>>>>,
    #[cfg(feature = "trace")]
    trace: MessageTracer<'b>,
}

impl<'b, T> Iterator for IterBufferMut<'b, T>
where
    T: 'static + Send + Sync,
{
    type Item = BMut<'b, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = &mut self.iter {
            iter.next().map(|entry| BMut {
                entry,
                tracer: BMutTracer {
                    #[cfg(feature = "trace")]
                    trace: self.trace,
                    _ignore: Default::default(),
                },
            })
        } else {
            None
        }
    }
}

pub struct DrainBuffer<'w, 's, 'b, T>
where
    T: 'static + Send + Sync,
{
    drain: Option<Rev<Drain<'b, [BufferEntry<T>; 16]>>>,
    // TODO(@mxgrey): We use a raw pointer here to escape an HRTB bug in the
    // Rust compiler: https://github.com/rust-lang/rust/issues/100013
    // When that issue is resolved we should try to revert this to a regular
    // safe borrow.
    _commands: *mut Commands<'w, 's>,
    #[cfg(feature = "trace")]
    tracer: MessageTracer<'b>,
    #[cfg(feature = "trace")]
    accessor: TraceTarget,
    #[cfg(feature = "trace")]
    buffer: TraceBuffer,
}

impl<'w, 's, 'b, T> Iterator for DrainBuffer<'w, 's, 'b, T>
where
    T: 'static + Send + Sync,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(drain) = &mut self.drain {
            let entry = drain.next();

            #[cfg(feature = "trace")]
            if self.tracer.is_on()
                && let Some(entry) = &entry
            {
                let seq = entry.seq;
                let instant = std::time::Instant::now();
                let time = std::time::SystemTime::now();
                let access = BufferAccessRecord::Removed(BufferRemoval { seq });
                let event = BufferEvent {
                    accessor: self.accessor.clone(),
                    buffer: self.buffer.clone(),
                    access,
                };
                // SAFETY: The _commands pointer comes from a valid reference
                // that outlives this DrainBuffer, so it is safe to dereference.
                unsafe { &mut *self._commands }.write_trace(TracedEvent {
                    event: event.into(),
                    instant,
                    time,
                });
            }

            entry.map(|e| e.message)
        } else {
            None
        }
    }
}
