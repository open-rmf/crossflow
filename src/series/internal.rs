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

use bevy_derive::Deref;
use bevy_ecs::prelude::{Bundle, ChildOf, Children, Command, Component, Entity, Resource, World};

use backtrace::Backtrace;

use tokio::sync::mpsc::{
    UnboundedReceiver as TokioReceiver, UnboundedSender as TokioSender, unbounded_channel,
};

use smallvec::SmallVec;

use anyhow::anyhow;

use std::sync::Arc;

use crate::{
    Broken, Cancel, CancelFailure, Cancellable, Cancellation, DisposalListener, DisposalUpdate,
    ManageCancellation, MiscellaneousFailure, OnCancel, OperationError, OperationExecuteStorage,
    OperationRequest, OperationResult, OperationResultFilter, OperationSetup, SetupFailure,
    SingleTargetStorage, UnhandledErrors, UnusedTarget, OrBroken,
};

pub(crate) trait Executable {
    fn setup(self, info: OperationSetup) -> OperationResult;
    fn execute(request: OperationRequest) -> OperationResult;
}

#[derive(Bundle)]
pub(crate) struct SeriesSessionBundle {
    disposal_listener: DisposalListener,
    sequence: SequenceInSeries,
}

/// Ordered sequence of operations in a series
#[derive(Default, Clone, Debug, Component)]
pub(crate) struct SequenceInSeries(Vec<Entity>);

impl SequenceInSeries {
    pub(crate) fn push(&mut self, operation: Entity) {
        self.0.push(operation);
    }
}

impl SeriesSessionBundle {
    pub(crate) fn new() -> Self {
        Self {
            disposal_listener: DisposalListener(series_session_disposal_listener),
            sequence: Default::default(),
        }
    }
}

fn series_session_disposal_listener(
    DisposalUpdate {
        listener: _,
        origin,
        session,
        disposal,
        world,
        roster,
    }: DisposalUpdate,
) -> OperationResult {
    // The disposal happened for an operation in a series. If the
    // operation cannot be completed, then the series needs to be
    // cancelled.
    //
    // We do not convert stream disposals into a cancellation
    // because they do not affect the ability of the series to
    // reach its end.
    if !disposal.cause.is_stream_disposal() {
        let cancellation = Cancellation::unreachable(session, session, vec![disposal]);
        world.emit_series_cancel(origin, session, cancellation, roster);
    }

    Ok(())
}

pub(crate) struct AddExecution<E: Executable> {
    target: Entity,
    execution: E,
}

impl<E: Executable> AddExecution<E> {
    pub(crate) fn new(target: Entity, execution: E) -> Self {
        Self {
            target,
            execution,
        }
    }
}

impl<E: Executable + 'static + Sync + Send> Command for AddExecution<E> {
    fn apply(self, world: &mut World) {
        if let Err(error) = self.execution.setup(OperationSetup {
            source: self.target,
            world,
        }) {
            world
                .get_resource_or_insert_with(UnhandledErrors::default)
                .setup
                .push(SetupFailure {
                    broken_node: self.target,
                    error,
                });
        }

        world
            .entity_mut(self.target)
            .insert(OperationExecuteStorage(perform_execution::<E>))
            .remove::<UnusedTarget>();
    }
}

pub(crate) struct AddExecutableToSeries<E: Executable> {
    series: Entity,
    executable: AddExecution<E>,
}

impl<E: Executable> AddExecutableToSeries<E> {
    pub(crate) fn new(session: Entity, target: Entity, execution: E) -> Self {
        Self {
            series: session,
            executable: AddExecution { target, execution },
        }
    }
}

impl<E: Executable + 'static + Sync + Send> Command for AddExecutableToSeries<E> {
    fn apply(self, world: &mut World) -> () {
        let series = self.series;
        let target = self.executable.target;
        AddConnectionToSeries { series, target }.apply(world);
        self.executable.apply(world);
    }
}

pub(crate) struct AddConnectionToSeries {
    series: Entity,
    target: Entity,
}

