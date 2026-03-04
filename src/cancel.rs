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

use bevy_ecs::prelude::{Bundle, Component, Entity, World};

use backtrace::Backtrace;

use thiserror::Error as ThisError;

use std::{fmt::Display, sync::Arc};

use smallvec::smallvec;

use crate::{
    CancelFailure, DisplayDebugSlice, Disposal, Filtered, OperationError, OperationResult,
    OperationRoster, Supplanted, UnhandledErrors, RouteSource, RequestId,
    SessionOfScope, RouteTarget, InScope, InSeries,
    ScopeEndpoints, OrBroken, ManageInput, Routing,
};

#[cfg(feature = "trace")]
use crate::{SessionEvent, TracedEvent};

/// Information about the cancellation that occurred.
#[derive(ThisError, Debug, Clone)]
#[error("A workflow or a request was cancelled")]
pub struct Cancellation {
    /// The cause of a cancellation
    pub cause: Arc<CancellationCause>,
    /// Cancellations that occurred within cancellation workflows that were
    /// triggered by this cancellation.
    pub while_cancelling: Vec<Cancellation>,
}

impl Cancellation {
    pub fn from_cause(cause: CancellationCause) -> Self {
        Self {
            cause: Arc::new(cause),
            while_cancelling: Default::default(),
        }
    }

    pub fn target_dropped(target: Entity) -> Self {
        CancellationCause::TargetDropped(target).into()
    }

    pub fn unreachable(scope: Entity, session: Entity, disposals: Vec<Disposal>) -> Self {
        Unreachability {
            scope,
            session,
            disposals,
        }
        .into()
    }

    pub fn filtered(filtered_at_node: Entity, reason: Option<anyhow::Error>) -> Self {
        Filtered {
            filtered_at_node,
            reason,
        }
        .into()
    }

    pub fn triggered(cancelled_at_node: Entity, value: Option<String>) -> Self {
        TriggeredCancellation {
            cancelled_at_node,
            value,
        }
        .into()
    }

    pub fn supplanted(
        supplanted_by: RequestId,
    ) -> Self {
        Supplanted { supplanted_by }.into()
    }

    pub fn invalid_span(from_point: Entity, to_point: Option<Entity>) -> Self {
        InvalidSpan {
            from_point,
            to_point,
        }
        .into()
    }

    pub fn circular_collect(conflicts: Vec<[Entity; 2]>) -> Self {
        CircularCollect { conflicts }.into()
    }

    pub fn undeliverable() -> Self {
        CancellationCause::Undeliverable.into()
    }
}

impl From<Cancellation> for Arc<dyn std::error::Error + Send + Sync + 'static> {
    fn from(value: Cancellation) -> Self {
        Arc::new(value)
    }
}

impl<T: Into<CancellationCause>> From<T> for Cancellation {
    fn from(value: T) -> Self {
        Cancellation {
            cause: Arc::new(value.into()),
            while_cancelling: Default::default(),
        }
    }
}

/// Get an explanation for why a cancellation occurred.
#[derive(ThisError, Debug)]

pub enum CancellationCause {
    /// The promise taken by the requester was dropped without being detached.
    #[error("the promise taken by the requester was dropped without being detached: {:?}", .0)]
    TargetDropped(Entity),

    /// There are no terminating nodes for the workflow that can be reached
    /// anymore.
    #[error("{}", .0)]
    Unreachable(Unreachability),

    /// A filtering node has triggered a cancellation.
    #[error("{}", .0)]
    Filtered(Filtered),

    /// The workflow triggered its own cancellation.
    #[error("{}", .0)]
    Triggered(TriggeredCancellation),

    /// Some workflows will queue up requests to deliver them one at a time.
    /// Depending on the label of the incoming requests, a new request might
    /// supplant an earlier one, causing the earlier request to be cancelled.
    #[error("{}", .0)]
    Supplanted(Supplanted),

    /// An operation that acts on nodes within a workflow was given an invalid
    /// span to operate on.
    #[error("{}", .0)]
    InvalidSpan(InvalidSpan),

