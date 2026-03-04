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
use bevy_ecs::prelude::{Bundle, Command, Component, Entity, Resource, World};

use backtrace::Backtrace;

use tokio::sync::mpsc::{
    UnboundedReceiver as TokioReceiver, UnboundedSender as TokioSender, unbounded_channel,
};

use smallvec::{smallvec, SmallVec};

use anyhow::anyhow;

use std::sync::Arc;

use crate::{
    Cancel, Cancellation, DisposalListener, DisposalUpdate,
    ManageCancellation, MiscellaneousFailure, OnCancel, OperationError, OperationExecuteStorage,
    OperationRequest, OperationResult, OperationResultFilter, OperationSetup, SetupFailure,
    UnhandledErrors, UnusedTarget, OrBroken, ManageSession, DeferredRoster, SessionStatus,
    Cancellable, OperationType, CleanupContents, FinalizeCleanup, FinalizeCleanupRequest,
    Cleanup, OperationRoster, Broken, OnCleanup, OperationCleanup, RequestId, Detached,
    UnusedTargetDrop, DisposalInformation,
};

#[cfg(feature = "trace")]
use crate::SessionEvent;

pub(crate) trait Executable {
    fn setup(self, info: OperationSetup) -> OperationResult;
    fn execute(request: OperationRequest) -> OperationResult;

    /// Specify a type name for the operation. This will be stored
    fn operation_type(&self) -> Arc<str> {
        std::any::type_name::<Self>().into()
    }
}

#[derive(Bundle)]
pub(crate) struct SeriesSessionBundle {
    disposal_listener: DisposalListener,
    cancellable: Cancellable,
    sequence: SequenceInSeries,
    status: SessionStatus,
    op_type: OperationType,
    progress: ProgressInSeries,
    cleanup: CleanupContents,
    finalize_cleanup: FinalizeCleanup,
}

/// Ordered sequence of operations in a series
#[derive(Default, Clone, Debug, Component, Deref)]
pub struct SequenceInSeries(Vec<Entity>);

#[derive(Default, Clone, Debug, Component, Deref)]
pub struct ProgressInSeries(pub(crate) Option<Entity>);

#[derive(Clone, Copy, Debug, Component, Deref)]
pub struct InSeries(Entity);

impl InSeries {
    pub fn series(&self) -> Entity {
        self.0
    }
}

impl SequenceInSeries {
    pub(crate) fn push(&mut self, operation: Entity) {
        self.0.push(operation);
    }
}

impl SeriesSessionBundle {
    pub(crate) fn new() -> Self {
        Self {
            disposal_listener: DisposalListener(series_session_disposal_listener),
            cancellable: Cancellable::new(cancel_series),
            sequence: Default::default(),
            status: SessionStatus::Active,
            op_type: OperationType::new("SeriesSession".into()),
            progress: Default::default(),
            cleanup: Default::default(),
            finalize_cleanup: FinalizeCleanup(finalize_series_cleanup),
        }
    }
}

fn finalize_series_cleanup(
    FinalizeCleanupRequest {
        cleanup: Cleanup { session: series, .. },
        world,
        roster,
    }: FinalizeCleanupRequest,
) -> OperationResult {
    // The series is finished cleaning up, so cancel and despawn the session
    let cancellation = world
        .get_mut::<SessionStatus>(series)
        .or_broken()?
        .cancellation()
        .or_broken()?;

    finalize_series_cancel(series, cancellation, world, roster)
}

pub(crate) fn finalize_series_cancel(
    series: Entity,
    cancellation: Cancellation,
    world: &mut World,
    roster: &mut OperationRoster,
) -> OperationResult {
    if let Some(operations) = world.get::<SequenceInSeries>(series) {
        if let Some(last_op) = operations.last().cloned() {
            if let Some(on_cancel) = world.get::<OnCancel>(last_op).map(|c| c.0) {
                let r = on_cancel(Cancel {
                    target: last_op,
                    session: Some(series),
                    cancellation,
                    world,
                    roster,
                });

                if let Err(OperationError::Broken(backtrace)) = r {
                    world.get_resource_or_init::<UnhandledErrors>()
                        .broken
                        .push(Broken { node: last_op, backtrace });
                }
            };
        }
    }

    #[cfg(feature = "trace")]
    {
        SessionEvent::despawned(series, world);
    }

    world.despawn_session(series);
    Ok(())
}

fn series_session_disposal_listener(
    DisposalUpdate {
        info: DisposalInformation {
            listener: _,
            trigger,
            disposed: _,
            session,
            disposal,
        },
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
        world.emit_series_cancel(trigger.as_borrowed(), session, cancellation, roster);
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
        let operation_type = self.execution.operation_type();
        world.entity_mut(self.target).insert(OperationType::new(operation_type));

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
        AddToSeries { series, target }.apply(world);
        self.executable.apply(world);
    }
}

pub(crate) struct AddToSeries {
    series: Entity,
    target: Entity,
}

