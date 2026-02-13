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

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::Arc,
    ops::Deref,
};

use crate::{
    AnyBuffer, BufferIdentifier, BufferMap, Builder, BuilderScopeContext, JsonMessage,
    Scope, StreamPack, dyn_node::DynStreamInputPack, PortRef,
};

#[cfg(feature = "trace")]
use crate::{OperationInfo, Trace};

use super::{
    BufferSelection, Diagram, DiagramContext, DiagramElementRegistry, DiagramError, DiagramErrorCode, DynInputSlot,
    DynOutput, FinishingErrors, ImplicitDeserialization, ImplicitSerialization, ImplicitStringify,
    NamedOperationRef, NamespaceList, NextOperation, OperationName, OperationRef,
    Operations, TraceToggle, TypeInfo, InferenceContext, InferenceBoundaryConditions, TypeMismatch,
};

use bevy_ecs::prelude::Entity;

use serde::Serialize;

#[derive(Default)]
struct DiagramConstruction {
    /// Implementations that define how outputs can connect to their target operations
    connect_into_target: HashMap<OperationRef, Target>,
    /// A map of what outputs are going into each target operation
    outputs_into_target: HashMap<OperationRef, Vec<DynOutput>>,
    /// A map of what buffers exist in the diagram
    buffers: HashMap<OperationRef, BufferRef>,
    /// Operations that were spawned by another operation.
    generated_operations: Vec<UnfinishedOperation>,
}

impl<'a> DiagramConstruction {
    fn transfer_generated_operations(
        &mut self,
        deferred_operations: &mut Vec<UnfinishedOperation>,
        made_progress: &mut bool,
    ) {
        deferred_operations.extend(self.generated_operations.drain(..).map(|unfinished| {
            *made_progress = true;
            unfinished
        }));
    }
}

#[derive(Clone, Debug)]
enum BufferRef {
    Ref(OperationRef),
    Value(AnyBuffer),
}

impl DiagramConstruction {
    fn has_outputs(&self) -> bool {
        for outputs in self.outputs_into_target.values() {
            if !outputs.is_empty() {
                return true;
            }
        }

        return false;
    }
}

pub struct BuilderContext<'a, 'c, 'w, 's, 'b> {
    pub registry: &'a DiagramElementRegistry,
    message_type_inference: &'a HashMap<PortRef, usize>,
    construction: &'c mut DiagramConstruction,
    pub builder: &'c mut Builder<'w, 's, 'b>,
    diagram_context: DiagramContext<'a>,
}

impl<'a, 'c, 'w, 's, 'b> BuilderContext<'a, 'c, 'w, 's, 'b> {
    /// Get the message type that has been inferred for a certain input/output
    /// port. This will only include ports that your operation added constraints
    /// for.
    pub fn inferred_message_type(&self, port: impl Into<PortRef>) -> Result<TypeInfo, DiagramErrorCode> {
        let port = self.into_port_ref(port);
        let type_index = self
            .message_type_inference
            .get(&port)
            .copied()
            .ok_or_else(|| DiagramErrorCode::MessageTypeNotInferred(port))?;

        self.registry.messages.get_type_info_for(type_index)
    }

    /// Add an output to connect into a target.
    ///
    /// This can be used during both the [`BuildDiagramOperation`] phase and the
    /// [`ConnectIntoTarget`] phase.
    ///
    /// # Arguments
    ///
    /// * `target` - The operation that needs to receive the output
    /// * `output` - The output channel that needs to be connected into the target.
    pub fn add_output_into_target(&mut self, target: impl Into<OperationRef>, output: DynOutput) {
        let target = self.into_operation_ref(target);
        self.construction
            .outputs_into_target
            .entry(target)
            .or_default()
            .push(output);
    }