    /// There is a circular dependency between two or more collect operations.
    /// This will lead to problems with calculating reachability within the
    /// workflow and is likely to make the collect operations fail to behave as
    /// intended.
    ///
    /// If you need to have collect operations happen in a cycle, you can avoid
    /// this automatic cancellation by putting one or more of the offending
    /// collect operations into a scope that excludes the other collect
    /// operations while including the branches that it needs to collect from.
    #[error("{}", .0)]
    CircularCollect(CircularCollect),

    /// A request became undeliverable because the sender was dropped. This may
    /// indicate that a critical entity within a workflow was manually despawned.
    /// Check to make sure that you are not manually despawning anything that
    /// you shouldn't.
    #[error("request become undeliverable")]
    Undeliverable,

    /// A promise can never be delivered because the mutex inside of a [`Promise`][1]
    /// was poisoned.
    ///
    /// [1]: crate::Promise
    #[error("mutex poisoned inside of a promise")]
    PoisonedMutexInPromise,

    /// A node in the workflow was broken, for example despawned or missing a
    /// component. This type of cancellation indicates that you are modifying
    /// the entities in a workflow in an unsupported way. If you believe that
    /// you are not doing anything unsupported then this could indicate a bug in
    /// `crossflow` itself, and you encouraged to open an issue with a minimal
    /// reproducible example.
    ///
    /// The entity provided in [`Broken`] is the link where the breakage was
    /// detected.
    #[error("{}", .0)]
    Broken(Broken),
}

/// A variant of [`CancellationCause`]
#[derive(ThisError, Debug)]
pub struct TriggeredCancellation {
    /// The cancellation node that was triggered.
    pub cancelled_at_node: Entity,
    /// The value that triggered the cancellation, if one was provided.
    pub value: Option<String>,
}

impl Display for TriggeredCancellation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cancellation triggered at node [{:?}]",
            self.cancelled_at_node
        )?;
        if let Some(value) = &self.value {
            write!(f, " with value [{}]", value)?;
        } else {
            write!(f, " [no value mentioned]")?;
        }
        Ok(())
    }
}

impl From<TriggeredCancellation> for CancellationCause {
    fn from(value: TriggeredCancellation) -> Self {
        CancellationCause::Triggered(value)
    }
}

impl From<Filtered> for CancellationCause {
    fn from(value: Filtered) -> Self {
        CancellationCause::Filtered(value)
    }
}

impl From<Supplanted> for CancellationCause {
    fn from(value: Supplanted) -> Self {
        CancellationCause::Supplanted(value)
    }
}

#[derive(ThisError, Debug, Clone)]
pub struct Broken {
    pub node: Entity,
    pub backtrace: Option<Backtrace>,
}

impl From<Broken> for CancellationCause {
    fn from(value: Broken) -> Self {
        CancellationCause::Broken(value)
    }
}

impl Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation [{:?}] is broken", self.node)?;
        if let Some(backtrace) = &self.backtrace {
            write!(f, " at\n{backtrace:#?}")?;
        } else {
            write!(f, " [backtrace not given]")?;
        }

        Ok(())
    }
}

/// Input argument for asking a sesion or operation to cancel
pub struct Cancel<'a> {
    /// The target of the cancellation
    pub target: Entity,
    /// A specific session which is being cancelled for the target. If left
    /// blank, cancel all activity for the target.
    pub session: Option<Entity>,
    /// Information about why a cancellation is happening
    pub cancellation: Cancellation,
    pub world: &'a mut World,
    pub roster: &'a mut OperationRoster,
}

impl<'a> Cancel<'a> {
    pub fn for_target(self, target: Entity) -> Cancel<'a> {
        Cancel {
            target,
            ..self
        }
    }
}

/// A variant of [`CancellationCause`]
#[derive(ThisError, Debug)]
pub struct Unreachability {
    /// The ID of the scope whose termination became unreachable.
    pub scope: Entity,
    /// The ID of the session whose termination became unreachable.
    pub session: Entity,
    /// A list of the disposals that occurred for this session.
    pub disposals: Vec<Disposal>,
}