impl Command for AddToSeries {
    fn apply(self, world: &mut World) -> () {
        let node = self.target;
        if let Err(OperationError::Broken(backtrace)) = self.try_apply(world) {
            world.get_resource_or_init::<DeferredRoster>();
            world.resource_scope::<DeferredRoster, _>(|world, mut roster| {
                world.emit_broken(node, backtrace, &mut *roster);
            });
        }
    }
}

impl AddToSeries {
    pub(crate) fn new(session: Entity, target: Entity) -> Self {
        Self {
            series: session,
            target,
        }
    }

    fn try_apply(self, world: &mut World) -> OperationResult {
        world.get_mut::<SequenceInSeries>(self.series).or_broken()?.push(self.target);
        world.get_entity_mut(self.target).or_broken()?.insert(InSeries(self.series));
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
            world.emit_broken(source, backtrace, roster);
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
    if let Some(ProgressInSeries(Some(progress))) = world.get::<ProgressInSeries>(session) {
        // Make sure no outputs from this operation can make progress, or else
        // the series might continue in a broken state.
        let progress = *progress;
        roster.purge(progress);

        let cleanup_id = RequestId { session, source: session, seq: 0 };
        world.get_mut::<CleanupContents>(session).or_broken()?.add_cleanup(
            cleanup_id,
            smallvec![progress],
        );

        // Attempt to cleanup the in-progress operation, in case it is long-running
        let is_cleaning = OperationCleanup::new(
            session,
            progress,
            session,
            cleanup_id,
            world,
            roster,
        )
            .clean();

        if is_cleaning {
            // Return and wait to receive the cleanup notification from the operation
            *world.get_mut::<SessionStatus>(session).or_broken()? = SessionStatus::Dropped {
                stop_at: progress,
                cancellation,
            };
            return Ok(());
        }
    }

    finalize_series_cancel(session, cancellation, world, roster)?;

    Ok(())
}

fn clean_series_from_progress_point(
    series: Entity,
    progress: Entity,
    cancellation: &Cancellation,
    world: &mut World,
    roster: &mut OperationRoster,
) -> Result<bool, OperationError> {
    // Make sure no outputs from this operation can make progress, or else
    // the series might continue in a broken state.
    roster.purge(progress);

    let cleanup_id = RequestId { session: series, source: series, seq: 0 };
    world.get_mut::<CleanupContents>(series).or_broken()?.add_cleanup(
        cleanup_id,
        smallvec![progress],
    );

    // Attempt to cleanup the in-progress operation, in case it is long-running
    let is_cleaning = OperationCleanup::new(
        series,
        progress,
        series,
        cleanup_id,
        world,
        roster,
    )
        .clean();

    if is_cleaning {
        *world.get_mut::<SessionStatus>(series).or_broken()? = SessionStatus::Dropped {
            stop_at: progress,
            cancellation: cancellation.clone(),
        };
    }
    Ok(is_cleaning)
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

pub(crate) fn drop_series_target(
    target: Entity,
    world: &mut World,
    roster: &mut OperationRoster,
    unused: bool,
) -> OperationResult {
    let mut dropped_operations = Vec::new();
    if world.get_entity(target).is_err() {
        // The session has already despawned
        return Ok(());
    }
    let series = world.get::<InSeries>(target).or_broken()?.series();
    let sequence = world.get::<SequenceInSeries>(series).or_broken()?;

    let mut reached_drop = None;
    for op in sequence.iter().rev() {
        if let Some(detachment) = world.get::<Detached>(*op) {
            if detachment.is_detached() {
                break;
            }
        }

        dropped_operations.push(*op);

        if let Some(ProgressInSeries(Some(progress))) = world.get::<ProgressInSeries>(series) {
            if *op == *progress {
                reached_drop = Some(*progress);
                // Stop dropping anything that comes before the last progress point
                break;
            }
        }
    }

    dropped_operations.reverse();
    let drop_up_to = dropped_operations.first().copied();
    if !dropped_operations.is_empty() && unused {
        world
            .get_resource_or_insert_with(UnhandledErrors::default)
            .unused_targets
            .push(UnusedTargetDrop {
                unused_target: target,
                dropped_operations,
            });
    }

    let cancellation = Cancellation::target_dropped(target);

    if let Some(progress) = reached_drop {
        let is_cleaning = clean_series_from_progress_point(series, progress, &cancellation, world, roster)?;
        if is_cleaning {
            // Return and wait to receive the cleanup notification from the operation
            return Ok(());
        }

        // We are dropping up to the current progress point and it doesn't need
        // to be cleaned, so we should just proceed with cancelling the series.
        return finalize_series_cancel(series, cancellation, world, roster);
    }

    // If we reach this point, the series is still making progress through a
    // detached part of its sequence, so we need to let that keep running and
    // only cancel after that part of the series finishes.
    if let Some(new_final) = drop_up_to {
        // We only dropped up to a specific point, which has not yet been reached
        // by the series. We should change the status of the session so that it
        // automatically gets cancelled when it reaches this point.
        *world.get_mut::<SessionStatus>(series).or_broken()? = SessionStatus::Dropped {
            stop_at: new_final,
            cancellation,
        };
        return Ok(());
    };

    // If we reach this point, the series has not even started to run, so we
    // should just cancel it immediately.
    finalize_series_cancel(series, cancellation, world, roster)
}