impl Command for AddConnectionToSeries {
    fn apply(self, world: &mut World) -> () {
        let node = self.target;
        if let Err(OperationError::Broken(backtrace)) = self.try_apply(world) {
            world
                .get_resource_or_init::<UnhandledErrors>()
                .broken
                .push(Broken { node, backtrace });

            world.emit_broken()
        }
    }
}

impl AddConnectionToSeries {
    pub(crate) fn new(session: Entity, target: Entity) -> Self {
        Self {
            series: session,
            target,
        }
    }

    fn try_apply(self, world: &mut World) -> OperationResult {
        world.get_mut::<SequenceInSeries>(self.series).or_broken()?.push(self.target);
        Ok(())
    }
}

fn perform_execution<E: Executable>(
    OperationRequest {
        source,
        world,
        roster,
    }: OperationRequest,
) {
    match E::execute(OperationRequest {
        source,
        world,
        roster,
    }) {
        Ok(()) => {
            // Do nothing
        }
        Err(OperationError::NotReady) => {
            // Do nothing
        }
        Err(OperationError::Broken(backtrace)) => {
            if let Ok(mut source_mut) = world.get_entity_mut(source) {
                source_mut.emit_broken(backtrace, roster);
            }
        }
    }
}

pub(crate) fn cancel_series(cancel: Cancel) -> OperationResult {
    let Cancel {
        target: session,
        session: _,
        cancellation,
        world,
        roster,
    } = cancel;
    if let Some(operations) = world.get::<SequenceInSeries>(session) {
        let operations: SmallVec<[Entity; 8]> = operations.0.iter().cloned().collect();
        for op in operations {
            let Some(on_cancel) = world.get::<OnCancel>(op).map(|c| c.0) else {
                continue;
            };

            on_cancel(Cancel {
                target: op,
                session: Some(session),
                cancellation: cancellation.clone(),
                world,
                roster,
            })
            .ignore_not_ready()?;
        }
    }

    Ok(())
}

#[derive(Resource)]
pub(crate) struct SeriesLifecycleChannel {
    pub(crate) sender: TokioSender<Entity>,
    pub(crate) receiver: TokioReceiver<Entity>,
}

impl Default for SeriesLifecycleChannel {
    fn default() -> Self {
        let (sender, receiver) = unbounded_channel();
        Self { sender, receiver }
    }
}

/// This component tracks the lifecycle of an entity that is the terminal
/// target of a series. When this component gets dropped, the upstream
/// chain will be notified.
#[derive(Component)]
pub(crate) struct SeriesLifecycle {
    /// The series sources that are feeding into the entity which holds this
    /// component.
    sources: SmallVec<[Entity; 8]>,
    /// Used to notify the flusher that the target of the sources has been dropped
    sender: TokioSender<Entity>,
}

impl SeriesLifecycle {
    fn new(source: Entity, sender: TokioSender<Entity>) -> Self {
        Self {
            sources: SmallVec::from_iter([source]),
            sender,
        }
    }
}

impl Drop for SeriesLifecycle {
    fn drop(&mut self) {
        for source in &self.sources {
            if let Err(err) = self.sender.send(*source) {
                eprintln!(
                    "Failed to notify that a series was dropped: {err}\nBacktrace:\n{:#?}",
                    Backtrace::new(),
                );
            }
        }
    }
}

pub(crate) fn add_lifecycle_dependency(source: Entity, target: Entity, world: &mut World) {
    let sender = world
        .get_resource_or_insert_with(SeriesLifecycleChannel::default)
        .sender
        .clone();

    if let Some(mut lifecycle) = world.get_mut::<SeriesLifecycle>(target) {
        lifecycle.sources.push(source);
    } else if let Ok(mut target_mut) = world.get_entity_mut(target) {
        target_mut.insert(SeriesLifecycle::new(source, sender));
    } else {
        // The target is already despawned
        if let Err(err) = sender.send(source) {
            world
                .get_resource_or_insert_with(UnhandledErrors::default)
                .miscellaneous
                .push(MiscellaneousFailure {
                    error: Arc::new(anyhow!(
                        "Failed to notify that a target is already despawned: {err}"
                    )),
                    backtrace: Some(Backtrace::new()),
                })
        }
    }
}