    /// Set the input slot of an operation. This should not be called more than
    /// once per operation, because only one input slot can be used for any
    /// operation.
    ///
    /// This should be used during the [`BuildDiagramOperation`] phase to set
    /// the input slot of each operation with standard connection behavior.
    /// Standard connection behavior means that implicit serialization or
    /// deserialization will be applied to its inputs as needed to match the
    /// [`DynInputSlot`] that you provide.
    ///
    /// If you need non-standard connection behavior then you can use
    /// [`Self::set_connect_into_target`] to set the exact connection behavior.
    /// No implicit behaviors will be provided for [`Self::set_connect_into_target`],
    /// but you can enable those behaviors using [`ImplicitSerialization`] and
    /// [`ImplicitDeserialization`].
    ///
    /// This should never be used during the [`ConnectIntoTarget`] phase because
    /// all connection behaviors must already be set by then.
    pub fn set_input_for_target(
        &mut self,
        operation: impl Into<OperationRef>,
        input: DynInputSlot,
        #[allow(unused)] trace_info: TraceInfo,
    ) -> Result<(), DiagramErrorCode> {
        let operation = self.into_operation_ref(operation);
        let connect = standard_input_connection(input, &self.registry)?;

        #[cfg(feature = "trace")]
        {
            let trace_toggle = trace_info.trace.unwrap_or(self.default_trace);
            let operation_info = OperationInfo::new(
                Some(operation.clone()),
                Some(input.message_info().type_name.into()),
                trace_info.construction,
            );

            let mut trace = Trace::new(trace_toggle, Arc::new(operation_info));

            let enable_trace_serialization = self
                .registry
                .messages
                .get_operations(input.message_info())?
                .enable_trace_serialization;

            if let Some(enable) = enable_trace_serialization {
                enable(&mut trace);
            }

            self.builder.commands().entity(input.id()).insert(trace);
        }

        self.impl_connect_into_target(operation, connect)
    }

