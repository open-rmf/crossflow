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
    prelude::{Component, Entity, World, Children},
    world::{EntityRef, EntityWorldMut},
};

use backtrace::Backtrace;

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    sync::Arc,
};

use smallvec::SmallVec;

use thiserror::Error as ThisError;

use crate::{
    Cancel, Cancellation, DisposalFailure, OperationResult, OperationRoster, OrBroken, OutputPort,
    SeriesMarker, UnhandledErrors, UnusedTarget, operation::ScopeStorage, RequestId, ManageCancellation, RouteSource,
};

#[cfg(feature = "trace")]
use crate::OutputDisposed;

#[derive(ThisError, Debug, Clone)]
#[error("the output of an operation in a workflow was disposed: {}", .cause)]
pub struct Disposal {
    pub cause: Arc<DisposalCause>,
}

impl<T: Into<DisposalCause>> From<T> for Disposal {
    fn from(value: T) -> Self {
        Disposal {
            cause: Arc::new(value.into()),
        }
    }
}

impl Disposal {
    pub fn service_unavailable(service: Entity, for_node: Entity) -> Disposal {
        ServiceUnavailable { service, for_node }.into()
    }

    pub fn task_despawned(task: Entity, node: Entity) -> Disposal {
        TaskDespawned { task, node }.into()
    }

    pub fn branching(
        branched_at_node: Entity,
        disposed_for_target: Entity,
        reason: Option<anyhow::Error>,
    ) -> Disposal {
        DisposedBranch {
            branched_at_node,
            disposed_for_target,
            reason,
        }
        .into()
    }

    pub fn buffer_key(accessor_node: Entity, key_for_buffer: Entity) -> Disposal {
        DisposedBufferKey {
            accessor_node,
            key_for_buffer,
        }
        .into()
    }

    pub fn supplanted(
        supplanted_by: RequestId,
    ) -> Self {
        Supplanted { supplanted_by }.into()
    }

    pub fn async_node_with_streams() -> Self {
        Self {
            cause: Arc::new(DisposalCause::AsyncNodeWithStreams),
        }
    }

    pub fn filtered(filtered_at_node: Entity, reason: Option<anyhow::Error>) -> Self {
        Filtered {
            filtered_at_node,
            reason,
        }
        .into()
    }

    pub fn trimming(trimmer: Entity, nodes: SmallVec<[Entity; 16]>) -> Self {
        Trimming { trimmer, nodes }.into()
    }

    pub fn closed_gate(gate_node: Entity, closed_buffers: SmallVec<[Entity; 8]>) -> Self {
        ClosedGate {
            gate_node,
            closed_buffers,
        }
        .into()
    }

    pub fn empty_spread(spread_node: Entity) -> Self {
        EmptySpread { spread_node }.into()
    }

    pub fn deficient_collection(collect_node: Entity, min: usize, actual: usize) -> Self {
        DeficientCollection {
            collect_node,
            min,
            actual,
        }
        .into()
    }

    pub fn incomplete_split(
        split_node: Entity,
        missing_keys: SmallVec<[Option<Arc<str>>; 16]>,
    ) -> Self {
        IncompleteSplit {
            split_node,
            missing_keys,
        }
        .into()
    }
}

#[derive(ThisError, Debug)]
pub enum DisposalCause {
    /// Some services will queue up requests to deliver them one at a time.
    /// Depending on the label of the incoming requests, a new request might
    /// supplant an earlier one, causing the earlier request to be disposed.
    #[error("{}", .0)]
    Supplanted(Supplanted),

    /// A node filtered out a response.
    #[error("{}", .0)]
    Filtered(Filtered),

    /// A node disposed of one of its output branches.
    #[error("{}", .0)]
    Branching(DisposedBranch),

    /// A buffer key was disposed, so a buffer will no longer be able to update.
    #[error("{}", .0)]
    BufferKey(DisposedBufferKey),

    /// A [`Service`](crate::Service) provider needed by the chain was despawned
    /// or had a critical component removed. The entity provided in the variant
    /// is the unavailable service.
    #[error("{}", .0)]
    ServiceUnavailable(ServiceUnavailable),

    /// An entity that was managing the execution of a task was despawned,
    /// causing the task to be cancelled and making it impossible to deliver a
    /// response.
    #[error("{}", .0)]
    TaskDespawned(TaskDespawned),

    /// An output was disposed because a mutex was poisoned.
    #[error("{}", .0)]
    PoisonedMutex(PoisonedMutexDisposal),

