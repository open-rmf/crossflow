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
    JsonMessage, OperationRef, TraceToggle, TypeInfo, OutputPort, Seq, OutputRef,
    Routing, OutputKey, RequestId, BufferKeyTag, RouteSource, Cancellation, Disposal,
    Broken, IdentifierRef, OperationType,
};

use bevy_ecs::{
    prelude::{Component, Entity, Event, World, ChildOf, Query, Commands, Resource, Res},
    system::SystemParam,
};
use bevy_derive::{Deref, DerefMut};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::{
    any::Any,
    borrow::Cow,
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

    pub fn set_toggle(&mut self, toggle: TraceToggle) {
        self.toggle = toggle;
    }

    pub fn toggle(&self) -> TraceToggle {
        self.toggle
    }

    /// Get the information for this workflow operation.
    pub fn info(&self) -> &Arc<OperationInfo> {
        &self.info
    }

    /// Attempt to serialize the value. This will return a None if the trace is
    /// not set up to serialize the values.
    pub fn serialize_value(&self, value: &dyn Any) -> Option<Result<JsonMessage, GetValueError>> {
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
    fn new(
        route_source: RouteSource,
        world: &mut World,
    ) -> Self {
        let output_port = route_source.port;
        let session_stack = get_session_stack_from_world(route_source.session, world);
        let port = route_source
            .port
            .iter()
            .map(|p| p.to_owned())
            .collect();
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
            labels: world.get::<OperationLabels>(route_source.source)
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
    fn new(
        request_id: RequestId,
        world: &mut World,
    ) -> Self {
        let RequestId { session, source, seq } = request_id;
        let operation_type = world
            .get::<OperationType>(source)
            .map(|op| (**op).clone())
            .unwrap_or_else(|| "<unknown>".into());
        let info = world
            .get::<Trace>(source)
            .map(|t| t.info.clone());

        Self {
            session_stack: get_session_stack_from_world(session, world),
            target: source,
            seq,
            labels: world.get::<OperationLabels>(source)
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

/// An event that tracks when each message is sent for a request or within a
/// workflow.
///
/// Set up [`TraceHandler`] to have a custom system handle these messages.
/// Otherwise if you use the default [`TraceHandler`] then you can add a system
/// to the App schedule to read this event.
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
    pub message: Option<Result<JsonMessage, GetValueError>>,
}

impl MessageSent {
    pub(crate) fn trace(
        route: Routing,
        target_seq: Seq,
        message: Option<Result<JsonMessage, GetValueError>>,
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

        let event = MessageSent { output, input, message };
        world.trigger(TracedEvent::now(event));
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
        let trigger = TraceSource::new(trigger, world);
        let disposed_in_session = get_session_stack_from_world(disposed_in_session, world);
        world.trigger(TracedEvent::now(Self { trigger, disposed_operation, disposed_in_session, disposal }));
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
    Modified,
}

#[derive(SystemParam)]
pub(crate) struct BufferTracer<'w, 's> {
    trace: Query<'w, 's, &'static Trace>,
    child_of: Query<'w, 's, &'static ChildOf>,
    labels: Query<'w, 's, &'static OperationLabels>,
    op_type: Query<'w, 's, &'static OperationType>,
    info: Query<'w, 's, &'static Trace>,
    universal: Option<Res<'w, UniversalTraceToggle>>,
    commands: Commands<'w, 's>,
}

impl<'w, 's> BufferTracer<'w, 's> {
    pub(crate) fn trace(
        &mut self,
        req: RequestId,
        key: &BufferKeyTag,
        access: BufferAccessRecord,
    ) {
        let toggle = if let Some(universal) = self.universal.as_ref().map(|u| ***u).flatten() {
            universal
        } else if let Ok(buffer_trace) = self.trace.get(key.buffer) {
            buffer_trace.toggle
        } else {
            return;
        };

        if !toggle.is_on() {
            return;
        }

        let buffer_trace = self.trace.get(key.buffer).ok();
        let buffer_labels = self.labels.get(key.buffer).ok().map(|l| l.input.clone());
        let accessor_labels = self.labels.get(req.source).ok().map(|l| l.input.clone());

        // let value_serializer = if toggle.with_messages() {
        //     buffer_trace.map(|t| t.serialize_value).flatten()
        // } else {
        //     None
        // };

        let accessor_session_stack = get_session_stack(req.session, &self.child_of);
        let buffer_session_stack = if key.session == req.session {
            accessor_session_stack.clone()
        } else {
            get_session_stack(key.session, &self.child_of)
        };

        let operation_type = self
            .op_type
            .get(key.buffer)
            .map(|op| (**op).clone())
            .unwrap_or_else(|_| "<unknown>".into());
        let info = self
            .info
            .get(key.buffer)
            .ok()
            .map(|t| t.info.clone());

        let buffer_event = BufferEvent {
            accessor: TraceTarget {
                session_stack: accessor_session_stack,
                target: req.source,
                seq: req.seq,
                labels: accessor_labels,
                operation_type,
                info,
            },
            buffer: TraceBuffer {
                session_stack: buffer_session_stack,
                id: key.buffer,
                labels: buffer_labels,
            },
            access,
        };

        self.commands.trigger(TracedEvent::now(buffer_event));
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

        world.trigger(TracedEvent::now(event));
    }

    pub(crate) fn despawned(session: Entity, world: &mut World) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::Despawned,
        };

        world.trigger(TracedEvent::now(event));
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
            change: SessionChange::Cancelled { source, cancellation },
        };

        world.trigger(TracedEvent::now(event));
    }

    pub(crate) fn cleanup(
        session: Entity,
        world: &mut World,
    ) {
        let session_stack = get_session_stack_from_world(session, world);
        let event = SessionEvent {
            session_stack,
            change: SessionChange::BeginCleanup,
        };

        world.trigger(TracedEvent::now(event));
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
        world.trigger(Self::now(event));
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

fn get_session_stack(
    mut session: Entity,
    child_of: &Query<&ChildOf>,
) -> SmallVec<[Entity; 8]> {
    let mut session_stack = SmallVec::new();
    session_stack.push(session);
    while let Ok(child_of) = child_of.get(session) {
        session = child_of.parent();
        session_stack.push(session);
    }
    session_stack.reverse();
    session_stack
}

fn get_session_stack_from_world(
    session: Entity,
    world: &mut World,
) -> SmallVec<[Entity; 8]> {
    let mut child_of_state = world.query::<&ChildOf>();
    let child_of = child_of_state.query(world);
    get_session_stack(session, &child_of)
}

#[cfg(test)]
mod tests {

    use crate::{
        diagram::{testing::*, *},
        prelude::*,
        TracedEvent, TracedEventKind
    };
    use bevy_app::App;
    use bevy_ecs::prelude::{Entity, ResMut, Resource, Trigger};
    use serde_json::json;
    use std::{sync::Arc, time::Duration, collections::VecDeque};

    #[derive(Clone, Resource, Default, Debug)]
    struct TraceRecorder {
        record: VecDeque<TracedEvent>,
    }

    fn enable_trace_recording(app: &mut App) {
        app.init_resource::<TraceRecorder>()
            .add_observer(write_trace_events);
    }

    fn write_trace_events(
        trigger: Trigger<TracedEvent>,
        mut recorder: ResMut<TraceRecorder>,
    ) {
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
            .world_mut()
            .resource_mut::<TraceRecorder>()
            .clone();
        confirm_trace(&recorder, route, session);

        // Clear the record so these results do not interfere with the next test
        fixture
            .context
            .app
            .world_mut()
            .resource_mut::<TraceRecorder>()
            .record
            .clear();
    }

    fn confirm_trace(
        recorder: &TraceRecorder,
        expectation: &[&str],
        expected_root_session: Entity,
    ) {
        let mut actual = recorder.record.clone();
        for next_op_name in expectation {
            let name: Arc<str> = (*next_op_name).into();
            let expected_op = OperationRef::Named((&name).into());
            let next_actual = loop {
                let Some(next) = actual.pop_front() else {
                    break None;
                };

                match next.event {
                    TracedEventKind::MessageSent(sent) => {
                        if let Some(info) = &sent.input.info && let Some(id) = &info.id {
                            break Some((id.clone(), sent.input.session_stack))
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
}