    /// Set the implementation for how outputs connect into this target. This is
    /// a more general method than [`Self::set_input_for_target`].
    ///
    /// There will be no additional connection behavior beyond what you specify
    /// with the object passed in for `connect`. That means we will not
    /// automatically add implicit serialization or deserialization when you use
    /// this method. If you want your connection behavior to have those implicit
    /// operations you can use [`ImplicitSerialization`] and
    /// [`ImplicitDeserialization`] inside your `connect` implementation.
    ///
    /// This should never be necessary to use this during the [`ConnectIntoTarget`]
    /// phase because all connection behaviors should already be set by then.
    pub fn set_connect_into_target<C: ConnectIntoTarget + 'static>(
        &mut self,
        operation: impl Into<OperationRef>,
        connect: C,
    ) -> Result<(), DiagramErrorCode> {
        let operation = self.into_operation_ref(operation);
        let connect = Box::new(connect);
        self.impl_connect_into_target(operation, connect)
    }

    /// Internal implementation of adding a connection into a target
    fn impl_connect_into_target(
        &mut self,
        operation: OperationRef,
        connector: Box<dyn ConnectIntoTarget + 'static>,
    ) -> Result<(), DiagramErrorCode> {
        match self
            .construction
            .connect_into_target
            .entry(operation.clone())
        {
            Entry::Occupied(_) => {
                return Err(DiagramErrorCode::DuplicateInputsCreated(operation));
            }
            Entry::Vacant(vacant) => {
                vacant.insert(Target {
                    connector,
                    scope: self.builder.context,
                });
            }
        }
        Ok(())
    }

    /// Set the buffer that should be used for a certain operation. This will
    /// also set its connection callback.
    pub fn set_buffer_for_operation(
        &mut self,
        operation: impl Into<OperationRef> + Clone,
        buffer: AnyBuffer,
        trace: TraceInfo,
    ) -> Result<(), DiagramErrorCode> {
        let input: DynInputSlot = buffer.into();
        self.set_input_for_target(operation.clone(), input, trace)?;

        let operation = self.into_operation_ref(operation);
        match self.construction.buffers.entry(operation.clone()) {
            Entry::Occupied(_) => {
                return Err(DiagramErrorCode::DuplicateBuffersCreated(operation));
            }
            Entry::Vacant(vacant) => {
                vacant.insert(BufferRef::Value(buffer));
            }
        }

        Ok(())
    }

    /// Create a buffer map based on the buffer inputs provided. If one or more
    /// of the buffers in BufferInputs is not available, get an error including
    /// the name of the missing buffer.
    pub fn create_buffer_map(&self, inputs: &BufferSelection) -> Result<BufferMap, DiagramErrorCode> {
        let attempt_get_buffer = |buffer: &NextOperation| -> Result<AnyBuffer, DiagramErrorCode> {
            let mut buffer_ref: OperationRef = self.into_operation_ref(buffer);
            let mut visited = HashSet::new();
            loop {
                if !visited.insert(buffer_ref.clone()) {
                    return Err(DiagramErrorCode::CircularRedirect(visited.into_iter().collect()));
                }

                let next = self
                    .construction
                    .buffers
                    .get(&buffer_ref)
                    .ok_or_else(|| DiagramErrorCode::UnknownOperation(buffer_ref))?;

                buffer_ref = match next {
                    BufferRef::Value(value) => return Ok(*value),
                    BufferRef::Ref(reference) => reference.clone(),
                };
            }
        };

        match inputs {
            BufferSelection::Dict(mapping) => {
                let mut buffer_map = BufferMap::with_capacity(mapping.len());
                for (k, op_id) in mapping {
                    buffer_map.insert(
                        BufferIdentifier::Name(k.clone().into()),
                        attempt_get_buffer(op_id)?,
                    );
                }
                Ok(buffer_map)
            }
            BufferSelection::Array(arr) => {
                let mut buffer_map = BufferMap::with_capacity(arr.len());
                for (i, op_id) in arr.into_iter().enumerate() {
                    buffer_map.insert(BufferIdentifier::Index(i), attempt_get_buffer(op_id)?);
                }
                Ok(buffer_map)
            }
        }
    }

    /// Add an operation which exists as a child inside another operation.
    ///
    /// For example this is used by section templates to add their inner
    /// operations into the workflow builder.
    ///
    /// Operations added this way will be internal to the parent operation,
    /// which means they are not "exposed" to be connected to other operations
    /// that are not inside the parent.
    ///
    /// Use the scope argument if the child operation exists in a different scope
    /// than the parent. For the child of a Section, this is None.
    pub fn add_child_operation<T: BuildDiagramOperation + 'static>(
        &mut self,
        id: &OperationName,
        child_id: &OperationName,
        op: &Arc<T>,
        sibling_ops: Operations,
        on_implicit_error: Option<NextOperation>,
        scope: Option<BuilderScopeContext>,
    ) {
        let mut namespaces = self.namespaces.clone();
        namespaces.push(Arc::clone(id));

        let on_implicit_error = match on_implicit_error {
            Some(op) => {
                let op: OperationRef = (&op).into();
                op.in_namespaces(&namespaces)
            }
            None => {
                self.on_implicit_error.clone()
            }
        };

        self.construction
            .generated_operations
            .push(UnfinishedOperation {
                id: Arc::clone(child_id),
                namespaces,
                op: op.clone() as Arc<dyn BuildDiagramOperation>,
                sibling_ops: sibling_ops.clone(),
                on_implicit_error,
                scope: scope.unwrap_or(self.builder.context),
            });
    }

    /// Create a connection for an exposed input that allows it to redirect any
    /// connections to an internal (child) input.
    ///
    /// # Arguments
    ///
    /// * `section_id` - The ID of the parent operation (e.g. ID of the section)
    ///   that is exposing an operation to its siblings.
    /// * `exposed_id` - The ID of the exposed operation that is being provided
    ///   by the section. E.g. siblings of the section will refer to the redirected
    ///   input by `{ section_id: exposed_id }`
    /// * `child_id` - The ID of the operation inside the section which the exposed
    ///   operation ID is being redirected to.
    ///
    /// This should not be used to expose a buffer. For that, use
    /// [`Self::redirect_to_child_buffer`].
    pub fn redirect_to_child_input(
        &mut self,
        section_id: &OperationName,
        exposed_id: &OperationName,
        child_id: &NextOperation,
    ) -> Result<(), DiagramErrorCode> {
        let (exposed, redirect_to) =
            self.get_exposed_and_inner_ids(section_id, exposed_id, child_id);

        self.impl_connect_into_target(exposed, Box::new(RedirectConnection::new(redirect_to)))
    }

    /// Same as [`Self::redirect_to_child_input`], but meant for buffers.
    pub fn redirect_to_child_buffer(
        &mut self,
        section_id: &OperationName,
        exposed_id: &OperationName,
        child_id: &NextOperation,
    ) -> Result<(), DiagramErrorCode> {
        let (exposed, redirect_to) =
            self.get_exposed_and_inner_ids(section_id, exposed_id, child_id);

        self.impl_connect_into_target(
            exposed.clone(),
            Box::new(RedirectConnection::new(redirect_to.clone())),
        )?;

        match self.construction.buffers.entry(exposed.clone()) {
            Entry::Occupied(_) => {
                return Err(DiagramErrorCode::DuplicateBuffersCreated(exposed));
            }
            Entry::Vacant(vacant) => {
                vacant.insert(BufferRef::Ref(redirect_to));
            }
        }

        Ok(())
    }

    /// Create a connection for an exposed output that allows it to redirect
    /// any connections to an external (sibling) input. This is used to implement
    /// the `"connect":` schema for section templates.
    ///
    /// # Arguments
    ///
    /// * `section_id` - The ID of the parent operation (e.g. ID of the section)
    ///   that is exposing an output to its siblings.
    /// * `output_id` - The ID of the exposed output that is being provided by
    ///   the section. E.g. siblings of the section will refer to the redirected
    ///   output by `{ section_id: output_id }`.
    /// * `sibling_id` - The sibling of the section that should receive the output.
    pub fn redirect_exposed_output_to_sibling(
        &mut self,
        section_id: &OperationName,
        output_id: &OperationName,
        sibling_id: &NextOperation,
    ) -> Result<(), DiagramErrorCode> {
        // This is the slot that operations inside of the section will direct
        // their outputs to. It will receive the outputs and then redirect them.
        let internal: OperationRef = NamedOperationRef {
            namespaces: NamespaceList::for_child_of(Arc::clone(section_id)),
            exposed_namespace: None,
            name: Arc::clone(output_id),
        }
        .in_namespaces(&self.namespaces)
        .into();

        let redirect_to = self.into_operation_ref(sibling_id);
        self.impl_connect_into_target(internal, Box::new(RedirectConnection::new(redirect_to)))
    }

    fn get_exposed_and_inner_ids(
        &self,
        section_id: &OperationName,
        exposed_id: &OperationName,
        child_id: &NextOperation,
    ) -> (OperationRef, OperationRef) {
        let mut child_namespaces = self.namespaces.clone();
        child_namespaces.push(Arc::clone(section_id));
        let inner = Into::<OperationRef>::into(child_id).in_namespaces(&child_namespaces);

        let exposed: OperationRef = NamedOperationRef {
            namespaces: self.namespaces.clone(),
            exposed_namespace: Some(section_id.clone()),
            name: exposed_id.clone(),
        }
        .into();

        (exposed, inner)
    }
}

