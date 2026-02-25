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
    prelude::{Bundle, Command, Component, Entity},
    world::{EntityRef, EntityWorldMut, World},
};

#[cfg(feature = "trace")]
use bevy_ecs::prelude::ChildOf;

use smallvec::{smallvec, SmallVec};

#[cfg(feature = "trace")]
use std::{
    num::Wrapping,
    sync::Arc,
};

use backtrace::Backtrace;

use crate::{
    Broken, BufferStorage, Cancel, Cancellation, CancellationCause, DeferredRoster, Detached,
    MiscellaneousFailure, OperationError, OperationRoster, OrBroken, SessionStatus,
    UnhandledErrors, UnusedTarget, OutputPort, MessageSent, BufferWorldAccess,
    RequestId, BufferKeyTag,
};

#[cfg(feature = "trace")]
use crate::{OperationStarted, Trace};

pub type Seq = u32;

/// This contains data that has been provided as input into an operation, along
/// with an indication of what session the data belongs to.
pub struct Input<T> {
    pub session: Entity,
    pub seq: Seq,
    pub data: T,
}

/// General purpose input storage used by most [operations](crate::Operation).
/// This component is inserted on the source entity of the operation and will
/// queue up inputs that have arrived for the source.
#[derive(Component)]
pub struct InputStorage<T> {
    // Items will be inserted into this queue from the front, so we pop off the
    // back to get the oldest items out.
    // TODO(@mxgrey): Consider if it's worth implementing a Deque on top of
    // the SmallVec data structure.
    reverse_queue: SmallVec<[Input<T>; 16]>,
    sequence: Wrapping<Seq>,
}

impl<T> InputStorage<T> {
    pub fn new() -> Self {
        Self {
            reverse_queue: Default::default(),
            sequence: Default::default(),
        }
    }

    pub fn contains_session(&self, session: Entity) -> bool {
        self.reverse_queue
            .iter()
            .any(|input| input.session == session)
    }

    fn push(&mut self, session: Entity, data: T) -> u32 {
        let seq = self.increment_seq();
        self.reverse_queue.insert(0, Input { session, seq, data });
        seq
    }

    pub fn increment_seq(&mut self) -> Seq {
        let seq = self.sequence.0;
        self.sequence += 1;
        seq
    }
}

impl<T> Default for InputStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Used to keep track of the expected input type for an operation
#[derive(Component)]
pub(crate) struct InputTypeIndicator {
    pub(crate) name: &'static str,
}

impl InputTypeIndicator {
    fn new<T>() -> Self {
        Self {
            name: std::any::type_name::<T>(),
        }
    }
}

#[derive(Bundle)]
pub struct InputBundle<T: 'static + Send + Sync> {
    storage: InputStorage<T>,
    indicator: InputTypeIndicator,
}

impl<T: 'static + Send + Sync> InputBundle<T> {
    pub fn new() -> Self {
        Self::custom(Default::default())
    }

    pub fn custom(storage: InputStorage<T>) -> Self {
        Self {
            storage,
            indicator: InputTypeIndicator::new::<T>(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MessageRoute<'a> {
    pub session: Entity,
    pub source: Entity,
    pub seq: Seq,
    pub port: OutputPort<'a>,
    pub target: Entity,
}

pub struct Routing<'a> {
    pub outputs: SmallVec<[RouteSource<'a>; 8]>,
    pub input: RouteTarget,
}

pub struct RouteSource<'a> {
    pub session: Entity,
    pub source: Entity,
    pub seq: Seq,
    pub port: OutputPort<'a>,
}

pub struct RouteTarget {
    pub session: Entity,
    pub target: Entity,
}

impl<'a> From<MessageRoute<'a>> for Routing<'a> {
    fn from(route: MessageRoute<'a>) -> Self {
        Routing {
            outputs: smallvec![RouteSource {
                session: route.session,
                source: route.source,
                seq: route.seq,
                port: route.port,
            }],
            input: RouteTarget {
                session: route.session,
                target: route.target,
            },
        }
    }
}