    /// A scope was cancelled so its output has been disposed.
    #[error("{}", .0)]
    Scope(Cancellation),

    /// A stream from a node never emitted any signal. This can cause some
    /// branches of the workflow to become unreachable, so we consider it a
    /// disposal event.
    #[error("{}", .0)]
    UnusedStreams(UnusedStreams),

    /// Whenever an async node with streams finishes running we need to do a
    /// reachability check because it's possible that an earlier reachability
    /// check was depending on the possibility that this node would eventually
    /// produce one of its streams.
    #[error("An async node with streams finished running")]
    AsyncNodeWithStreams,

    /// Some nodes in the workflow were trimmed.
    #[error("{}", .0)]
    Trimming(Trimming),

    /// A gate was closed, which cut off the ability of a workflow to proceed.
    #[error("{}", .0)]
    ClosedGate(ClosedGate),

    /// A spread operation was given an empty collection so there was nothing to
    /// spread. As a result, no signal was sent out of the node after it
    /// received a signal.
    #[error("{}", .0)]
    EmptySpread(EmptySpread),

    /// A collect operation has a minimum number of entries, and it appears the
    /// workflow will not be able to meet that minimum, so a disposal notice has
    /// been sent out to indicate that the workflow is blocked up on the
    /// collection.
    #[error("{}", .0)]
    DeficientCollection(DeficientCollection),

    /// A split operation took place, but not all connections to the split
    /// received a value.
    #[error("{}", .0)]
    IncompleteSplit(IncompleteSplit),
}

impl DisposalCause {
    pub fn stream_disposal(&self) -> bool {
        matches!(self, Self::UnusedStreams(_) | Self::AsyncNodeWithStreams)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug, Clone, Copy)]
#[error("request was supplanted")]
pub struct Supplanted {
    /// ID of the request that did the supplanting
    pub supplanted_by: RequestId,
}

impl From<Supplanted> for DisposalCause {
    fn from(value: Supplanted) -> Self {
        DisposalCause::Supplanted(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
pub struct Filtered {
    /// ID of the node that did the filtering
    pub filtered_at_node: Entity,
    /// Optionally, a reason given for why the filtering happened.
    pub reason: Option<anyhow::Error>,
}

impl Filtered {
    pub fn new(filtered_at_node: Entity, reason: Option<anyhow::Error>) -> Self {
        Self {
            filtered_at_node,
            reason,
        }
    }
}

impl From<Filtered> for DisposalCause {
    fn from(value: Filtered) -> Self {
        Self::Filtered(value)
    }
}

impl Display for Filtered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "filtered at node [{:?}]", self.filtered_at_node)?;
        if let Some(reason) = &self.reason {
            write!(f, ": {}", reason)?;
        } else {
            write!(f, " [no reason given]")?;
        }
        Ok(())
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
pub struct DisposedBranch {
    /// The node where the branching happened
    pub branched_at_node: Entity,
    /// The target node whose input was disposed
    pub disposed_for_target: Entity,
    /// Optionally, a reason given for the branching
    pub reason: Option<anyhow::Error>,
}

impl From<DisposedBranch> for DisposalCause {
    fn from(value: DisposedBranch) -> Self {
        Self::Branching(value)
    }
}

impl Display for DisposedBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "branch [{:?}] -> [{:?}] disposed",
            self.branched_at_node, self.disposed_for_target,
        )?;
        if let Some(reason) = &self.reason {
            write!(f, ": {}", reason)?;
        } else {
            write!(f, " [no reason given]")?;
        }

        Ok(())
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("buffer key disposed")]
pub struct DisposedBufferKey {
    pub accessor_node: Entity,
    pub key_for_buffer: Entity,
}