impl<'a, 'c, 'w, 's, 'b> Deref for BuilderContext<'a, 'c, 'w, 's, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

/// Indicate whether the operation has finished building.
#[derive(Debug, Clone)]
pub enum BuildStatus {
    /// The operation has finished building.
    Finished,
    /// The operation needs to make another attempt at building after more
    /// information becomes available.
    Defer {
        /// Progress was made during this run. This can mean the operation has
        /// added some information into the diagram but might need to provide
        /// more later. If no operations make any progress within an entire
        /// iteration of the workflow build then we assume it is impossible to
        /// build the diagram.
        progress: bool,
        reason: DiagramErrorCode,
    },
}

impl BuildStatus {
    /// Indicate that the build of the operation needs to be deferred.
    pub fn defer(reason: impl Into<DiagramErrorCode>) -> Self {
        Self::Defer {
            progress: false,
            reason: reason.into(),
        }
    }

    /// Indicate that the operation made progress, even if it's deferred.
    #[allow(unused)]
    pub fn with_progress(mut self) -> Self {
        match &mut self {
            Self::Defer { progress, .. } => {
                *progress = true;
            }
            Self::Finished => {
                // Do nothing
            }
        }

        self
    }

    /// Did this operation make progress on this round?
    pub fn made_progress(&self) -> bool {
        match self {
            Self::Defer { progress, .. } => *progress,
            Self::Finished => true,
        }
    }

    /// Change this build status into its reason for deferral, if it is a
    /// deferral.
    pub fn into_deferral_reason(self) -> Option<DiagramErrorCode> {
        match self {
            Self::Defer { reason, .. } => Some(reason),
            Self::Finished => None,
        }
    }

    /// Check if the build finished.
    pub fn is_finished(&self) -> bool {
        matches!(self, BuildStatus::Finished)
    }
}

/// This trait is used to instantiate operations in the workflow. This trait
/// will be called on each operation in the diagram until it finishes building.
/// Each operation should use this to provide a [`ConnectIntoTarget`] handle for
/// itself (if relevant) and deposit [`DynOutput`]s into [`DiagramContext`].
pub trait BuildDiagramOperation {
    fn build_diagram_operation<'a, 'c>(
        &self,
        id: &OperationName,
        ctx: &mut BuilderContext,
    ) -> Result<BuildStatus, DiagramErrorCode>;

    fn apply_message_type_constraints(
        &self,
        id: &OperationName,
        ctx: &mut InferenceContext,
    ) -> Result<(), DiagramErrorCode>;
}

/// This trait is used to connect outputs to their target operations. This trait
/// will be called for each output produced by [`BuildDiagramOperation`].
///
/// You are allowed to generate new outputs during the [`ConnectIntoTarget`]
/// phase by calling [`DiagramContext::add_output_into_target`].
pub trait ConnectIntoTarget {
    fn connect_into_target(
        &mut self,
        output: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode>;

    fn is_finished(&self) -> Result<(), DiagramErrorCode> {
        Ok(())
    }
}

pub(super) fn create_workflow<Request, Response, Streams>(
    scope: Scope<Request, Response, Streams>,
    builder: &mut Builder,
    registry: &DiagramElementRegistry,
    diagram: &Diagram,
) -> Result<(), DiagramError>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    let inference = diagram.infer_message_types(
        registry,
        InferenceBoundaryConditions::new::<Request, Response, Streams>(registry)?,
    )?;