impl<T: 'static + Send + Sync> Default for InputBundle<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CleanInputsOf {
    pub session: Entity,
    pub source: Entity,
}

pub trait ManageInput {
    /// Give an input to this node. The node will be queued up to immediately
    /// process the input.
    fn give_input<'a, T: 'static + Send + Sync>(
        &mut self,
        route: impl Into<Routing<'a>>,
        data: T,
        roster: &mut OperationRoster,
    ) -> Result<(), OperationError>;

    /// Same as [`Self::give_input`], but the wakeup for this node will be
    /// deferred until after the async updates are flushed. This is used for
    /// async task output to ensure that all async operations, such as streams,
    /// are finished being processed before the final output gets processed.
    fn defer_input<'a, T: 'static + Send + Sync>(
        &mut self,
        route: impl Into<Routing<'a>>,
        data: T,
        roster: &mut OperationRoster,
    ) -> Result<(), OperationError>;

    /// Give an input to this node without flagging it in the roster. This
    /// should not generally be used. It's only for special cases where we know
    /// the node will be manually run after giving this input. It's marked
    /// unsafe to bring attention to this requirement.
    ///
    /// # Safety
    ///
    /// After calling this function you must make sure to either add the target
    /// operation to the queue or run the operation explicitly. Failing to do
    /// one of these could mean that this input (or one that follows it) will
    /// never be processed, which could cause a workflow to hang forever.
    unsafe fn sneak_input<'a, T: 'static + Send + Sync>(
        &mut self,
        route: impl Into<Routing<'a>>,
        data: T,
        only_if_active: bool,
        roster: &mut OperationRoster,
    ) -> Result<bool, OperationError>;

    /// Get an input that is ready to be taken, or else produce an error.
    fn take_input<T: 'static + Send + Sync>(
        &mut self,
        source: Entity,
    ) -> Result<Input<T>, OperationError>;

    /// Try to take an input if one is ready. If no input is ready this will
    /// return Ok(None). It only returns an error if the node is broken.
    fn try_take_input<T: 'static + Send + Sync>(
        &mut self,
        source: Entity,
    ) -> Result<Option<Input<T>>, OperationError>;

    fn cleanup_inputs<T: 'static + Send + Sync>(
        &mut self,
        clean: CleanInputsOf,
    );
}

pub trait InspectInput {
    fn has_input<T: 'static + Send + Sync>(&self, session: Entity) -> Result<bool, OperationError>;
}

impl ManageInput for World {
    fn give_input<'a, T: 'static + Send + Sync>(
        &mut self,
        route: impl Into<Routing<'a>>,
        data: T,
        roster: &mut OperationRoster,
    ) -> Result<(), OperationError> {
        let route: Routing = route.into();
        let target = route.input.target;
        if unsafe { self.sneak_input(route, data, true, roster)? } {
            roster.queue(target);
        }
        Ok(())
    }

    fn defer_input<'a, T: 'static + Send + Sync>(
        &mut self,
        route: impl Into<Routing<'a>>,
        data: T,
        roster: &mut OperationRoster,
    ) -> Result<(), OperationError> {
        let route: Routing = route.into();
        let target = route.input.target;
        if unsafe { self.sneak_input(route, data, true, roster)? } {
            roster.defer(target);
        }
        Ok(())
    }