impl Unreachability {
    pub fn new(scope: Entity, session: Entity, disposals: Vec<Disposal>) -> Self {
        Self {
            scope,
            session,
            disposals,
        }
    }
}

impl From<Unreachability> for CancellationCause {
    fn from(value: Unreachability) -> Self {
        CancellationCause::Unreachable(value)
    }
}

impl Display for Unreachability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.disposals.len() == 1 {
            write!(f, "termination node cannot be reached after 1 disposal:")?;
        } else {
            write!(
                f,
                "termination node cannot be reached after {} disposals:",
                self.disposals.len()
            )?;
        }
        for disposal in &self.disposals {
            write!(f, "\n - {}", disposal.cause)?;
        }
        Ok(())
    }
}

/// A variant of [`CancellationCause`]
#[derive(ThisError, Debug)]
#[error("unable to calculate span from [{:?}] to [{:?}]", .from_point, .to_point)]
pub struct InvalidSpan {
    /// The starting point of the span
    pub from_point: Entity,
    /// The ending point of the span
    pub to_point: Option<Entity>,
}

impl From<InvalidSpan> for CancellationCause {
    fn from(value: InvalidSpan) -> Self {
        CancellationCause::InvalidSpan(value)
    }
}

/// A variant of [`CancellationCause`]
#[derive(ThisError, Debug)]
#[error("a circular collect exists for:{}", DisplayDebugSlice(.conflicts))]
pub struct CircularCollect {
    pub conflicts: Vec<[Entity; 2]>,
}

impl From<CircularCollect> for CancellationCause {
    fn from(value: CircularCollect) -> Self {
        CancellationCause::CircularCollect(value)
    }
}

pub trait ManageCancellation {
    /// Have a workflow operation emit a signal to cancel a session of a scope.
    ///
    /// Note: session_to_cancel is intentionally a separate argument from the
    /// session inside the RouteSource. In many cases they will be the same, but
    /// it is possible for an operation from a different session to cancel the
    /// session of a scope, so we must allow these two session values to be
    /// defined separately.
    fn emit_scope_cancel(
        &mut self,
        source: RouteSource,
        session_to_cancel: Entity,
        cancellation: Cancellation,
        roster: &mut OperationRoster,
    );

    /// Notify an operation within a series that the series is being cancelled.
    fn emit_series_cancel(
        &mut self,
        source: RouteSource,
        session_to_cancel: Entity,
        cancellation: Cancellation,
        roster: &mut OperationRoster,
    );

    /// Force a session to cancel. This is for downstream users to externally
    /// impose a cancellation on a session. For example, if a user cancels a
    /// task, this could be used to force the execution of that task to shut
    /// down.
    fn cancel_session(
        &mut self,
        session: Entity,
        cancellation: Cancellation,
        roster: &mut OperationRoster,
    );

    fn emit_broken(
        &mut self,
        broken_id: Entity,
        backtrace: Option<Backtrace>,
        roster: &mut OperationRoster,
    );
}

impl ManageCancellation for World {
    fn emit_scope_cancel(
        &mut self,
        source: RouteSource,
        session_to_cancel: Entity,
        cancellation: Cancellation,
        roster: &mut OperationRoster,
    ) {
        if let Err(error) = try_emit_scope_cancel(source, session_to_cancel, cancellation.clone(), self, roster) {
            let RouteSource { session, source, seq, .. } = source;
            self.get_resource_or_init::<UnhandledErrors>()
                .cancellations
                .push(CancelFailure {
                    error,
                    source: Some(RequestId { session, source, seq }),
                    target_to_cancel: session_to_cancel,
                    session_to_cancel: Some(session_to_cancel),
                    cancellation,
                });
        }
    }

    fn emit_series_cancel(
        &mut self,
        source: RouteSource,
        session_to_cancel: Entity,
        cancellation: Cancellation,
        roster: &mut OperationRoster,
    ) {
        cancel_operation(Some(source), session_to_cancel, Some(session_to_cancel), cancellation, self, roster);
    }