    // This borrow is a trick to make it cleaner to create BuilderContext.
    let message_type_inference = &inference;

    let mut construction = DiagramConstruction::default();

    let root_on_implicit_error: OperationRef = (&diagram.on_implicit_error()).into();

    initialize_builtin_operations(
        diagram.start.clone(),
        scope,
        &mut BuilderContext {
            construction: &mut construction,
            builder,
            message_type_inference,
            registry,
            diagram_context: DiagramContext {
                operations: diagram.ops.clone(),
                templates: &diagram.templates,
                default_trace: diagram.default_trace,
                on_implicit_error: &root_on_implicit_error,
                namespaces: NamespaceList::default(),
            },
        },
    )?;

    let mut unfinished_operations: Vec<UnfinishedOperation> = diagram
        .ops
        .iter()
        .map(|(id, op)| {
            UnfinishedOperation::new(
                id,
                op.clone() as Arc<dyn BuildDiagramOperation>,
                &diagram.ops,
                root_on_implicit_error.clone(),
                builder.context,
            )
        })
        .collect();
    let mut deferred_operations: Vec<UnfinishedOperation> = Vec::new();
    let mut deferred_statuses: Vec<(OperationRef, BuildStatus)> = Vec::new();

    let mut deferred_connections = HashMap::new();
    let mut connector_construction = DiagramConstruction::default();

    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 10_000;

    // Iteratively build all the operations in the diagram
    while !unfinished_operations.is_empty() || construction.has_outputs() {
        let mut made_progress = false;
        for unfinished in unfinished_operations.drain(..) {
            let mut builder = Builder {
                context: unfinished.scope,
                commands: builder.commands,
            };

            let mut ctx = BuilderContext {
                construction: &mut construction,
                builder: &mut builder,
                message_type_inference,
                registry,
                diagram_context: DiagramContext {
                    operations: unfinished.sibling_ops.clone(),
                    templates: &diagram.templates,
                    default_trace: diagram.default_trace,
                    on_implicit_error: &unfinished.on_implicit_error,
                    namespaces: unfinished.namespaces.clone(),
                },
            };

            // Attempt to build this operation
            let status = unfinished
                .op
                .build_diagram_operation(&unfinished.id, &mut ctx)
                .map_err(|code| code.in_port(unfinished.as_operation_ref()))?;

            ctx.construction
                .transfer_generated_operations(&mut deferred_operations, &mut made_progress);

            made_progress |= status.made_progress();
            if !status.is_finished() {
                // The operation did not finish, so pass it into the deferred
                // operations list.
                deferred_statuses.push((unfinished.as_operation_ref(), status));
                deferred_operations.push(unfinished);
            }
        }

        unfinished_operations.extend(deferred_operations.drain(..));

        // Transfer outputs into their connections. Sometimes this needs to be
        // done before other operations can be built, e.g. a connection may need
        // to be redirected before its target operation knows how to infer its
        // message type.
        connector_construction.buffers = construction.buffers.clone();
        loop {
            for (id, outputs) in construction.outputs_into_target.drain() {
                let Some(target) = construction.connect_into_target.get_mut(&id) else {
                    if unfinished_operations.is_empty() {
                        return Err(DiagramErrorCode::UnknownOperation(id.into()).into());
                    } else {
                        deferred_connections.insert(id, outputs);
                        continue;
                    }
                };

                let mut builder = Builder {
                    context: target.scope,
                    commands: builder.commands,
                };

                let mut ctx = BuilderContext {
                    construction: &mut connector_construction,
                    builder: &mut builder,
                    message_type_inference,
                    registry,
                    diagram_context: DiagramContext {
                        operations: diagram.ops.clone(),
                        templates: &diagram.templates,
                        default_trace: diagram.default_trace,
                        on_implicit_error: &root_on_implicit_error,
                        // TODO(@mxgrey): The namespace while connecting into targets
                        // is always empty since the ConnectIntoTargets implementation
                        // is expected to provide targets that are already fully
                        // resolved. This inconsistency is questionable and will
                        // probably lead to bugs in the future, so we should
                        // reconisder our approach to this.
                        //
                        // Perhaps we can introduce a specific trait IntoOperationRef
                        // that takes in parent namespace information and behaves
                        // differently between NextOperation vs OperationRef. Then
                        // we also store namespace information per Target.
                        namespaces: Default::default(),
                    },
                };

                for output in outputs {
                    made_progress = true;
                    target
                        .connector
                        .connect_into_target(output, &mut ctx)
                        .map_err(|code| code.in_port(id.clone()))?;
                }
            }

            let new_connections = !connector_construction.outputs_into_target.is_empty();

            construction
                .outputs_into_target
                .extend(connector_construction.outputs_into_target.drain());

            construction
                .outputs_into_target
                .extend(deferred_connections.drain());

            connector_construction
                .transfer_generated_operations(&mut unfinished_operations, &mut made_progress);

            // TODO(@mxgrey): Consider draining new connect_into_target entries
            // out of connector_construction.

            iterations += 1;
            if iterations > MAX_ITERATIONS {
                return Err(DiagramErrorCode::ExcessiveIterations.into());
            }

            if !new_connections {
                break;
            }
        }

        if !made_progress {
            // No progress can be made any longer so return an error
            return Err(DiagramErrorCode::BuildHalted {
                reasons: deferred_statuses
                    .drain(..)
                    .filter_map(|(id, status)| {
                        status.into_deferral_reason().map(|reason| (id, reason))
                    })
                    .collect(),
            }
            .into());
        }

        iterations += 1;
        if iterations > MAX_ITERATIONS {
            return Err(DiagramErrorCode::ExcessiveIterations.into());
        }
    }