    unsafe fn sneak_input<'a, T: 'static + Send + Sync>(
        &mut self,
        route: impl Into<Routing<'a>>,
        data: T,
        only_if_active: bool,
        roster: &mut OperationRoster,
    ) -> Result<bool, OperationError> {
        let route: Routing = route.into();
        let session = route.input.session;
        let target = route.input.target;

        if only_if_active {
            let active_session =
                if let Some(session_status) = self.get::<SessionStatus>(session) {
                    matches!(session_status, SessionStatus::Active)
                } else {
                    false
                };

            if !active_session {
                // The session being sent is not active, either it is being cleaned
                // or already despawned. Therefore we should not propogate any inputs
                // related to it.
                return Ok(false);
            }
        }

        let mut serialized_msg = None;
        let mut perform_trace = false;
        #[cfg(feature = "trace")]
        {
            if let Some(trace) = self.get::<Trace>(target) {
                if trace.toggle().is_on() {
                    perform_trace = true;
                }

                if trace.toggle().with_messages() {
                    serialized_msg = trace.serialize_value(&data);
                }
            }

            if !perform_trace {
                // Check if any of the sources want to trace
                for output in &route.outputs {
                    if let Some(trace) = self.get::<Trace>(output.source) {
                        if trace.toggle().is_on() {
                            perform_trace = true;
                            break;
                        }
                    }
                }
            }
        }

        if let Some(mut storage) = self.get_mut::<InputStorage<T>>(target) {
            let target_seq = storage.push(session, data);

            #[cfg(feature = "trace")]
            {
                if perform_trace {
                    MessageSent::trace(route, target_seq, serialized_msg, self);
                }
            }

        } else if self.get::<UnusedTarget>(target).is_none() {
            if let Some(detached) = self.get::<Detached>(target) {
                if detached.is_detached() {
                    // The input is going to a detached series that will not
                    // react any further. We need to tell that detached series
                    // to despawn since it is no longer needed.
                    roster.defer_despawn(target);

                    // No error occurred, but the caller should not queue the
                    // operation into the roster because it is being despawned.
                    return Ok(false);
                }
            }

            let expected = self.get::<InputTypeIndicator>(target).map(|i| i.name);
            // If the input is being fed to an unused target then we can
            // generally ignore it, although it may indicate a bug in the user's
            // workflow because workflow branches that end in an unused target
            // will be spuriously dropped when the scope terminates.

            // However in this case, the target is not unused but also does not
            // have the correct input storage type. This indicates a bug in
            // crossflow itself, since the API should ensure that connection
            // mismatches are impossible.
            self.get_resource_or_insert_with(|| UnhandledErrors::default())
                .miscellaneous
                .push(MiscellaneousFailure {
                    error: std::sync::Arc::new(anyhow::anyhow!(
                        "Incorrect input type for operation [{:?}]: received [{}], expected [{}]",
                        target,
                        std::any::type_name::<T>(),
                        expected.unwrap_or("<undefined>"),
                    )),
                    backtrace: Some(Backtrace::new()),
                });
            None.or_broken()?;
        }
        Ok(true)
    }

