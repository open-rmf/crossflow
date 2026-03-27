/*
 * Copyright (C) 2025 Open Source Robotics Foundation
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

use crate::{
    Broken, BufferKeyTag, Cancellation, Disposal, IdentifierRef, JsonMessage, OperationRef,
    OperationType, OutputKey, OutputPort, OutputRef, RequestId, RouteSource, Routing, Seq,
    TraceToggle, TypeInfo,
};

use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    prelude::{ChildOf, Command, Commands, Component, Entity, Event, Query, Res, Resource, World},
    system::SystemParam,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::{
    any::Any,
    borrow::Cow,
    collections::VecDeque,
    sync::Arc,
    time::{Instant, SystemTime},
};
use thiserror::Error as ThisError;

/// The trace toggle settings of this resource override any other trace settings.
/// This is typically used to turn on debugging in cases where tracing is not
/// normally used.
#[derive(Clone, Copy, Debug, Default, Resource, Deref, DerefMut)]
pub struct UniversalTraceToggle(Option<TraceToggle>);

impl UniversalTraceToggle {
    pub fn on() -> Self {
        Self(Some(TraceToggle::On))
    }

    pub fn with_messages() -> Self {
        Self(Some(TraceToggle::Messages))
    }
}

/// A component attached to workflow operation entities in order to trace their
/// activities.
#[derive(Component)]
pub struct Trace {
    toggle: TraceToggle,
    info: Arc<OperationInfo>,
    serialize_value: Option<fn(&dyn Any) -> Result<JsonMessage, GetValueError>>,
}

impl Trace {
    /// Create trace information for an entity. By default this will not serialize
    /// the messages passing through.
    pub fn new(toggle: TraceToggle, info: Arc<OperationInfo>) -> Self {
        Trace {
            toggle,
            info,
            serialize_value: None,
        }
    }

    /// Enable the trace for this operation to also send out the data of the
    /// messages that are passing through.
    pub fn enable_value_serialization<T: Any + Serialize>(&mut self) {
        self.serialize_value = Some(get_serialize_value::<T>);
    }

    /// Change the current tracing setting for this operation.
    pub fn set_toggle(&mut self, toggle: TraceToggle) {
        self.toggle = toggle;
    }

    /// Get the current tracing setting for this operation.
    pub fn toggle(&self) -> TraceToggle {
        self.toggle
    }

    /// Get the information for this workflow operation.
    pub fn info(&self) -> &Arc<OperationInfo> {
        &self.info
    }

    /// Attempt to serialize the value. This will return a None if the trace is
    /// not set up to serialize the values.
    pub fn serialize_value(&self, value: &dyn Any) -> TracedMessage {
        self.serialize_value.map(|f| f(value))
    }
}

fn get_serialize_value<T: Any + Serialize>(value: &dyn Any) -> Result<JsonMessage, GetValueError> {
    let Some(value_ref) = value.downcast_ref::<T>() else {
        return Err(GetValueError::FailedDowncast {
            expected: TypeInfo::of::<T>(),
            received: std::any::TypeId::of::<T>(),
        });
    };

    serde_json::to_value(value_ref).map_err(|err| GetValueError::FailedSerialization(Arc::new(err)))
}

#[derive(ThisError, Debug, Clone)]
pub enum GetValueError {
    #[error("The downcast was incompatible. Expected {expected:?}, received {received:?}")]
    FailedDowncast {
        expected: TypeInfo,
        received: std::any::TypeId,
    },
    #[error("The serialization into json failed: {0}")]
    FailedSerialization(Arc<serde_json::Error>),
}

#[derive(Debug, Clone, Component)]
pub struct OperationLabels {
    input: Arc<Vec<OperationRef>>,
    outputs: Vec<(OutputKey, Arc<Vec<OutputRef>>)>,
}

impl OperationLabels {
    pub fn input(&self) -> Arc<Vec<OperationRef>> {
        Arc::clone(&self.input)
    }

    pub fn outputs(&self, port: OutputPort) -> Option<Arc<Vec<OutputRef>>> {
        for (key, labels) in &self.outputs {
            if port.len() != key.len() {
                continue;
            }

            let mut is_match = true;
            for (lhs, rhs) in port.iter().zip(key.iter()) {
                is_match = lhs == rhs;
            }

            if is_match {
                return Some(Arc::clone(labels));
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct TraceSource {
    /// The stack of session IDs that sent the message was sent from. The first
    /// entry is the root session. Each subsequent entry is a child session of
    /// the previous. There are two common ways to get a child session:
    /// * In a series, earlier sessions in the chain are children of later
    ///   sessions in the chain, so the last session of the chain is the root of
    ///   the entire chain.
    /// * When a scope operation is triggered, a new session is created. Its parent
    ///   is the session of the message that triggered the scope operation. Every
    ///   time a workflow is triggered it creates a new scope, and therefore also
    ///   creates a child session.
    pub session_stack: SmallVec<[Entity; 8]>,
    pub source: Entity,
    pub seq: Seq,
    pub port: SmallVec<[IdentifierRef<'static>; 2]>,
    pub labels: Option<Arc<Vec<OutputRef>>>,
    /// Information about what kind of operation produced the message
    pub operation_type: Arc<str>,
    pub info: Option<Arc<OperationInfo>>,
}

impl TraceSource {
    fn new(route_source: RouteSource, world: &mut World) -> Self {
        let output_port = route_source.port;
        let session_stack = get_session_stack_from_world(route_source.session, world);
        let port = route_source.port.iter().map(|p| p.to_owned()).collect();
        let operation_type = world
            .get::<OperationType>(route_source.source)
            .map(|op| (**op).clone())
            .unwrap_or_else(|| "<unknown>".into());
        let info = world
            .get::<Trace>(route_source.source)
            .map(|t| t.info.clone());

        TraceSource {
            session_stack,
            source: route_source.source,
            seq: route_source.seq,
            port,
            labels: world
                .get::<OperationLabels>(route_source.source)
                .map(move |labels| labels.outputs(output_port))
                .flatten(),
            operation_type,
            info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceTarget {
    /// The stack of session IDs that sent the message was sent into. The first
    /// entry is the root session. Each subsequent entry is a child session of
    /// the previous. There are two common ways to get a child session:
    /// * In a series, earlier sessions in the chain are children of later
    ///   sessions in the chain, so the last session of the chain is the root of
    ///   the entire chain.
    /// * When a scope operation is triggered, a new session is created. Its parent
    ///   is the session of the message that triggered the scope operation. Every
    ///   time a workflow is triggered it creates a new scope, and therefore also
    ///   creates a child session.
    pub session_stack: SmallVec<[Entity; 8]>,
    pub target: Entity,
    pub seq: Seq,
    pub labels: Option<Arc<Vec<OperationRef>>>,
    pub operation_type: Arc<str>,
    pub info: Option<Arc<OperationInfo>>,
}

impl TraceTarget {
    fn new(request_id: RequestId, world: &mut World) -> Self {
        let RequestId {
            session,
            source,
            seq,
        } = request_id;
        let operation_type = world
            .get::<OperationType>(source)
            .map(|op| (**op).clone())
            .unwrap_or_else(|| "<unknown>".into());
        let info = world.get::<Trace>(source).map(|t| t.info.clone());

        Self {
            session_stack: get_session_stack_from_world(session, world),
            target: source,
            seq,
            labels: world
                .get::<OperationLabels>(source)
                .map(|labels| labels.input()),
            operation_type,
            info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceBuffer {
    /// The stack of session IDs of the buffer that's being accessed. The first
    /// entry is the root session. Each subsequent entry is a child session of
    /// the previous. There are two common ways to get a child session:
    /// * In a series, earlier sessions in the chain are children of later
    ///   sessions in the chain, so the last session of the chain is the root of
    ///   the entire chain.
    /// * When a scope operation is triggered, a new session is created. Its parent
    ///   is the session of the message that triggered the scope operation. Every
    ///   time a workflow is triggered it creates a new scope, and therefore also
    ///   creates a child session.
    pub session_stack: SmallVec<[Entity; 8]>,
    /// The unique ID of the buffer.
    pub id: Entity,
    pub labels: Option<Arc<Vec<OperationRef>>>,
}

pub type TracedMessage = Option<Result<JsonMessage, GetValueError>>;

/// An event that tracks when each message is sent for a request or within a
/// workflow.
#[derive(Debug, Clone, Event)]
pub struct MessageSent {
    /// Information about what output(s) the message is coming from. For most
    /// operations this will be a single output, but it may be multiple for
    /// operations where messages are converging, such as collect and join.
    pub output: SmallVec<[TraceSource; 8]>,
    /// Information about what input slot the message is going to.
    pub input: TraceTarget,
    /// The message itself, if message tracing is turned on and the message
    /// could be serialized.
    pub message: TracedMessage,
}

impl MessageSent {
    pub(crate) fn trace(
        route: Routing,
        target_seq: Seq,
        message: TracedMessage,
        world: &mut World,
    ) {
        let mut output = SmallVec::new();
        for out in route.outputs {
            output.push(TraceSource::new(out, world));
        }

        let input = TraceTarget::new(
            RequestId {
                session: route.input.session,
                source: route.input.target,
                seq: target_seq,
            },
            world,
        );

        let event = MessageSent {
            output,
            input,
            message,
        };
        world.write_trace(TracedEvent::now(event));
    }
}

/// Track which outputs of an operation did not yield any message after the
/// operation was triggered.
#[derive(Debug, Clone)]
pub struct OutputDisposed {
    /// Information about which output port did not yield any message for a
    /// request that came in.
    pub trigger: TraceSource,
    pub disposed_operation: Entity,
    pub disposed_in_session: SmallVec<[Entity; 8]>,
    pub disposal: Disposal,
}

impl OutputDisposed {
    pub(crate) fn trace(
        trigger: RouteSource,
        disposed_operation: Entity,
        disposed_in_session: Entity,
        disposal: Disposal,
        world: &mut World,
    ) {
        let tracer = MessageTracer::get_for(disposed_operation, world);
        if tracer.is_on() {
            let trigger = TraceSource::new(trigger, world);
            let disposed_in_session = get_session_stack_from_world(disposed_in_session, world);
            let event = TracedEvent::now(Self {
                trigger,
                disposed_operation,
                disposed_in_session,
                disposal,
            });
            world.write_trace(event);
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct BufferEvent {
    /// The stack of session IDs in which the buffer was accessed. The first entry
    /// is the root session. Each subsequent entry is a child session of the
    /// previous. There are two common ways to get a child session:
    /// * In a series, earlier sessions in the chain are children of later
    ///   sessions in the chain, so the last session of the chain is the root of
    ///   the entire chain.
    /// * When a scope operation is triggered, a new session is created. Its parent
    ///   is the session of the message that triggered the scope operation. Every
    ///   time a workflow is triggered it creates a new scope, and therefore also
    ///   creates a child session.
    pub accessor: TraceTarget,
    pub buffer: TraceBuffer,
    pub access: BufferAccessRecord,
}

#[derive(Debug, Clone)]
pub enum BufferAccessRecord {
    Viewed,
    Modified(BufferModification),
    Pushed(BufferPush),
    Removed(BufferRemoval),
}

impl BufferAccessRecord {
    pub fn is_viewed(&self) -> bool {
        matches!(self, Self::Viewed)
    }

    pub fn modified(&self) -> Option<&BufferModification> {
        match self {
            Self::Modified(modified) => Some(modified),
            _ => None,
        }
    }

    pub fn pushed(&self) -> Option<&BufferPush> {
        match self {
            Self::Pushed(pushed) => Some(pushed),
            _ => None,
        }
    }

    pub fn removed(&self) -> Option<&BufferRemoval> {
        match self {
            Self::Removed(removed) => Some(removed),
            _ => None,
        }
    }
}

/// A record of a modification performed on a buffer.
#[derive(Debug, Clone)]
pub struct BufferModification {
    /// The sequence number of the message within the buffer. This is unique
    /// across all sessions for each new item that is added to a buffer. The
    /// sequence number does not necessarily reflect the positioning of the
    /// messages with the buffer because users are allowed to insert new messages
    /// into arbitary positions.
    pub seq: Seq,
    /// The original message before the modification.
    pub original: TracedMessage,
    /// The new message after the modification.
    pub modified: TracedMessage,
}

/// A record of a message being pushed into a buffer.
#[derive(Debug, Clone)]
pub struct BufferPush {
    /// The sequence number of the message within the buffer. This is unique
    /// across all sessions for each new item that is added to a buffer. The
    /// sequence number does not necessarily reflect the positioning of the
    /// messages with the buffer because users are allowed to insert new messages
    /// into arbitary positions.
    pub seq: Seq,
    /// The position that the message was placed within the buffer for this session.
    pub position: usize,
    /// The message that was pushed.
    pub message: TracedMessage,
}

/// A record of a message being removed from a buffer.
#[derive(Debug, Clone)]
pub struct BufferRemoval {
    /// The sequence number of the message within the buffer. This is unique
    /// across all sessions for each new item that is added to a buffer. The
    /// sequence number does not necessarily reflect the positioning of the
    /// messages with the buffer because users are allowed to insert new messages
    /// into arbitary positions.
    pub seq: Seq,
}

/// This system param is used by the `BufferManager` to trace the buffer activity.
#[derive(SystemParam)]
pub(crate) struct BufferTracer<'w, 's> {
    trace: Query<'w, 's, &'static Trace>,
    child_of: Query<'w, 's, &'static ChildOf>,
    labels: Query<'w, 's, &'static OperationLabels>,
    op_type: Query<'w, 's, &'static OperationType>,
    universal: Option<Res<'w, UniversalTraceToggle>>,
    pub(crate) commands: Commands<'w, 's>,
}

/// This is a small wrapper of the minimal borrows needed to trace modifications
/// on a single item in a buffer. This is used by BMut.
#[derive(Clone, Copy)]
pub(crate) struct MessageTracer<'a> {
    trace: Option<&'a Trace>,
    universal: Option<&'a TraceToggle>,
}

impl<'a> MessageTracer<'a> {
    pub(crate) fn get_for(op: Entity, world: &'a World) -> Self {
        let trace = world.get::<Trace>(op);
        let universal = world
            .get_resource::<UniversalTraceToggle>()
            .map(|u| u.0.as_ref())
            .flatten();
        Self { trace, universal }
    }

    pub(crate) fn is_on(&self) -> bool {
        if let Some(toggle) = self.universal {
            return toggle.is_on();
        }

        self.trace.is_some_and(|t| t.toggle().is_on())
    }

    pub(crate) fn trace_message(&self, value: &dyn Any) -> TracedMessage {
        let Some(toggle) = self.universal.or_else(|| self.trace.map(|t| &t.toggle)) else {
            return None;
        };

        if toggle.with_messages()
            && let Some(trace) = self.trace
        {
            trace.serialize_value(value)
        } else {
            None
        }
    }
}

impl<'w, 's> BufferTracer<'w, 's> {
    pub(crate) fn trace(&mut self, req: RequestId, key: &BufferKeyTag, access: BufferAccessRecord) {
        let toggle = self.get_trace_toggle(key);
        if !toggle.is_on() {
            return;
        }

        let accessor = self.get_trace_target(req);
        let buffer = self.get_trace_buffer(key);
        let buffer_event = BufferEvent {
            accessor,
            buffer,
            access,
        };

        self.commands.write_trace(TracedEvent::now(buffer_event));
    }

    pub(crate) fn get_message_tracer(&self, key: &BufferKeyTag) -> MessageTracer<'_> {
        let trace = self.trace.get(key.buffer).ok();
        let universal = self.universal.as_ref().map(|u| u.0.as_ref()).flatten();
        MessageTracer { trace, universal }
    }

    pub(crate) fn get_trace_toggle(&self, key: &BufferKeyTag) -> TraceToggle {
        if let Some(universal) = self.universal.as_ref().map(|u| ***u).flatten() {
            universal
        } else if let Ok(buffer_trace) = self.trace.get(key.buffer) {
            buffer_trace.toggle
        } else {
            TraceToggle::Off
        }
    }

    pub(crate) fn get_trace_buffer(&self, key: &BufferKeyTag) -> TraceBuffer {
        let session_stack = get_session_stack(key.session, &self.child_of);
        let labels = self.labels.get(key.buffer).ok().map(|l| l.input.clone());
        TraceBuffer {
            session_stack,
            id: key.buffer,
            labels: labels,
        }
    }

    pub(crate) fn get_trace_target(&self, req: RequestId) -> TraceTarget {
        let labels = self.labels.get(req.source).ok().map(|l| l.input.clone());
        let session_stack = get_session_stack(req.session, &self.child_of);
        let operation_type = self
            .op_type
            .get(req.source)
            .map(|op| (**op).clone())
            .unwrap_or_else(|_| "<unknown>".into());
        let info = self.trace.get(req.source).ok().map(|t| t.info.clone());

        TraceTarget {
            session_stack,
            target: req.source,
            seq: req.seq,
            labels,
            operation_type,
            info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub session_stack: SmallVec<[Entity; 8]>,
    pub change: SessionChange,
}

impl SessionEvent {
    pub(crate) fn spawned(
        scope_request: Option<RequestId>,
        spawned_session: Entity,
        world: &mut World,
    ) {
        let session_stack = get_session_stack_from_world(spawned_session, world);
        let scope = scope_request.map(|s| TraceTarget::new(s, world));
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Spawned { scope },
        };

        world.write_trace(TracedEvent::now(event));
    }

    pub(crate) fn despawned(session: Entity, world: &mut World) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Despawned,
        };

        world.write_trace(TracedEvent::now(event));
    }

    pub(crate) fn cancelled(
        source: Option<RouteSource>,
        session: Entity,
        cancellation: Cancellation,
        world: &mut World,
    ) {
        let source = source.map(|source| TraceSource::new(source, world));
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Cancelled {
                source,
                cancellation,
            },
        };

        world.write_trace(TracedEvent::now(event));
    }

    pub(crate) fn paused_by_user(session: Entity, world: &mut World) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Paused(PauseCause::UserRequest),
        };

        world.write_trace(TracedEvent::now(event));
    }

    pub(crate) fn paused_by_breakpoint(session: Entity, breakpoint: Entity, world: &mut World) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Paused(PauseCause::Breakpoint(breakpoint)),
        };

        world.write_trace(TracedEvent::now(event));
    }

    pub(crate) fn unpaused(session: Entity, world: &mut World) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Unpaused,
        };

        world.write_trace(TracedEvent::now(event));
    }

    pub(crate) fn cleanup(session: Entity, world: &mut World) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::BeginCleanup,
        };

        world.write_trace(TracedEvent::now(event));
    }
}

#[derive(Debug, Clone)]
pub enum SessionChange {
    Spawned {
        scope: Option<TraceTarget>,
    },
    Terminated {
        source: TraceSource,
    },
    Cancelled {
        /// What operation triggered the cancellation
        source: Option<TraceSource>,
        /// Summary of the cancellation cause
        cancellation: Cancellation,
    },
    BeginCleanup,
    Despawned,
    Paused(PauseCause),
    Unpaused,
}

#[derive(Debug, Clone)]
pub enum PauseCause {
    UserRequest,
    Breakpoint(Entity),
}

#[derive(Debug, Clone)]
pub enum TracedEventKind {
    /// A message was sent from one operation to another
    MessageSent(MessageSent),
    /// A buffer was viewed or modified by an operation
    BufferEvent(BufferEvent),
    /// A session was spawned despawned, or changed state
    SessionEvent(SessionEvent),
    /// An output from an operation was disposed or never activated
    OutputDisposed(OutputDisposed),
    /// Something in the execution structure is broken. This indicates a severe
    /// implementation error.
    Broken(Broken),
}

impl TracedEventKind {
    pub fn is_for_session(&self, session: Entity) -> bool {
        match self {
            Self::MessageSent(msg) => {
                if msg.input.session_stack.contains(&session) {
                    return true;
                }

                for out in &msg.output {
                    if out.session_stack.contains(&session) {
                        return true;
                    }
                }
            }
            Self::SessionEvent(s) => {
                return s.session_stack.contains(&session);
            }
            Self::BufferEvent(b) => {
                if b.buffer.session_stack.contains(&session) {
                    return true;
                }

                return b.accessor.session_stack.contains(&session);
            }
            Self::OutputDisposed(d) => {
                return d.disposed_in_session.contains(&session);
            }
            Self::Broken(_) => {
                // For now we consider broken to always be relevant to all sessions
                return true;
            }
        }

        false
    }
}

impl From<MessageSent> for TracedEventKind {
    fn from(value: MessageSent) -> Self {
        Self::MessageSent(value)
    }
}

impl From<BufferEvent> for TracedEventKind {
    fn from(value: BufferEvent) -> Self {
        Self::BufferEvent(value)
    }
}

impl From<SessionEvent> for TracedEventKind {
    fn from(value: SessionEvent) -> Self {
        Self::SessionEvent(value)
    }
}

impl From<OutputDisposed> for TracedEventKind {
    fn from(value: OutputDisposed) -> Self {
        Self::OutputDisposed(value)
    }
}

impl From<Broken> for TracedEventKind {
    fn from(value: Broken) -> Self {
        Self::Broken(value)
    }
}

/// Top-level description of any traceable event that happens while executing.
/// Add a world observer to this event to track what events take place while
/// executing.
#[derive(Debug, Clone, Event)]
pub struct TracedEvent {
    pub event: TracedEventKind,
    pub instant: Instant,
    pub time: SystemTime,
}

impl TracedEvent {
    pub fn now(event: impl Into<TracedEventKind>) -> Self {
        Self {
            event: event.into(),
            instant: Instant::now(),
            time: SystemTime::now(),
        }
    }

    pub fn trace(event: impl Into<TracedEventKind>, world: &mut World) {
        let event = event.into();
        world.write_trace(Self::now(event));
    }
}

/// Information about an operation.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct OperationInfo {
    /// The identifier of this operation which is unique within the workflow
    id: Option<OperationRef>,
    /// The input message type
    message_type: Option<Cow<'static, str>>,
    /// Information about how the operation was constructed. For operations
    /// built by the diagram workflow builder, this will be the contents of the
    /// operation definition.
    ///
    /// For manually inserted traces, the content of this is user-defined.
    construction: Option<Arc<JsonMessage>>,
}

impl OperationInfo {
    pub fn new(
        id: Option<OperationRef>,
        message_type: Option<Cow<'static, str>>,
        construction: Option<Arc<JsonMessage>>,
    ) -> Self {
        Self {
            id,
            message_type,
            construction,
        }
    }

    /// The unique identifier for this operation within the workflow.
    pub fn id(&self) -> &Option<OperationRef> {
        &self.id
    }

    /// Get the message type that this operation uses, if one is available.
    pub fn message_type(&self) -> &Option<Cow<'static, str>> {
        &self.message_type
    }

    /// If this operation was created by a builder, this is the ID of that
    /// builder
    pub fn construction(&self) -> &Option<Arc<JsonMessage>> {
        &self.construction
    }
}

fn get_session_stack(mut session: Entity, child_of: &Query<&ChildOf>) -> SmallVec<[Entity; 8]> {
    let mut session_stack = SmallVec::new();
    session_stack.push(session);
    while let Ok(child_of) = child_of.get(session) {
        session = child_of.parent();
        session_stack.push(session);
    }
    session_stack.reverse();
    session_stack
}

fn get_session_stack_from_world(session: Entity, world: &mut World) -> SmallVec<[Entity; 8]> {
    let mut child_of_state = world.query::<&ChildOf>();
    let child_of = child_of_state.query(world);
    get_session_stack(session, &child_of)
}

#[derive(Resource, Default, Debug, Deref, DerefMut)]
pub(crate) struct TraceLog(pub(crate) VecDeque<TracedEvent>);

struct WriteToTraceLogCmd(TracedEvent);

impl Command for WriteToTraceLogCmd {
    fn apply(self, world: &mut World) -> () {
        world.get_resource_or_init::<TraceLog>().push_back(self.0);
    }
}

pub(crate) trait WriteToTraceLog {
    fn write_trace(&mut self, event: TracedEvent);
}

impl WriteToTraceLog for World {
    fn write_trace(&mut self, event: TracedEvent) {
        self.get_resource_or_init::<TraceLog>().push_back(event);
    }
}

impl<'w, 's> WriteToTraceLog for Commands<'w, 's> {
    fn write_trace(&mut self, event: TracedEvent) {
        self.queue(WriteToTraceLogCmd(event));
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        BufferEvent, TracedEvent, TracedEventKind, TracedMessage,
        diagram::{testing::*, *},
        prelude::*,
    };
    use bevy_app::App;
    use bevy_ecs::prelude::{Entity, ResMut, Resource, Trigger};
    use serde_json::json;
    use std::{collections::VecDeque, sync::Arc, time::Duration};

    #[derive(Clone, Resource, Default, Debug)]
    struct TraceRecorder {
        record: VecDeque<TracedEvent>,
    }

    fn enable_trace_recording(app: &mut App) {
        app.init_resource::<TraceRecorder>()
            .add_observer(write_trace_events);
    }

    fn write_trace_events(trigger: Trigger<TracedEvent>, mut recorder: ResMut<TraceRecorder>) {
        recorder.record.push_back(trigger.event().clone());
    }

    #[test]
    fn test_tracing_pachinko() {
        let mut fixture = DiagramTestFixture::new();
        enable_trace_recording(&mut fixture.context.app);

        fixture
            .registry
            .register_node_builder(
                NodeBuilderOptions::new("less_than"),
                |builder, config: i64| {
                    builder.create_map_block(move |value: i64| {
                        if value < config {
                            Ok(value)
                        } else {
                            Err(value)
                        }
                    })
                },
            )
            .with_result();

        fixture
            .registry
            .register_node_builder(NodeBuilderOptions::new("noop"), |builder, _config: ()| {
                builder.create_map_block(|value: i64| value)
            });

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "default_trace": "messages",
            "start": "less_than_60",
            "ops": {
                "less_than_60": {
                    "type": "node",
                    "builder": "less_than",
                    "config": 60,
                    "next": "fork_60",
                    "display_text": "Evaluate 60",
                },
                "fork_60": {
                    "type": "fork_result",
                    "ok": "less_than_30",
                    "err": "less_than_90",
                    "display_text": "Fork 60",
                },
                "less_than_30": {
                    "type": "node",
                    "builder": "less_than",
                    "config": 30,
                    "next": "fork_30",
                    "display_text": "Evaluate 30",
                },
                "fork_30": {
                    "type": "fork_result",
                    "ok": "towards_15",
                    "err": "towards_45",
                    "display_text": "Fork 30",
                },
                "towards_15": {
                    "type": "node",
                    "builder": "noop",
                    "next": { "builtin": "terminate" },
                    "display_text": "Towards 15",
                },
                "towards_45": {
                    "type": "node",
                    "builder": "noop",
                    "next": { "builtin": "terminate" },
                    "display_text": "Towards 45",
                },
                "less_than_90": {
                    "type": "node",
                    "builder": "less_than",
                    "config": 90,
                    "next": "fork_90",
                    "display_text": "Evaluate 90",
                },
                "fork_90": {
                    "type": "fork_result",
                    "ok": "towards_75",
                    "err": "towards_105",
                    "display_text": "Fork 90",
                },
                "towards_75": {
                    "type": "node",
                    "builder": "noop",
                    "next": { "builtin": "terminate" },
                    "display_text": "Towards 75",
                },
                "towards_105": {
                    "type": "node",
                    "builder": "noop",
                    "next": { "builtin": "terminate" },
                    "display_text": "Towards 105",
                },
            }
        }))
        .unwrap();

        let panchinko = fixture.spawn_io_workflow::<i64, i64>(&diagram).unwrap();

        confirm_panchinko_route(
            10,
            panchinko,
            &mut fixture,
            &[
                "less_than_60",
                "fork_60",
                "less_than_30",
                "fork_30",
                "towards_15",
            ],
        );

        confirm_panchinko_route(
            70,
            panchinko,
            &mut fixture,
            &[
                "less_than_60",
                "fork_60",
                "less_than_90",
                "fork_90",
                "towards_75",
            ],
        );

        confirm_panchinko_route(
            50,
            panchinko,
            &mut fixture,
            &[
                "less_than_60",
                "fork_60",
                "less_than_30",
                "fork_30",
                "towards_45",
            ],
        );
    }

    fn confirm_panchinko_route(
        value: i64,
        panchinko: Service<i64, i64>,
        fixture: &mut DiagramTestFixture,
        route: &[&str],
    ) {
        let Capture {
            mut outcome,
            session,
            ..
        } = fixture
            .context
            .command(|commands| commands.request(value, panchinko).capture());

        fixture
            .context
            .run_with_conditions(&mut outcome, Duration::from_secs(2));
        assert!(fixture.context.no_unhandled_errors());
        let result = outcome.try_recv().unwrap().unwrap();
        assert_eq!(value, result);

        let recorder = fixture
            .context
            .app
            .world()
            .resource::<TraceRecorder>()
            .clone();
        confirm_trace(recorder, route, session);

        // Clear the record so these results do not interfere with the next test
        fixture
            .context
            .app
            .world_mut()
            .resource_mut::<TraceRecorder>()
            .record
            .clear();
    }

    fn confirm_trace(recorder: TraceRecorder, expectation: &[&str], expected_root_session: Entity) {
        let mut actual = recorder.record;
        for next_op_name in expectation {
            let name: Arc<str> = (*next_op_name).into();
            let expected_op = OperationRef::Named((&name).into());
            let next_actual = loop {
                let Some(next) = actual.pop_front() else {
                    break None;
                };

                match next.event {
                    TracedEventKind::MessageSent(sent) => {
                        if let Some(info) = &sent.input.info
                            && let Some(id) = &info.id
                        {
                            break Some((id.clone(), sent.input.session_stack));
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            };

            let (id, actual_session_stack) = next_actual.unwrap();
            assert_eq!(expected_op, id);

            let actual_root_session = *actual_session_stack.first().unwrap();
            assert_eq!(expected_root_session, actual_root_session);
        }
    }

    #[test]
    fn test_tracing_buffer_input() {
        let mut fixture = DiagramTestFixture::new();
        enable_trace_recording(&mut fixture.context.app);

        #[derive(StreamPack)]
        struct TestStream {
            integers: u64,
        }

        #[derive(Accessor, Clone)]
        struct TestAccessor {
            integers: BufferKey<u64>,
        }

        fixture.registry.register_node_builder(
            NodeBuilderOptions::new("spread"),
            |builder, _: ()| {
                let f = |input: Blocking<Vec<u64>, TestStream>| {
                    for value in input.request {
                        input.streams.integers.send(value);
                    }
                };
                builder.create_node(f.into_map())
            },
        );

        fixture
            .registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .register_node_builder(NodeBuilderOptions::new("drain"), |builder, _: ()| {
                let f = |srv: Blocking<((), TestAccessor)>, mut access: BufferAccessMut<u64>| {
                    let _: Vec<_> = access
                        .get_mut(srv.id, &srv.request.1.integers)
                        .unwrap()
                        .drain(..)
                        .collect();
                };
                builder.create_node(f.into_callback())
            })
            .with_buffer_access();

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "default_trace": "messages",
            "start": "spread",
            "ops": {
                "spread": {
                    "type": "node",
                    "builder": "spread",
                    "stream_out": {
                        "integers": "test_buffer"
                    },
                    "next": "access"
                },
                "test_buffer": {
                    "type": "buffer",
                    "settings": {
                        "retention": "keep_all"
                    }
                },
                "access": {
                    "type": "buffer_access",
                    "buffers": {
                        "integers": "test_buffer"
                    },
                    "next": "drain"
                },
                "drain": {
                    "type": "node",
                    "builder": "drain",
                    "next": { "builtin" : "terminate" }
                }
            }
        }))
        .unwrap();

        let sequence = vec![0, 1, 2, 3, 4, 5];
        fixture
            .spawn_and_run::<Vec<u64>, ()>(&diagram, sequence.clone())
            .unwrap();

        let recorder = fixture
            .context
            .app
            .world_mut()
            .resource::<TraceRecorder>()
            .clone();
        confirm_buffer_input_sequence(recorder, sequence);
    }

    fn confirm_buffer_input_sequence(recorder: TraceRecorder, expected_sequence: Vec<u64>) {
        let mut actual = recorder.record;
        let mut seqs = Vec::new();

        for next_item in expected_sequence {
            // A viewed event happens before each push event because each buffer
            // entry arrived through a different push operation.
            let viewed = next_buffer_event(&mut actual).unwrap().access;
            assert!(viewed.is_viewed());

            let next_actual = next_buffer_event(&mut actual);
            let pushed = next_actual.as_ref().unwrap().access.pushed().unwrap();
            seqs.push(pushed.seq);

            let value = get_u64_from_trace(&pushed.message);
            assert_eq!(next_item, value);
        }

        let viewed = next_buffer_event(&mut actual).unwrap().access;
        assert!(viewed.is_viewed());

        // Test that we also traced the items being drained
        for seq in seqs {
            let next_actual = next_buffer_event(&mut actual);
            let removal = next_actual.as_ref().unwrap().access.removed().unwrap();
            assert_eq!(seq, removal.seq);
        }
    }

    fn next_buffer_event(queue: &mut VecDeque<TracedEvent>) -> Option<BufferEvent> {
        loop {
            let Some(next) = queue.pop_front() else {
                return None;
            };

            match next.event {
                TracedEventKind::BufferEvent(event) => {
                    return Some(event);
                }
                _ => {
                    continue;
                }
            }
        }
    }

    #[test]
    fn test_tracing_buffer_modifications() {
        let mut fixture = DiagramTestFixture::new();
        enable_trace_recording(&mut fixture.context.app);

        fixture
            .registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .register_node_builder(
                NodeBuilderOptions::new("push_to_buffer"),
                |builder, _: ()| {
                    let f = |srv: Blocking<(Vec<u64>, BufferKey<u64>)>,
                             mut access: BufferAccessMut<u64>| {
                        let mut buffer = access.get_mut(srv.id, &srv.request.1).unwrap();
                        for value in srv.request.0 {
                            buffer.push(value);
                        }
                        srv.request.1
                    };
                    builder.create_node(f.into_callback())
                },
            )
            .with_buffer_access();

        fixture
            .registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .register_node_builder(
                NodeBuilderOptions::new("multiply_values_in_buffer"),
                |builder, factor: u64| {
                    let f = move |srv: Blocking<BufferKey<u64>>,
                                  mut access: BufferAccessMut<u64>| {
                        let mut buffer = access.get_mut(srv.id, &srv.request).unwrap();
                        for mut value in buffer.iter_mut() {
                            *value = factor * *value;
                        }
                    };
                    builder.create_node(f.into_callback())
                },
            );

        let factor: u64 = 5;
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "default_trace": "messages",
            "start": "access",
            "ops": {
                "test_buffer": {
                    "type": "buffer",
                    "settings": {
                        "retention": "keep_all"
                    }
                },
                "access": {
                    "type": "buffer_access",
                    "buffers": ["test_buffer"],
                    "next": "push"
                },
                "push": {
                    "type": "node",
                    "builder": "push_to_buffer",
                    "next": "multiply"
                },
                "multiply": {
                    "type": "node",
                    "config": factor,
                    "builder": "multiply_values_in_buffer",
                    "next": { "builtin" : "terminate"}
                }
            }
        }))
        .unwrap();

        let sequence = vec![0, 1, 2, 3, 4, 5];
        fixture
            .spawn_and_run::<Vec<u64>, ()>(&diagram, sequence.clone())
            .unwrap();

        let recorder = fixture
            .context
            .app
            .world_mut()
            .resource::<TraceRecorder>()
            .clone();
        confirm_buffer_modifications(recorder, sequence, factor);
    }

    fn confirm_buffer_modifications(
        recorder: TraceRecorder,
        expected_sequence: Vec<u64>,
        factor: u64,
    ) {
        let mut actual = recorder.record;
        let mut entries = Vec::new();

        // The buffer is only accessed one time to perform all the pushes, so
        // we will only see one initial view event for all pushes.
        let viewed = next_buffer_event(&mut actual).unwrap().access;
        assert!(viewed.is_viewed());

        for next_item in expected_sequence {
            let next_actual = next_buffer_event(&mut actual);

            let pushed = next_actual.as_ref().unwrap().access.pushed().unwrap();

            let seq = pushed.seq;
            let value = get_u64_from_trace(&pushed.message);

            entries.push((seq, value));
            assert_eq!(next_item, value);
        }

        // The buffer is only accessed one time to perform all the modifications,
        // so we will only see one initial view event for all pushes.
        let viewed = next_buffer_event(&mut actual).unwrap().access;
        assert!(viewed.is_viewed());

        for (seq, value) in entries {
            let next_actual = next_buffer_event(&mut actual);

            let modification = next_actual.as_ref().unwrap().access.modified().unwrap();

            assert_eq!(modification.seq, seq);

            let original = get_u64_from_trace(&modification.original);
            assert_eq!(original, value);

            let modified = get_u64_from_trace(&modification.modified);
            assert_eq!(modified, factor * value);
        }
    }

    fn get_u64_from_trace(msg: &TracedMessage) -> u64 {
        msg.as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .as_number()
            .unwrap()
            .as_i64()
            .unwrap() as u64
    }
}