    let mut finishing = FinishingErrors::default();
    for (op, target) in &construction.connect_into_target {
        if let Err(err) = target.connector.is_finished() {
            finishing.errors.insert(op.clone(), err);
        }
    }
    finishing
        .as_result()
        .map_err(DiagramErrorCode::FinishingErrors)?;

    Ok(())
}

struct UnfinishedOperation {
    /// Name of the operation within its scope
    id: OperationName,
    /// The namespaces that this operation takes place inside
    namespaces: NamespaceList,
    /// Description of the operation
    op: Arc<dyn BuildDiagramOperation>,
    /// The sibling operations of the one that is being built
    sibling_ops: Operations,
    /// How this operation should handle implicit errors, if needed
    on_implicit_error: OperationRef,
    /// The scope of this operation. This is used to create the correct Builder
    /// for the operation.
    scope: BuilderScopeContext,
}

struct Target {
    connector: Box<dyn ConnectIntoTarget>,
    scope: BuilderScopeContext,
}

impl std::fmt::Debug for UnfinishedOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnfinishedOperation")
            .field("id", &self.id)
            .field("namespaces", &self.namespaces)
            .finish()
    }
}

impl UnfinishedOperation {
    pub fn new(
        id: &OperationName,
        op: Arc<dyn BuildDiagramOperation>,
        sibling_ops: &Operations,
        on_implicit_error: OperationRef,
        scope: BuilderScopeContext,
    ) -> Self {
        Self {
            id: Arc::clone(id),
            op,
            sibling_ops: sibling_ops.clone(),
            on_implicit_error,
            namespaces: Default::default(),
            scope,
        }
    }

    pub fn as_operation_ref(&self) -> OperationRef {
        NamedOperationRef {
            namespaces: self.namespaces.clone(),
            exposed_namespace: None,
            name: self.id.clone(),
        }
        .into()
    }
}

fn initialize_builtin_operations<Request, Response, Streams>(
    start: NextOperation,
    scope: Scope<Request, Response, Streams>,
    ctx: &mut BuilderContext,
) -> Result<(), DiagramError>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    // Put the input message into the diagram
    ctx.add_output_into_target(&start, scope.start.into());

    // Add the terminate operation
    ctx.set_input_for_target(
        OperationRef::Terminate(NamespaceList::default()),
        scope.terminate.into(),
        TraceInfo::default(),
    )?;

    let mut streams = DynStreamInputPack::default();
    Streams::into_dyn_stream_input_pack(&mut streams, scope.streams);
    for (name, input) in streams.named {
        // TODO(@mxgrey): The trace settings for stream_out are not properly
        // based on whatever the user sets in the StreamOutSchema.
        let name: Arc<str> = name.as_ref().into();
        ctx.set_input_for_target(
            OperationRef::stream_out(&name),
            input,
            TraceInfo::default(),
        )?;
    }

    // Add the dispose operation
    ctx.set_connect_into_target(OperationRef::Dispose, ConnectToDispose)?;

    // Add the cancel operation
    let connect_to_cancel = ConnectToCancel::new(ctx.builder)?;
    ctx.set_connect_into_target(
        OperationRef::Cancel(NamespaceList::default()),
        connect_to_cancel,
    )?;

    Ok(())
}

struct ConnectToDispose;