    fn cancel_session(
        &mut self,
        session: Entity,
        cancellation: Cancellation,
        roster: &mut OperationRoster,
    ) {
        cancel_operation(None, session, Some(session), cancellation, self, roster);
    }

    fn emit_broken(
        &mut self,
        broken_id: Entity,
        backtrace: Option<Backtrace>,
        roster: &mut OperationRoster,
    ) {
        let broken = Broken {
            node: broken_id,
            backtrace,
        };

        // Always put cases of broken structure into the unhandled error log.
        self.get_resource_or_init::<UnhandledErrors>()
            .broken
            .push(broken.clone());

        #[cfg(feature = "trace")]
        {
            TracedEvent::trace(broken.clone(), self);
        }

        // A broken operation could leave an Outcome hanging indefinitely, waiting
        // for a response that will never come. To mitigate this risk, we will try
        // to issue cancellation signals to any scope or series associated with the
        // broken operation. Unfortunately we cannot necessarily isolate a specific
        // session to cancel because we don't know what sessions are affected by
        // the brokenness.
        if let Some(scope) = self.get::<InScope>(broken_id).map(|s| s.scope()) {
            // The broken operation is within a scope, so cancel the whole scope.
            cancel_operation(None, scope, None, broken.into(), self, roster);
        } else if let Some(series) = self.get::<InSeries>(broken_id).map(|s| s.series()) {
            // The broken operation is within a series, so cancel the whole series.
            cancel_operation(None, series, Some(series), broken.into(), self, roster);
        } else {
            // The broken operation is not within a series or a scope, so maybe
            // it is a series or scope itself. Try cancelling it directly.
            cancel_operation(None, broken_id, Some(broken_id), broken.into(), self, roster);
        }
    }
}

fn try_emit_scope_cancel(
    source: RouteSource,
    session_to_cancel: Entity,
    cancellation: Cancellation,
    world: &mut World,
    roster: &mut OperationRoster,
) -> Result<(), OperationError> {
    // Workflow scopes are cancelled by sending a `Cancellation` input to the
    // `cancel_scope` endpoint of the scope.
    let scope = world.get::<SessionOfScope>(session_to_cancel).or_broken()?.scope();
    let cancel_scope = world.get::<ScopeEndpoints>(scope).or_broken()?.cancel_scope;
    let route = Routing {
        outputs: smallvec![source],
        input: RouteTarget {
            session: session_to_cancel,
            target: cancel_scope,
        },
    };
    world.give_input(route, cancellation, roster)
}

fn cancel_operation(
    source: Option<RouteSource>,
    target: Entity,
    session: Option<Entity>,
    cancellation: Cancellation,
    world: &mut World,
    roster: &mut OperationRoster,
) {
    #[cfg(feature = "trace")]
    {
        if let Some(session) = session {
            SessionEvent::cancelled(source, session, cancellation.clone(), world);
        }
    }

    match world.get::<OnCancel>(target).map(|c| c.0).or_broken() {
        Ok(on_cancel) => {
            if let Err(OperationError::Broken(backtrace)) = on_cancel(Cancel {
                target: target,
                session: session,
                cancellation,
                world,
                roster,
            }) {
                world.emit_broken(target, backtrace, roster);
            }
        }
        Err(error) => {
            world.get_resource_or_init::<UnhandledErrors>()
                .cancellations
                .push(CancelFailure {
                    error,
                    source: source.map(|s| s.request_id()),
                    target_to_cancel: target,
                    session_to_cancel: session,
                    cancellation,
                });
        }
    }
}

#[derive(Component)]
pub(crate) struct OnCancel(pub(crate) fn(Cancel) -> OperationResult);

#[derive(Bundle)]
pub struct Cancellable {
    cancel: OnCancel,
}

impl Cancellable {
    pub fn new(cancel: fn(Cancel) -> OperationResult) -> Self {
        Cancellable {
            cancel: OnCancel(cancel),
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Cancellation {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        CancellationCause::Undeliverable.into()
    }
}