    fn take_input<T: 'static + Send + Sync>(
        &mut self,
        source: Entity,
    ) -> Result<Input<T>, OperationError> {
        self.try_take_input(source)?.or_not_ready()
    }

    fn try_take_input<T: 'static + Send + Sync>(
        &mut self,
        source: Entity,
    ) -> Result<Option<Input<T>>, OperationError> {
        let mut storage = self.get_mut::<InputStorage<T>>(source).or_broken()?;
        let input = storage.reverse_queue.pop();

        #[cfg(feature = "trace")]
        {
            if let Some(input) = &input {
                if let Some(trace) = self.get::<Trace>(source) {
                    if trace.toggle().is_on() {
                        let message = trace
                            .toggle()
                            .with_messages()
                            .then(|| trace.serialize_value(&input.data))
                            .flatten()
                            .transpose();

                        let message = match message {
                            Ok(message) => message,
                            Err(err) => {
                                self.get_resource_or_insert_with(UnhandledErrors::default)
                                    .miscellaneous
                                    .push(MiscellaneousFailure {
                                        error: Arc::new(err.into()),
                                        backtrace: Some(Backtrace::new()),
                                    });
                                return Err(OperationError::Broken(Some(Backtrace::new())));
                            }
                        };

                        let mut session_stack = SmallVec::new();
                        session_stack.push(input.session);
                        let mut session = input.session;
                        while let Some(next_session) = self.get::<ChildOf>(session) {
                            session = next_session.parent();
                            session_stack.push(session);
                        }
                        session_stack.reverse();

                        let started = OperationStarted {
                            operation: source,
                            session_stack,
                            info: Arc::clone(trace.info()),
                            message,
                        };

                        self.send_event(started);
                    }
                }
            }
        }

        Ok(input)
    }

    fn cleanup_inputs<T: 'static + Send + Sync>(
        &mut self,
        CleanInputsOf { session, source }: CleanInputsOf,
    ) {
        if self.get::<BufferStorage<T>>(source).is_some() {
            // Buffers are handled in a special way because the data of some
            // buffers will be used during cancellation. Therefore we do not
            // want to just delete their contents, but instead store them in the
            // buffer storage until the scope gives the signal to clear all
            // buffer data after all the cancellation workflows are finished.
            if let Some(mut inputs) = self.get_mut::<InputStorage<T>>(source) {
                // Pull out only the data that belongs to the specified session
                let remaining_indices: SmallVec<[usize; 16]> = inputs
                    .reverse_queue
                    .iter()
                    .enumerate()
                    .filter_map(|(i, input)| {
                        if input.session == session {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut reverse_remaining: SmallVec<[Input<T>; 16]> = SmallVec::new();
                for i in remaining_indices.into_iter().rev() {
                    reverse_remaining.push(inputs.reverse_queue.remove(i));
                }

                for Input { data, seq, session } in reverse_remaining.into_iter().rev() {
                    let req = RequestId { source, seq, session };
                    let key = BufferKeyTag {
                        buffer: source,
                        accessor: source,
                        session,
                        lifecycle: None,
                    };

                    if let Err(_) = self.unchecked_buffer_mut::<T, _>(req, &key, |mut buffer| {
                        buffer.force_push(data);
                    }) {
                        self.get_resource_or_insert_with(UnhandledErrors::default)
                            .broken
                            .push(Broken {
                                node: source,
                                backtrace: Some(Backtrace::new()),
                            });
                    }
                }
            }

            return;
        }

        if let Some(mut inputs) = self.get_mut::<InputStorage<T>>(source) {
            inputs
                .reverse_queue
                .retain(|Input { session: r, .. }| *r != session);
        }
    }
}

impl<'a> InspectInput for EntityWorldMut<'a> {
    fn has_input<T: 'static + Send + Sync>(&self, session: Entity) -> Result<bool, OperationError> {
        let inputs = self.get::<InputStorage<T>>().or_broken()?;
        Ok(inputs.contains_session(session))
    }
}

impl<'a> InspectInput for EntityRef<'a> {
    fn has_input<T: 'static + Send + Sync>(&self, session: Entity) -> Result<bool, OperationError> {
        let inputs = self.get::<InputStorage<T>>().or_broken()?;
        Ok(inputs.contains_session(session))
    }
}

pub(crate) struct InputCommand<T> {
    pub(crate) target: Entity,
    pub(crate) session: Entity,
    pub(crate) data: T,
}

impl<T: 'static + Send + Sync> Command for InputCommand<T> {
    fn apply(self, world: &mut World) {
        match world.get_mut::<InputStorage<T>>(self.target) {
            Some(mut storage) => {
                storage.push(self.session, self.data);
                world
                    .get_resource_or_insert_with(DeferredRoster::default)
                    .queue(self.target);
            }
            None => {
                let cause = CancellationCause::Broken(Broken {
                    node: self.target,
                    backtrace: Some(Backtrace::new()),
                });
                let cancel = Cancel {
                    origin: self.target,
                    target: self.session,
                    session: Some(self.session),
                    cancellation: Cancellation::from_cause(cause),
                };

                world
                    .get_resource_or_insert_with(DeferredRoster::default)
                    .cancel(cancel);
            }
        }
    }
}