impl ConnectIntoTarget for ConnectToDispose {
    fn connect_into_target(
        &mut self,
        _output: DynOutput,
        _ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        Ok(())
    }
}

/// This returns an opaque [`ConnectIntoTarget`] implementation that provides
/// the standard behavior of an input slot that other operations are connecting
/// into.
pub fn standard_input_connection(
    input_slot: DynInputSlot,
    registry: &DiagramElementRegistry,
) -> Result<Box<dyn ConnectIntoTarget + 'static>, DiagramErrorCode> {
    if input_slot.message_info() == &TypeInfo::of::<JsonMessage>() {
        return Ok(Box::new(ImplicitSerialization::new(input_slot)?));
    }

    if let Some(deserialization) = ImplicitDeserialization::try_new(input_slot, &registry.messages)?
    {
        // The target type is deserializable, so let's apply implicit deserialization
        // to it.
        return Ok(Box::new(deserialization));
    }

    Ok(Box::new(BasicConnect::new(input_slot)))
}

impl ConnectIntoTarget for ImplicitSerialization {
    fn connect_into_target(
        &mut self,
        output: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        self.implicit_serialize(output, ctx)
    }
}

impl ConnectIntoTarget for ImplicitDeserialization {
    fn connect_into_target(
        &mut self,
        output: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        self.implicit_deserialize(output, ctx)
    }
}

pub(crate) struct BasicConnect {
    incoming_types: HashMap<TypeInfo, DynInputSlot>,
    pub(crate) input_slot: Arc<DynInputSlot>,
}

impl BasicConnect {
    pub(crate) fn is_compatible(
        &self,
        incoming: &TypeInfo,
        ctx: &BuilderContext,
    ) -> Result<bool, DiagramErrorCode> {
        let incoming_index = ctx.registry.messages.registration.get_index_dyn(incoming)?;
        let r = ctx
            .registry
            .messages
            .get_operations(self.input_slot.message_info())?
            .from_impls.contains_key(&incoming_index);

        Ok(r)
    }

    pub(crate) fn new(input_slot: DynInputSlot) -> Self {
        Self {
            input_slot: Arc::new(input_slot),
            incoming_types: Default::default(),
        }
    }
}

impl ConnectIntoTarget for BasicConnect {
    fn connect_into_target(
        &mut self,
        output: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        if output.message_info() == self.input_slot.message_info() {
            return Ok(output.connect_to(&self.input_slot, ctx.builder)?);
        }

        if let Some(conversion) = self.incoming_types.get(output.message_info()) {
            return Ok(output.connect_to(conversion, ctx.builder)?);
        }

        let incoming_type = *output.message_info();
        let incoming_type_index = ctx.registry.messages.registration.get_index_dyn(&incoming_type)?;
        let convert = ctx
            .registry
            .messages
            .get_dyn(self.input_slot.message_info())?
            .get_operations()?
            .from_impls
            .get(&incoming_type_index)
            .ok_or_else(|| DiagramErrorCode::TypeMismatch(TypeMismatch {
                source_type: incoming_type,
                target_type: *self.input_slot.message_info(),
            }))?;

        let (conversion_input, converted_output) = (*convert)(ctx.builder);
        converted_output.connect_to(&self.input_slot, ctx.builder)?;
        output.connect_to(&conversion_input, ctx.builder)?;
        self.incoming_types.insert(incoming_type, conversion_input);
        Ok(())
    }
}

struct ConnectToCancel {
    quiet_cancel: DynInputSlot,
    implicit_serialization: ImplicitSerialization,
    implicit_stringify: ImplicitStringify,
    triggers: HashMap<TypeInfo, DynInputSlot>,
}

impl ConnectToCancel {
    fn new(builder: &mut Builder) -> Result<Self, DiagramErrorCode> {
        Ok(Self {
            quiet_cancel: builder.create_quiet_cancel().into(),
            implicit_serialization: ImplicitSerialization::new(
                builder.create_cancel::<JsonMessage>().into(),
            )?,
            implicit_stringify: ImplicitStringify::new(builder.create_cancel::<String>().into())?,
            triggers: Default::default(),
        })
    }
}