impl From<DisposedBufferKey> for DisposalCause {
    fn from(value: DisposedBufferKey) -> Self {
        Self::BufferKey(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("service [{:?}] no longer available for node [{:?}]", .service, .for_node)]
pub struct ServiceUnavailable {
    /// The service that is no longer available
    pub service: Entity,
    /// The node that intended to use the service
    pub for_node: Entity,
}

impl From<ServiceUnavailable> for DisposalCause {
    fn from(value: ServiceUnavailable) -> Self {
        Self::ServiceUnavailable(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("task [{:?}] despawned for node [{:?}]", .task, .node)]
pub struct TaskDespawned {
    /// The entity that was managing the task
    pub task: Entity,
    /// The node that the task was spawned by
    pub node: Entity,
}

impl From<TaskDespawned> for DisposalCause {
    fn from(value: TaskDespawned) -> Self {
        Self::TaskDespawned(value)
    }
}
/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("poisoned mutex in node [{:?}]", .for_node)]
pub struct PoisonedMutexDisposal {
    /// The node containing the poisoned mutex
    pub for_node: Entity,
}

impl From<PoisonedMutexDisposal> for DisposalCause {
    fn from(value: PoisonedMutexDisposal) -> Self {
        Self::PoisonedMutex(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("streams unused for a request [{:?}]:{}", .request_id, DisplaySlice(.streams))]
pub struct UnusedStreams {
    /// The node which did not use all its streams
    pub request_id: RequestId,
    /// The streams which went unused.
    pub streams: Vec<&'static str>,
}

impl UnusedStreams {
    pub fn new(request_id: RequestId) -> Self {
        Self {
            request_id,
            streams: Default::default(),
        }
    }
}

impl From<UnusedStreams> for DisposalCause {
    fn from(value: UnusedStreams) -> Self {
        Self::UnusedStreams(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("nodes trimmed by [{:?}]:{}", .trimmer, DisplayDebugSlice(.nodes))]
pub struct Trimming {
    pub trimmer: Entity,
    pub nodes: SmallVec<[Entity; 16]>,
}

impl From<Trimming> for DisposalCause {
    fn from(value: Trimming) -> Self {
        Self::Trimming(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("gate [{:?}] closed buffers:{}", .gate_node, DisplayDebugSlice(.closed_buffers))]
pub struct ClosedGate {
    /// The gate node which triggered the closing
    pub gate_node: Entity,
    /// The buffers which were closed by the gate node
    pub closed_buffers: SmallVec<[Entity; 8]>,
}

impl From<ClosedGate> for DisposalCause {
    fn from(value: ClosedGate) -> Self {
        Self::ClosedGate(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("spread operation [{:?}] had an empty collection", .spread_node)]
pub struct EmptySpread {
    /// The node that was doing the spreading
    pub spread_node: Entity,
}

impl From<EmptySpread> for DisposalCause {
    fn from(value: EmptySpread) -> Self {
        Self::EmptySpread(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("collect operation [{:?}] needed {} items but ran out at {}", .collect_node, .min, .actual)]
pub struct DeficientCollection {
    /// The node that is doing the collection
    pub collect_node: Entity,
    /// The minimum required size of the collection
    pub min: usize,
    /// The actual size of the collection when it became unreachable
    pub actual: usize,
}

impl From<DeficientCollection> for DisposalCause {
    fn from(value: DeficientCollection) -> Self {
        Self::DeficientCollection(value)
    }
}

/// A variant of [`DisposalCause`]
#[derive(ThisError, Debug)]
#[error("split operation [{:?}] was missing items for keys:{}", .split_node, DisplayDebugSlice(.missing_keys))]
pub struct IncompleteSplit {
    /// The node that does the splitting
    pub split_node: Entity,
    /// The debug text of each key that was missing in the split
    pub missing_keys: SmallVec<[Option<Arc<str>>; 16]>,
}

impl From<IncompleteSplit> for DisposalCause {
    fn from(value: IncompleteSplit) -> Self {
        Self::IncompleteSplit(value)
    }
}

pub trait ManageDisposal {
    fn emit_disposal(
        &mut self,
        request_id: RequestId,
        port: OutputPort,
        disposal: Disposal,
        roster: &mut OperationRoster,
    );

    fn clear_disposals(&mut self, session: Entity);

    /// Used to transfer the disposals gathered by a temporary operation (e.g.
    /// a task) over to a persistent node
    fn transfer_disposals(&mut self, to_node: Entity) -> OperationResult;
}

pub trait InspectDisposals {
    fn get_disposals(&self, session: Entity) -> Option<&Vec<Disposal>>;
}

impl ManageDisposal for World {
    /// Emit a signal that an output has been disposed for a certain operation.
    fn emit_disposal(
        &mut self,
        request_id: RequestId,
        port: OutputPort,
        disposal: Disposal,
        roster: &mut OperationRoster,
    ) {
        let RequestId { session, source, seq } = request_id;
        #[cfg(feature = "trace")]
        {
            // TODO(@mxgrey): Consider not tracing stream-related disposals
            // since that could produce a lot of useless noise.
            OutputDisposed::trace(
                RouteSource { session, source, seq, port },
                disposal.clone(),
                self,
            );
        }

        if let Some(scope) = self.get::<ScopeStorage>(source) {
            // The source is inside of a workflow scope, so we need to notify
            // the scope that a disposal is taking place.
            let scope = scope.get();

            if let Some(mut storage) = self.get_mut::<DisposalStorage>(source) {
                storage.disposals.entry(session).or_default().push(disposal);
            } else {
                let mut storage = DisposalStorage::default();
                storage.disposals.entry(session).or_default().push(disposal);
                self.entity_mut(source).insert(storage);
            }

            roster.disposed(scope, source, session);
        } else {
            // The source is not inside a workflow scope, so we expect it to
            // either be part of a series or to be something unused.
            if self.get::<SeriesMarker>(source).is_some() {
                // The disposal happened for an operation in a series. If the
                // operation cannot be completed, then the series needs to be
                // cancelled.
                //
                // We do not convert stream disposals into a cancellation
                // because they do not affect the ability of the series to
                // reach its end.
                if !disposal.cause.stream_disposal() {
                    let cancellation = Cancellation::unreachable(session, session, vec![disposal]);
                    self.notify_series_cancel(
                        RouteSource {
                            session,
                            source,
                            seq,
                            port,
                        },
                        session,
                        cancellation,
                    );
                }
            } else if self.get::<UnusedTarget>(source).is_none() {
                // If the emitting node does not have a scope, is not part of
                // a series, and is not an unused target, then something is broken.
                //
                // We can safely ignore disposals for unused targets because
                // unused targets cannot affect the reachability of a workflow
                // or a series.
                let broken_node = request_id.source;
                self.get_resource_or_insert_with(UnhandledErrors::default)
                    .disposals
                    .push(DisposalFailure {
                        disposal,
                        broken_node,
                        backtrace: Some(Backtrace::new()),
                    });
            }
            return;
        };
    }

    fn clear_disposals(&mut self, session: Entity) {
        if let Some(mut storage) = self.get_mut::<DisposalStorage>() {
            storage.disposals.remove(&session);
        }
    }

    fn transfer_disposals(&mut self, to: Entity) -> OperationResult {
        if let Some(from_storage) = self.take::<DisposalStorage>() {
            self.world_scope::<OperationResult>(|world| {
                let mut to_mut = world.get_entity_mut(to).or_broken()?;
                match to_mut.get_mut::<DisposalStorage>() {
                    Some(mut to_storage) => {
                        for (session, disposals) in from_storage.disposals {
                            to_storage
                                .disposals
                                .entry(session)
                                .or_default()
                                .extend(disposals);
                        }
                    }
                    None => {
                        to_mut.insert(from_storage);
                    }
                }
                Ok(())
            })?;
        }

        Ok(())
    }
}

impl<'w> InspectDisposals for EntityWorldMut<'w> {
    fn get_disposals(&self, session: Entity) -> Option<&Vec<Disposal>> {
        if let Some(storage) = self.get::<DisposalStorage>() {
            return storage.disposals.get(&session);
        }

        None
    }
}

impl<'w> InspectDisposals for EntityRef<'w> {
    fn get_disposals(&self, session: Entity) -> Option<&Vec<Disposal>> {
        if let Some(storage) = self.get::<DisposalStorage>() {
            return storage.disposals.get(&session);
        }

        None
    }
}

pub fn emit_disposal(
    source: Entity,
    session: Entity,
    disposal: Disposal,
    world: &mut World,
    roster: &mut OperationRoster,
) {
    if let Ok(mut source_mut) = world.get_entity_mut(source) {
        source_mut.emit_disposal(session, disposal, roster);
    } else {
        world
            .get_resource_or_insert_with(UnhandledErrors::default)
            .disposals
            .push(DisposalFailure {
                disposal,
                broken_node: source,
                backtrace: Some(Backtrace::new()),
            });
    }
}

#[derive(Component, Default)]
struct DisposalStorage {
    /// A map from a session to all the disposals that occurred for the session
    disposals: HashMap<Entity, Vec<Disposal>>,
}

pub(crate) struct DisplaySlice<'a, T>(&'a [T]);

impl<'a, T> Display for DisplaySlice<'a, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in self.0 {
            write!(f, " {}", item)?;
        }
        Ok(())
    }
}

pub(crate) struct DisplayDebugSlice<'a, T>(pub(crate) &'a [T]);

impl<'a, T> Display for DisplayDebugSlice<'a, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in self.0 {
            write!(f, " {:?}", item)?;
        }
        Ok(())
    }
}