impl ConnectIntoTarget for ConnectToCancel {
    fn connect_into_target(
        &mut self,
        output: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        let Err(output) = self
            .implicit_stringify
            .try_implicit_stringify(output, ctx)?
        else {
            // We successfully converted the output into a string, so we are done.
            return Ok(());
        };

        // Try to implicitly serialize the incoming message if the message
        // type supports it. That way we can connect it to the regular
        // cancel operation.
        let Err(output) = self
            .implicit_serialization
            .try_implicit_serialize(output, ctx)?
        else {
            // We successfully converted the output into a json, so we are done.
            return Ok(());
        };

        // In this case, the message type cannot be stringified or serialized so
        // we'll change it into a trigger and then connect it to the quiet
        // cancel instead.
        let input_slot = match self.triggers.entry(*output.message_info()) {
            Entry::Occupied(occupied) => occupied.get().clone(),
            Entry::Vacant(vacant) => {
                let trigger = ctx
                    .registry
                    .messages
                    .trigger(output.message_info(), ctx.builder)?;
                trigger.output.connect_to(&self.quiet_cancel, ctx.builder)?;
                vacant.insert(trigger.input).clone()
            }
        };

        output.connect_to(&input_slot, ctx.builder)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct RedirectConnection {
    redirect_to: OperationRef,
    /// Keep track of which DynOutputs have been redirected in the past so we
    /// can identify when a circular redirection is happening.
    redirected: HashSet<Entity>,
}

impl RedirectConnection {
    pub fn new(redirect_to: OperationRef) -> Self {
        Self {
            redirect_to,
            redirected: Default::default(),
        }
    }
}

impl ConnectIntoTarget for RedirectConnection {
    fn connect_into_target(
        &mut self,
        output: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        if self.redirected.insert(output.id()) {
            // This DynOutput has not been redirected by this connector yet, so
            // we should go ahead and redirect it.
            ctx.add_output_into_target(self.redirect_to.clone(), output);
        } else {
            // This DynOutput has been redirected by this connector before, so
            // we have a circular connection, making it impossible for the
            // output to ever really be connected to anything.
            return Err(DiagramErrorCode::CircularRedirect(vec![
                self.redirect_to.clone(),
            ]));
        }
        Ok(())
    }
}

impl<'a, 'c, 'w, 's, 'b> std::fmt::Debug for BuilderContext<'a, 'c, 'w, 's, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagramContext")
            .field("construction", &DebugDiagramConstruction(self))
            .finish()
    }
}

struct DebugDiagramConstruction<'a, 'c, 'w, 's, 'b, 'd>(&'d BuilderContext<'a, 'c, 'w, 's, 'b>);

impl<'a, 'c, 'w, 's, 'b, 'd> std::fmt::Debug for DebugDiagramConstruction<'a, 'c, 'w, 's, 'b, 'd> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagramConstruction")
            .field(
                "outputs_into_target",
                &self.0.construction.outputs_into_target,
            )
            .field("buffers", &self.0.construction.buffers)
            .field(
                "generated_operations",
                &self.0.construction.generated_operations,
            )
            .finish()
    }
}

/// Information for how an operation should be traced.
#[derive(Clone, Default)]
pub struct TraceInfo {
    /// Information about how the operation was constructed.
    pub construction: Option<Arc<JsonMessage>>,
    /// Whether or not tracing should be enabled for this operation.
    pub trace: Option<TraceToggle>,
}

impl TraceInfo {
    pub fn new(
        construction: impl Serialize,
        trace: Option<TraceToggle>,
    ) -> Result<Self, DiagramErrorCode> {
        let construction = Some(Arc::new(
            serde_json::to_value(construction).map_err(|err|
                DiagramErrorCode::TraceInfoError(Arc::new(err))
            )?,
        ));

        Ok(Self {
            construction,
            trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, diagram::testing::*};
    use serde_json::json;

    #[test]
    fn test_implicit_conversion() {
        let mut fixture = DiagramTestFixture::new();

        fixture.registry.register_node_builder(
            NodeBuilderOptions::new("low_res_add"),
            |builder, config: i8| {
                builder.create_map_block(move |request: i8| {
                    request + config
                })
            }
        );

        fixture.registry.register_node_builder(
            NodeBuilderOptions::new("mid_res_add"),
            |builder, config: i32| {
                builder.create_map_block(move |request: i32| {
                    request + config
                })
            }
        );

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "low_res",
            "ops": {
                "low_res": {
                    "type": "node",
                    "builder": "low_res_add",
                    "config": 1,
                    "next": "mid_res"
                },
                "mid_res": {
                    "type": "node",
                    "builder": "mid_res_add",
                    "config": 2,
                    "next": "hi_res"
                },
                "hi_res": {
                    "type": "node",
                    "builder": "add_to",
                    "config": 3,
                    "next": { "builtin": "terminate" }
                }
            }
        }))
        .unwrap();

        let r: JsonMessage = fixture.spawn_and_run(&diagram, JsonMessage::from(1)).unwrap();
        assert_eq!(r.as_i64().unwrap(), 7);
    }
}
