/*
 * Copyright (C) 2026 Open Source Robotics Foundation
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

use smallvec::{smallvec, SmallVec};

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::Deref,
    sync::Arc,
    cmp::Ordering,
};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

use crate::{
    OutputRef, DiagramElementRegistry, DiagramErrorCode, DiagramContext, MessageOperations,
    JsonMessage, BufferIdentifier, BufferMapLayoutHints, BufferMapLayoutConstraint, AnyMessageBox,
    MessageTypeHint, BufferSelection, NamedOutputRef, BuildDiagramOperation, MessageTypeInferenceFailure,
    Operations, OperationName, NamespaceList, NextOperation, output_ref, Diagram, StreamPack, StreamAvailability,
    DiagramError, TypeInfo, IncompatibleLayout, BufferIncompatibility, OperationRef,
    NodeSchema, SectionSchema, SectionProvider, SectionError, NamespacedOperation, ScopeSchema,
    WithContext,
};

pub type InferredMessageTypes = HashMap<PortRef, usize>;

impl Diagram {
    pub fn infer_message_types<Request, Response, Streams>(
        &self,
        registry: &DiagramElementRegistry,
    ) -> Result<InferredMessageTypes, DiagramError>
    where
        Request: 'static + Send + Sync,
        Response: 'static + Send + Sync,
        Streams: StreamPack,
    {
        self.validate_operation_names()?;
        self.validate_template_usage()?;

        let root_on_implicit_error: OperationRef = (&self.on_implicit_error()).into();

        let mut inferences = Inferences::default();

        let mut unfinished_operations: Vec<UnfinishedOperation> = self
            .ops
            .iter()
            .map(|(id, op)| {
                UnfinishedOperation::new(
                    id,
                    op.clone() as Arc<dyn BuildDiagramOperation>,
                    &self.ops,
                    root_on_implicit_error.clone(),
                )
            })
            .collect();
        let mut generated_operations = Vec::new();

        while !unfinished_operations.is_empty() {
            for unfinished in unfinished_operations.drain(..) {
                let mut ctx = InferenceContext {
                    inference: &mut inferences,
                    diagram_context: DiagramContext {
                        operations: unfinished.sibling_ops.clone(),
                        templates: &self.templates,
                        on_implicit_error: &unfinished.on_implicit_error,
                        default_trace: self.default_trace,
                        namespaces: unfinished.namespaces.clone(),
                    },
                    registry,
                    generated_operations: &mut generated_operations,
                };

                unfinished.op.apply_message_type_constraints(&unfinished.id, &mut ctx)
                    .in_port(||
                        OperationRef::from(&unfinished.id)
                        .in_namespaces(&unfinished.namespaces)
                    )?;
            }

            unfinished_operations.extend(generated_operations.drain(..));
        }

        // Test for circular redirections, which are impossible to solve for and
        // also logically unsound.
        for redirect_from in inferences.redirected_input.keys() {
            let mut next = Some(redirect_from);
            let mut visited = Vec::new();

            while let Some(top) = next.take() {
                let circular = visited.contains(&top);
                visited.push(top);

                if circular {
                    return Err(DiagramErrorCode::CircularRedirect(
                        visited.into_iter().cloned().collect()
                    ).into());
                }

                next = inferences.redirected_input.get(top);
            }
        }

        let dependents = {
            let mut dependents = HashMap::<_, Vec<PortRef>>::new();
            for (id, inference) in &inferences.evaluations {
                if let Some(constraint) = &inference.constraint {
                    let ctx = ConstraintContext { inferences: &inferences, registry };
                    let dependencies = constraint.dependencies(&ctx);
                    for dependency in dependencies {
                        dependents.entry(dependency).or_default().push(id.clone());
                    }
                }
            }

            dependents
        };

        let mut queue = VecDeque::new();
        for port in inferences.evaluations.keys() {
            queue.push_back(port.clone());
        }

        set_boundary_conditions::<Request, Response, Streams>(&mut inferences, self, registry)?;

        while let Some(port) = queue.pop_front() {
            let evaluation = inferences.get_evaluation(&port).in_port(|| port.clone())?;
            if let Some(message_type) = evaluation.evaluate(&inferences, registry).in_port(|| port.clone())? {
                if Some(message_type) != evaluation.message_type {
                    // A new message type was determined for this port, so update
                    // it and notify all dependents.
                    inferences.evaluation(port.clone()).message_type = Some(message_type);
                    if let Some(deps) = dependents.get(&port) {
                        for dep in deps {
                            if !queue.contains(&dep) {
                                queue.push_back(dep.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(inferences.try_infer_types()?)
    }
}

fn set_boundary_conditions<Request, Response, Streams>(
    inferences: &mut Inferences,
    diagram: &Diagram,
    registry: &DiagramElementRegistry,
) -> Result<(), DiagramErrorCode>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    let root_on_implicit_error: OperationRef = (&diagram.on_implicit_error()).into();
    let mut generated_operations = Vec::new();

    let mut ctx = InferenceContext {
        inference: inferences,
        diagram_context: DiagramContext {
            operations: diagram.ops.clone(),
            templates: &diagram.templates,
            on_implicit_error: &root_on_implicit_error,
            default_trace: diagram.default_trace,
            namespaces: Default::default()
        },
        registry,
        generated_operations: &mut generated_operations,
    };

    // Add constraints for the start operation
    ctx.connect(OutputRef::start(), ctx.into_operation_ref(&diagram.start));
    let request_msg_index = registry.messages.registration.get_index::<Request>()?;
    let start = ctx.into_port_ref(OutputRef::start());
    ctx.fixed(start, request_msg_index);

    // Add constraints for the terminate operation
    let response_msg_index = registry.messages.registration.get_index::<Response>()?;
    let terminate = ctx.into_port_ref(OperationRef::Terminate(Default::default()));
    ctx.fixed(terminate, response_msg_index);

    let mut streams = StreamAvailability::default();
    Streams::set_stream_availability(&mut streams);
    for (name, stream_type) in streams.named_streams() {
        let stream_msg_index = registry.messages.registration.get_index_dyn(&stream_type)?;
        let stream = ctx.into_port_ref(OperationRef::stream_out(&Arc::from(name)));
        ctx.fixed(stream, stream_msg_index);
    }

    Ok(())
}

struct DebugMessageTypeChoice {
    name: &'static str,
    cost: u32,
}

impl std::fmt::Debug for DebugMessageTypeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f
            .debug_map()
            .entry(&"name", &self.name)
            .entry(&"cost", &self.cost)
            .finish()
    }
}

pub struct InferenceContext<'a, 'b> {
    inference: &'b mut Inferences,
    diagram_context: DiagramContext<'a>,
    pub registry: &'a DiagramElementRegistry,
    generated_operations: &'b mut Vec<UnfinishedOperation>,
}

impl<'a, 'b> InferenceContext<'a, 'b> {
    /// Specify exactly what message types a port may have, irrespective of
    /// any connections to other operations.
    fn fixed(
        &mut self,
        port: PortRef,
        message_type: usize,
    ) {
        self
            .inference
            .evaluations
            .entry(port)
            .or_default()
            .message_type = Some(message_type);
    }

    /// Specify that the message type of an output should be directly inferred
    /// from the message type of the input that it connects into.
    fn infer_from_downstream(
        &mut self,
        output: OutputRef,
        input: OperationRef,
    ) {
        self.inference.constrain(output.clone(), ExactMatch(input.clone().into()));
        self.connect(output, input);
    }

    /// Specify that an output connects into an input.
    fn connect(
        &mut self,
        output: OutputRef,
        input: OperationRef,
    ) {
        self.inference.connection_from.insert(output.clone(), input.clone());
        self.inference.connections_into.entry(input).or_default().insert(output);
    }

    /// Specify that an operation simply redirects its inputs into another operation
    fn redirect(
        &mut self,
        from: OperationRef,
        into: OperationRef,
    ) {
        self.inference.constrain(from.clone(), ExactMatch(into.clone().into()));
        self.inference.redirected_input.insert(from.clone(), into.clone());
        self.inference.redirections_into.entry(into).or_default().insert(from);
    }

    pub fn node(
        &mut self,
        operation_name: &OperationName,
        schema: &NodeSchema,
    ) -> Result<(), DiagramErrorCode> {
        let node = self.registry.get_node_registration(&schema.builder)?.metadata();
        let input = self.into_operation_ref(operation_name);

        // Set the exact message type of the input port
        self.fixed(input.into(), node.request);

        // Set the exact message type of the output port, and connect it to the
        // next operation.
        let output = self.into_output_ref(output_ref(operation_name).next());
        let target = self.into_operation_ref(&schema.next);
        self.fixed(output.clone().into(), node.response);
        self.connect(output, target);

        // Set the exact message type of each stream output.
        for (stream_id, stream_type) in &node.streams {
            let stream = self.into_output_ref(output_ref(operation_name).stream_out(stream_id));
            self.fixed(stream.into(), *stream_type);
        }

        // Connect each stream output to its target operation.
        for (stream_id, stream_target) in &schema.stream_out {
            let stream = self.into_output_ref(output_ref(operation_name).stream_out(stream_id));
            let stream_target = self.into_operation_ref(stream_target);
            self.connect(stream, stream_target);
        }

        Ok(())
    }

    pub fn section(
        &mut self,
        id: &OperationName,
        schema: &SectionSchema,
    ) -> Result<(), DiagramErrorCode> {
        match &schema.provider {
            SectionProvider::Builder(section_builder) => {
                let metadata = &self
                    .registry
                    .get_section_registration(section_builder)?
                    .metadata;

                for (input_name, input_metadata) in &metadata.interface.inputs {
                    let op = NextOperation::Namespace(NamespacedOperation {
                        namespace: id.clone(),
                        operation: input_name.clone()
                    });
                    let op = self.into_port_ref(&op);

                    self.fixed(op, input_metadata.message_type);
                }

                for (buffer_name, buffer_metadata) in &metadata.interface.buffers {
                    let op = NextOperation::Namespace(NamespacedOperation {
                        namespace: id.clone(),
                        operation: buffer_name.clone(),
                    });
                    let op = self.into_port_ref(&op);

                    if let Some(buffer_message_type) = buffer_metadata.message_type {
                        self.fixed(op, buffer_message_type);
                    }
                }

                for (output_name, output_metadata) in &metadata.interface.outputs {
                    let op = self.into_port_ref(
                        output_ref(id).section_output(output_name)
                    );
                    self.fixed(op, output_metadata.message_type);
                }

                for (output_name, next) in &schema.connect {
                    let op = self.into_output_ref(
                        output_ref(id).section_output(output_name)
                    );
                    let target = self.into_operation_ref(next);
                    self.connect(op, target);
                }
            }
            SectionProvider::Template(section_template) => {
                let section = self.templates.get_template(section_template)?;

                for (child_id, op) in section.ops.iter() {
                    self.add_child_operation(id, child_id, op, section.ops.clone(), None);
                }

                section.inputs.redirect(|op, next| {
                    let op = self.into_operation_ref(OperationRef::exposed_input(id, op));
                    let next = self.into_operation_ref(next.in_namespace(id));
                    self.redirect(op, next);
                    Ok(())
                })?;

                section.buffers.redirect(|op, next| {
                    let op = self.into_operation_ref(OperationRef::exposed_input(id, op));
                    let next = self.into_operation_ref(next.in_namespace(id));
                    self.redirect(op, next);
                    Ok(())
                })?;

                for expected_output in schema.connect.keys() {
                    if !section.outputs.contains(expected_output) {
                        return Err(SectionError::UnknownOutput(Arc::clone(expected_output)).into());
                    }
                }

                for output in &section.outputs {
                    if let Some(target) = schema.connect.get(output) {
                        let output = self.into_operation_ref(
                            NextOperation::Name(Arc::clone(output)).in_namespace(id)
                        );
                        let target = self.into_operation_ref(target);
                        self.redirect(output, target);
                    }
                }
            }
        }

        let inner_terminate = self.into_operation_ref(OperationRef::terminate_for(id));
        let outer_terminate = self.into_operation_ref(&NextOperation::terminate());
        self.redirect(inner_terminate, outer_terminate);

        Ok(())
    }

    pub fn scope(
        &mut self,
        id: &OperationName,
        schema: &ScopeSchema,
    ) {
        let operation = self.into_operation_ref(id);

        // The request type of this scope must exactly match the request type
        // of the starting operation.
        let start_target = self.into_operation_ref(
            OperationRef::from(&schema.start).in_namespace(id)
        );
        self.redirect(operation, start_target);

        for (stream_name, stream_target) in &schema.stream_out {
            let stream = self.into_operation_ref(
                OperationRef::scope_stream_out(id, stream_name)
            );
            let stream_target = self.into_operation_ref(stream_target);
            self.redirect(stream, stream_target);
        }

        // The terminating message type of this scope must exactly match the
        // request type of the next operation that the scope is connected to.
        let terminate = self.into_operation_ref(OperationRef::terminate_for(id));
        let terminate_target = self.into_operation_ref(&schema.next);
        self.redirect(terminate, terminate_target);

        for (child_id, op) in schema.ops.iter() {
            self.add_child_operation(id, child_id, op, schema.ops.clone(), Some(schema.on_implicit_error()));
        }
    }

    pub fn transform(
        &mut self,
        operation_name: &OperationName,
        next: &NextOperation,
    ) -> Result<(), DiagramErrorCode> {
        let json_message_index = self.registry.messages.registration.get_index::<JsonMessage>()?;
        let operation = self.into_operation_ref(operation_name);
        let output = self.into_output_ref(output_ref(operation_name).next());
        let target = self.into_operation_ref(next);

        self.fixed(operation.into(), json_message_index);
        self.fixed(output.clone().into(), json_message_index);
        self.connect(output, target);

        Ok(())
    }

    pub fn stream_out(
        &mut self,
        operation_name: &OperationName,
        stream_name: &OperationName,
    ) {
        let operation = self.into_operation_ref(operation_name);
        let stream = self.into_operation_ref(OperationRef::stream_out(stream_name));
        self.inference.constrain(operation, ExactMatch(stream.into()));
    }

    pub fn fork_clone(
        &mut self,
        operation_name: &OperationName,
        next: &[NextOperation],
    ) {
        let operation = self.into_operation_ref(operation_name);

        let mut targets = Vec::new();
        for (i, target) in next.iter().enumerate() {
            let target = self.into_operation_ref(target);
            targets.push(target.clone());

            let output = self.into_output_ref(output_ref(operation_name).next_index(i));
            self.connect(output.clone(), target);
            self.inference.constrain(output, ExactMatch(operation.clone().into()));
        }

        self.inference.constrain(operation.clone(), CloneInput { operation, targets });
    }

    pub fn result(
        &mut self,
        operation_name: &OperationName,
        ok: &NextOperation,
        err: &NextOperation,
    ) {
        let operation = self.into_operation_ref(operation_name);
        let ok_output = self.into_output_ref(output_ref(operation_name).ok());
        let err_output = self.into_output_ref(output_ref(operation_name).err());

        self.inference.constrain(ok_output.clone(), OkFrom(operation.clone()));
        self.inference.constrain(err_output.clone(), ErrFrom(operation.clone()));

        let ok_target = self.into_operation_ref(ok);
        let err_target = self.into_operation_ref(err);
        self.inference.constrain(
            operation.clone(),
            ResultInto{
                operation,
                ok: ok_target.clone(),
                err: err_target.clone(),
            }
        );

        self.connect(ok_output, ok_target);
        self.connect(err_output, err_target);
    }

    pub fn unzip<'u>(
        &mut self,
        unzippable_name: &OperationName,
        targets: impl Iterator<Item=&'u NextOperation>,
    ) {
        let unzippable = self.into_operation_ref(unzippable_name);
        for (i, target) in targets.enumerate() {
            let element_output = self.into_output_ref(
                output_ref(unzippable_name).next_index(i)
            );

            self.inference.constrain(
                element_output.clone(),
                UnzipOutput { op: unzippable.clone(), element: i }
            );

            let target = self.into_operation_ref(target);
            self.connect(element_output, target);
        }

        self.inference.constrain(unzippable.clone(), UnzipInput(unzippable));
    }

    pub fn buffer(
        &mut self,
        operation_name: &OperationName,
        serialize: bool,
    ) -> Result<(), DiagramErrorCode> {
        let operation = self.into_operation_ref(operation_name);
        if serialize {
            let json_index = self.registry.messages.registration.get_index::<JsonMessage>()?;
            self.fixed(operation.into(), json_index);
        } else {
            self.inference.constrain(operation.clone(), BufferInput(operation));
        }

        Ok(())
    }

    pub fn join(
        &mut self,
        operation_name: &OperationName,
        selection: &BufferSelection,
        next: &NextOperation,
        serialize: bool,
    ) -> Result<(), DiagramErrorCode> {
        let output = self.into_output_ref(output_ref(operation_name).next());
        let target = self.into_operation_ref(next);
        let evaluate = |context: &ConstraintContext, msg: usize, member: &BufferIdentifier| {
            let Some(join) = &context.operations_of(msg)?.join else {
                return Err(DiagramErrorCode::NotJoinable(
                    context.type_info_for(msg)?
                ));
            };

            evaluate_buffer_hint(&join.layout, member)
        };

        for (member, buffer) in selection.iter() {
            let buffer = self.into_operation_ref(buffer);
            self.inference.buffer_hints.entry(buffer).or_default().push(
                BufferInference {
                    used_by: output.clone().into(),
                    member: member.to_owned(),
                    evaluate,
                }
            );
        }

        if serialize {
            let json_message_index = self.registry.messages.registration.get_index::<JsonMessage>()?;
            self.fixed(output.clone().into(), json_message_index);
            self.connect(output, target);
        } else {
            self.infer_from_downstream(output, target);

            // TODO(@mxgrey): Consider applying a reverse constraint: inferring the
            // Joined type based on its constituent buffers. Unfortunately the Joined
            // type is ambiguous when basing it only on the parts that are being
            // joined into it, even accounting for the layout. That makes the
            // reverse inference both weak and expensive to calculate, so we skip it
            // for now.
        }
        Ok(())
    }

    pub fn buffer_access(
        &mut self,
        operation_name: &OperationName,
        selection: &BufferSelection,
        next: &NextOperation,
    ) {
        let operation = self.into_operation_ref(operation_name);
        let output = self.into_output_ref(output_ref(operation_name).next());
        let target = self.into_operation_ref(next);
        let evaluate = |context: &ConstraintContext, msg: usize, member: &BufferIdentifier| {
            let layout = match &context.operations_of(msg)?.buffer_access {
                Some(access) => &access.layout,
                None => return Err(DiagramErrorCode::CannotAccessBuffers(
                    context.type_info_for(msg)?
                )),
            };

            evaluate_buffer_hint(layout, member)
        };

        for (member, buffer) in selection.iter() {
            let buffer = self.into_operation_ref(buffer);
            self.inference.buffer_hints.entry(buffer.into()).or_default().push(
                BufferInference {
                    used_by: output.clone().into(),
                    member: member.to_owned(),
                    evaluate,
                }
            );
        }

        self.infer_from_downstream(output, target.clone());
        self.inference.constrain(operation, BufferAccessInput { target });
    }

    pub fn listen(
        &mut self,
        operation_name: &OperationName,
        selection: &BufferSelection,
        next: &NextOperation,
    ) {
        let output = self.into_output_ref(output_ref(operation_name).next());
        let target = self.into_operation_ref(next);
        let evaluate = |context: &ConstraintContext, msg: usize, member: &BufferIdentifier| {
            let Some(listen) = &context.operations_of(msg)?.listen else {
                return Err(DiagramErrorCode::CannotListen(
                    context.type_info_for(msg)?
                ));
            };

            evaluate_buffer_hint(&listen.layout, member)
        };

        for (member, buffer) in selection.iter() {
            let buffer = self.into_operation_ref(buffer);
            self.inference.buffer_hints.entry(buffer).or_default().push(
                BufferInference {
                    used_by: output.clone().into(),
                    member: member.to_owned(),
                    evaluate,
                }
            );
        }

        self.infer_from_downstream(output, target);
    }

    pub fn split<'s>(
        &mut self,
        split_name: &OperationName,
        sequential: &Vec<NextOperation>,
        keyed: &HashMap<OperationName, NextOperation>,
        remaining: &Option<NextOperation>,
    ) {
        let split = self.into_operation_ref(split_name);

        for (i, target) in sequential.iter().enumerate() {
            let output = self.into_output_ref(
                output_ref(split_name).next_index(i)
            );

            self.inference.constrain(output.clone(), SplitOutput(split.clone()));

            let target = self.into_operation_ref(target);
            self.connect(output, target);
        }

        for (key, target) in keyed {
            let output = self.into_output_ref(
                output_ref(split_name).keyed(key)
            );

            self.inference.constrain(output.clone(), SplitOutput(split.clone()));

            let target = self.into_operation_ref(target);
            self.connect(output, target);
        }

        if let Some(target) = remaining {
            let output = self.into_output_ref(
                output_ref(split_name).remaining()
            );

            self.inference.constrain(output.clone(), SplitOutput(split.clone()));

            let target = self.into_operation_ref(target);
            self.connect(output, target);
        }

        self.inference.constrain(split.clone(), SplitInput(split));
    }

    /// Add an operation that exists as a child inside another operation.
    pub fn add_child_operation<T: BuildDiagramOperation + 'static>(
        &mut self,
        id: &OperationName,
        child_id: &OperationName,
        op: &Arc<T>,
        sibling_ops: Operations,
        on_implicit_error: Option<NextOperation>,
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

        self.generated_operations
            .push(UnfinishedOperation {
                id: Arc::clone(child_id),
                namespaces,
                op: op.clone() as Arc<dyn BuildDiagramOperation>,
                sibling_ops,
                on_implicit_error,
            })
    }

}

#[derive(Clone, Copy)]
pub struct ConstraintContext<'a> {
    inferences: &'a Inferences,
    pub registry: &'a DiagramElementRegistry,
}

impl<'a> ConstraintContext<'a> {
    pub fn get_inference_of(
        &self,
        port: impl Into<PortRef>,
    ) -> Result<&Option<usize>, DiagramErrorCode> {
        let port = port.into();
        let one_of = &self
            .inferences
            .evaluations
            .get(&port)
            .ok_or_else(move || DiagramErrorCode::UnknownPort(port))?
            .message_type;

        Ok(one_of)
    }

    pub fn connections_into(&self, operation: &OperationRef) -> SmallVec<[OutputRef; 8]> {
        let Some(connections) = self.inferences.connections_into.get(operation) else {
            return smallvec![];
        };

        connections.iter().cloned().collect()
    }

    fn get_message_types_into(
        &self,
        input: &OperationRef,
    ) -> Result<SmallVec<[usize; 8]>, DiagramErrorCode> {
        self.impl_get_message_types_into(input, true)
    }

    fn try_get_message_types_into(
        &self,
        input: &OperationRef,
    ) -> Result<SmallVec<[usize; 8]>, DiagramErrorCode> {
        self.impl_get_message_types_into(input, false)
    }

    fn impl_get_message_types_into(
        &self,
        input: &OperationRef,
        connection_necessary: bool,
    ) -> Result<SmallVec<[usize; 8]>, DiagramErrorCode> {
        let mut message_types = SmallVec::new();
        let mut input_queue: SmallVec<[&OperationRef; 8]> = SmallVec::new();
        input_queue.push(input);
        let mut visited = HashSet::new();

        while let Some(input) = input_queue.pop() {
            if !visited.insert(input) {
                return Err(DiagramErrorCode::CircularRedirect(
                    visited.into_iter().cloned().collect()
                ));
            }

            let connections = self.inferences.connections_into.get(input);
            let redirections = self.inferences.redirections_into.get(input);

            let no_connections = connections.is_none_or(|c| c.is_empty());
            let no_redirections = redirections.is_none_or(|r| r.is_empty());
            if connection_necessary && no_connections && no_redirections {
                return Err(DiagramErrorCode::NoConnection(input.clone()));
            }

            if let Some(connections) = connections {
                for connection in connections {
                    if let Some(message_type) = self.get_inference_of(connection.clone())? {
                        message_types.push(*message_type);
                    }
                }
            }

            if let Some(redirections) = redirections {
                for redirect in redirections {
                    input_queue.push(redirect);
                }
            }
        }

        message_types.sort();
        message_types.dedup();
        Ok(message_types)
    }

    pub fn evaluate_clone_input(
        &self,
        operation: &OperationRef,
        targets: &[OperationRef],
    ) -> MessageTypeEvaluation {
        let incoming_message_types = self.get_message_types_into(operation)?;
        let selected_input_type = if incoming_message_types.is_empty() {
            // We don't have any upstream hints for the message type, so we
            // should attempt to infer something from our target message types.
            let Some(message_type) = self.same_output_type(targets)? else {
                return Ok(None);
            };

            message_type
        } else if incoming_message_types.len() == 1 {
            // There's exactly one incoming message type so we should settle on
            // that as the message type.

            // SAFETY: We already verified that this has a value
            *incoming_message_types.first().unwrap()
        } else {
            // We need to disambiguate the multiple incoming message types.
            if let Some(output_message_type) = self.same_output_type(targets)? {
                // Check if all incoming message types can be converted to the
                // output message type. If they can, use that type.
                let mut compatible = true;
                for incoming in &incoming_message_types {
                    compatible &= self.operations_of(*incoming)?.into_impls.get(&output_message_type).is_some();
                }

                if !compatible {
                    return Err(DiagramErrorCode::AmbiguousMessageType(
                        self.type_info_for_slice(&incoming_message_types)?
                    ));
                }

                output_message_type
            } else {
                // There isn't a clear choice for the output message type, so we
                // should see if we can resolve the incoming types.
                for incoming in &incoming_message_types {
                    if self.operations_of(*incoming)?.serialize.is_none() {
                        return Err(DiagramErrorCode::AmbiguousMessageType(
                            self.type_info_for_slice(&incoming_message_types)?
                        ));
                    }
                }

                // These are all serializable, so we should choose JsonMessage
                // if it's an option.
                let Ok(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() else {
                    return Err(DiagramErrorCode::AmbiguousMessageType(
                        self.type_info_for_slice(&incoming_message_types)?
                    ));
                };

                // TODO(@mxgrey): We could consider finding a different single
                // common type that all incoming messages can be converted into,
                // but there's a significant risk of ambiguity for that.

                json_index
            }
        };

        if self.operations_of(selected_input_type)?.fork_clone.is_none() {
            return Err(DiagramErrorCode::NotCloneable(
                self.registry.messages.get_type_info_for(selected_input_type)?
            ));
        }

        Ok(Some(selected_input_type))
    }

    /// Find a single output type that is compatible with all the target inputs,
    /// i.e. either the same message type being sent to all of them, or a single
    /// message type that can be converted to all of them.
    pub fn same_output_type(
        &self,
        targets: &[OperationRef],
    ) -> MessageTypeEvaluation {
        let mut message_types: SmallVec<[usize; 8]> = Default::default();
        for target in targets {
            let port: PortRef = target.clone().into();
            if let Some(message_type) = self.inferences.get_evaluation(&port)?.message_type {
                message_types.push(message_type);
            }
        }

        message_types.sort();
        message_types.dedup();
        if message_types.len() <= 1 {
            return Ok(message_types.pop());
        }

        // There is more than one message type consider. If all of them can be
        // deserialized then we will use JsonMessage.
        let Ok(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() else {
            // This is only meant to be a hint, so no need to call it an error.
            return Ok(None);
        };

        for message_type in message_types {
            if self.operations_of(message_type)?.deserialize.is_none() {
                // Cannot deserialize all of the target messages, so we can't
                // choose a clear outgoing message. This is only meant to be a
                // hint, so we don't treat it as an error.
                return Ok(None);
            }
        }

        // All target types are deserializable, so choose JsonMessage
        Ok(Some(json_index))

        // TODO(@mxgrey): We could consider finding a different single common
        // type that all target message types can be converted from, but there's
        // a signfiicant risk of ambiguity for that.
    }

    pub fn evaluate_result_input(
        &self,
        operation: &OperationRef,
        ok_target: &OperationRef,
        err_target: &OperationRef,
    ) -> MessageTypeEvaluation {
        let incoming_message_types = self.get_message_types_into(operation)?;
        let selected_input_type = if incoming_message_types.is_empty() {
            // We don't have any upstream hints for the message type, so we
            // should attempt to infer something from our target message types.
            let Some(ok_inference) = self.get_inference_of(ok_target.clone())? else {
                return Ok(None);
            };

            let Some(err_inference) = self.get_inference_of(err_target.clone())? else {
                return Ok(None);
            };

            let key = [*ok_inference, *err_inference];
            let Some(r) = self.registry.messages.registration.lookup.result.get(&key) else {
                // We can't conclude anything yet.
                return Ok(None);
            };

            *r
        } else if incoming_message_types.len() == 1 {
            *incoming_message_types.first().unwrap()
        } else {
            return Ok(None);
        };

        if self.operations_of(selected_input_type)?.fork_result.is_none() {
            return Err(DiagramErrorCode::CannotForkResult(
                self.type_info_for(selected_input_type)?
            ));
        }

        Ok(Some(selected_input_type))
    }

    pub fn evaluate_ok_from(
        &self,
        from_result: &OperationRef,
    ) -> MessageTypeEvaluation {
        self.evaluate_result_output(from_result, 0)
    }

    pub fn evaluate_err_from(
        &self,
        from_result: &OperationRef,
    ) -> MessageTypeEvaluation {
        self.evaluate_result_output(from_result, 1)
    }

    fn evaluate_result_output(
        &self,
        from_result: &OperationRef,
        index: usize,
    ) -> MessageTypeEvaluation {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let Some(r) = &self.operations_of(*result_inference)?.fork_result else {
            return Err(DiagramErrorCode::CannotForkResult(
                self.type_info_for(*result_inference)?
            ));
        };

        Ok(Some(r.output_types[index]))
    }

    pub fn evaluate_unzip_input(
        &self,
        operation: &OperationRef,
    ) -> MessageTypeEvaluation {
        let incoming_message_types = self.get_message_types_into(operation)?;
        if incoming_message_types.len() > 1 {
            return Err(DiagramErrorCode::AmbiguousMessageType(
                self.type_info_for_slice(&incoming_message_types)?
            ));
        }

        let Some(unzip_msg) = incoming_message_types.first() else {
            return Ok(None);
        };

        if !self.operations_of(*unzip_msg)?.unzip.is_some() {
            return Err(DiagramErrorCode::NotUnzippable(
                self.type_info_for(*unzip_msg)?
            ));
        }

        Ok(Some(*unzip_msg))

        // TODO(@mxgrey): We could consider backing out the unzip type based on
        // the target unzipped message types, but this seems unnecessary for now.
        // Unzip is a relatively specialized operation at the moment.
    }

    pub fn evaluate_unzip_output(
        &self,
        input: &OperationRef,
        element: usize,
    ) -> MessageTypeEvaluation {
        let Some(unzip_inference) = self.get_inference_of(input.clone())? else {
            return Ok(None);
        };

        let Some(unzip_op) = &self.operations_of(*unzip_inference)?.unzip else {
            return Err(DiagramErrorCode::NotUnzippable(
                self.registry.messages.get_type_info_for(*unzip_inference)?
            ))
        };

        let Some(id) = unzip_op.output_types.get(element).copied() else {
            return Err(DiagramErrorCode::InvalidUnzip {
                message: self.type_info_for(*unzip_inference)?,
                element,
            });
        };

        Ok(Some(id))
    }

    pub fn evaluate_buffer_input(
        &self,
        input: &OperationRef,
    ) -> MessageTypeEvaluation {
        let incoming_message_types = self.try_get_message_types_into(input)?;
        if incoming_message_types.is_empty() {
            // There might not be any direct inputs to this buffer, so check for
            // buffer hints from accessors.
            let Some(hints) = self.inferences.buffer_hints.get(input) else {
                // Keep waiting for more operations to be inferred.
                return Ok(None);
            };

            let mut hint_message_types: SmallVec<[usize; 8]> = SmallVec::new();
            for hint in hints {
                if let Some(evaluation) = hint.evaluate(self)? {
                    hint_message_types.push(evaluation);
                }
            }

            hint_message_types.sort();
            hint_message_types.dedup();

            if hint_message_types.len() <= 1 {
                return Ok(hint_message_types.first().cloned());
            } else {
                return Err(DiagramErrorCode::AmbiguousMessageType(
                    self.type_info_for_slice(&hint_message_types)?
                ));
            }
        } else if incoming_message_types.len() == 1 {
            return Ok(incoming_message_types.first().cloned());
        } else {
            return Err(DiagramErrorCode::AmbiguousMessageType(
                self.type_info_for_slice(&incoming_message_types)?
            ));
        }
    }

    pub fn evaluate_buffer_access_input(
        &self,
        target: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(target_inference) = self.get_inference_of(target.clone())? else {
            return Ok(None);
        };

        if let Some(buffer_access) = &self.operations_of(*target_inference)?.buffer_access {
            return Ok(Some(buffer_access.request_message));
        } else {
            return Err(DiagramErrorCode::CannotAccessBuffers(
                self.type_info_for(*target_inference)?
            ));
        }
    }

    pub fn evaluate_split_input(
        &self,
        operation: &OperationRef,
    ) -> MessageTypeEvaluation {
        let incoming_message_types = self.get_message_types_into(operation)?;
        if incoming_message_types.len() <= 1 {
            if let Some(incoming_message_type) = incoming_message_types.first() {
                let ops = self.operations_of(*incoming_message_type)?;
                if ops.split.is_none() && ops.serialize.is_some() {
                    // The message cannot be split but it can be serialized, so
                    // we should change it to a JsonMessage.
                    let Ok(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() else {
                        return Err(DiagramErrorCode::NotSplittable(
                            self.type_info_for(*incoming_message_type)?
                        ));
                    };

                    return Ok(Some(json_index));
                } else if ops.split.is_none() {
                    // The message cannot be split and cannot be serialized, so
                    // it is not a valid choice for the split operation.
                    return Err(DiagramErrorCode::NotSplittable(
                        self.type_info_for(*incoming_message_type)?
                    ));
                }
            }

            return Ok(incoming_message_types.first().copied());
        }

        let Ok(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() else {
            return Err(DiagramErrorCode::AmbiguousMessageType(
                self.type_info_for_slice(&incoming_message_types)?
            ));
        };

        // Check if all incoming messages can be serialized. If so, we can funnel
        // them into a JsonMessage before splitting.
        for incoming_message_type in &incoming_message_types {
            if self.operations_of(*incoming_message_type)?.serialize.is_none() {
                return Err(DiagramErrorCode::AmbiguousMessageType(
                    self.type_info_for_slice(&incoming_message_types)?
                ));
            }
        }

        // All incoming message types can be serialized, so take in a JsonMessage instead.
        Ok(Some(json_index))
    }

    pub fn evaluate_split_output(
        &self,
        operation: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(split_inference) = self.get_inference_of(operation.clone())? else {
            return Ok(None);
        };

        let Some(split) = &self.operations_of(*split_inference)?.split else {
            return Err(DiagramErrorCode::NotSplittable(
                self.type_info_for(*split_inference)?
            ));
        };

        Ok(Some(split.output_type))
    }

    pub fn operations_of(
        &self,
        message_type_index: usize,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        self
            .registry
            .messages
            .registration
            .get_by_index(message_type_index)?
            .get_operations()
    }

    pub fn type_info_for(
        &self,
        index: usize,
    ) -> Result<TypeInfo, DiagramErrorCode> {
        Ok(self.registry.messages.registration.get_by_index(index)?.type_info)
    }

    pub fn type_info_for_slice(
        &self,
        indices: &[usize],
    ) -> Result<Vec<TypeInfo>, DiagramErrorCode> {
        let mut info = Vec::new();
        for index in indices {
            info.push(self.type_info_for(*index)?);
        }

        Ok(info)
    }
}

fn evaluate_buffer_hint(
    layout: &BufferMapLayoutHints<usize>,
    member: &BufferIdentifier,
) -> MessageTypeEvaluation {
    match layout {
        BufferMapLayoutHints::Dynamic(dynamic) => {
            if dynamic.is_compatible(member) {
                if let Some(hint) = &dynamic.hint.map(|h| h.as_exact()).flatten() {
                    return Ok(Some(*hint));
                }

                return Ok(None);
            }

            let mut incompatibility = IncompatibleLayout::default();
            incompatibility.forbidden_buffers.push(member.to_owned());
            return Err(DiagramErrorCode::IncompatibleBuffers(
                incompatibility
            ));
        }
        BufferMapLayoutHints::Static(hints) => {
            if let Some(hint) = hints.get(member) {
                return Ok(hint.as_exact());
            }
        }
    }

    Ok(None)
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
    /// Where implicit errors should be connected if not overridden
    on_implicit_error: OperationRef,
}

impl UnfinishedOperation {
    fn new(
        id: &OperationName,
        op: Arc<dyn BuildDiagramOperation>,
        sibling_ops: &Operations,
        on_implicit_error: OperationRef,
    ) -> Self {
        Self {
            id: Arc::clone(id),
            op,
            sibling_ops: sibling_ops.clone(),
            on_implicit_error,
            namespaces: Default::default(),
        }
    }
}

impl<'a, 'b> Deref for InferenceContext<'a, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

pub type MessageTypeEvaluation = Result<Option<usize>, DiagramErrorCode>;

pub trait MessageTypeConstraint: std::fmt::Debug + 'static + Send + Sync {
    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation;

    fn dependencies(
        &self,
        context: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]>;
}

#[derive(Debug, Default, Clone)]
struct MessageTypeInference {
    message_type: Option<usize>,
    constraint: Option<Arc<dyn MessageTypeConstraint>>,
}

impl MessageTypeInference {
    fn evaluate(
        &self,
        inference: &Inferences,
        registry: &DiagramElementRegistry,
    ) -> MessageTypeEvaluation {
        let Some(constraint) = &self.constraint else {
            return Ok(None);
        };

        let ctx = ConstraintContext { inferences: inference, registry };
        constraint.evaluate(&ctx)
    }
}

/// An input or output port of an operation.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub enum PortRef {
    Input(OperationRef),
    Output(OutputRef),
}

impl PortRef {
    pub fn in_namespaces(self, parent_namespaces: &[Arc<str>]) -> Self {
        match self {
            Self::Input(input) => Self::Input(input.in_namespaces(parent_namespaces)),
            Self::Output(output) => Self::Output(output.in_namespaces(parent_namespaces)),
        }
    }
}

impl std::fmt::Display for PortRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(input) => {
                write!(f, "(input) {input}")
            }
            Self::Output(output) => {
                write!(f, "(output) {output}")
            }
        }
    }
}

impl std::fmt::Debug for PortRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl<T: Into<OperationRef>> From<T> for PortRef {
    fn from(value: T) -> Self {
        let op: OperationRef = value.into();
        Self::Input(op.into())
    }
}

impl From<OutputRef> for PortRef {
    fn from(value: OutputRef) -> Self {
        Self::Output(value)
    }
}

impl From<NamedOutputRef> for PortRef {
    fn from(value: NamedOutputRef) -> Self {
        Self::Output(value.into())
    }
}

#[derive(Debug, Default)]
struct Inferences {
    evaluations: HashMap<PortRef, MessageTypeInference>,
    connections_into: HashMap<OperationRef, HashSet<OutputRef>>,
    connection_from: HashMap<OutputRef, OperationRef>,
    redirected_input: HashMap<OperationRef, OperationRef>,
    redirections_into: HashMap<OperationRef, HashSet<OperationRef>>,
    buffer_hints: HashMap<OperationRef, Vec<BufferInference>>,
}

struct BufferInference {
    used_by: PortRef,
    member: BufferIdentifier<'static>,
    evaluate: EvaluateBufferLayoutHintFn,
}

impl BufferInference {
    pub fn evaluate(
        &self,
        ctx: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        let Some(target_inference) = ctx.get_inference_of(self.used_by.clone())? else {
            return Ok(None);
        };

        (self.evaluate)(ctx, *target_inference, &self.member)
    }
}

impl std::fmt::Debug for BufferInference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferInference")
            .field("used_by", &self.used_by)
            .field("member", &self.member)
            .finish()
    }
}

type EvaluateBufferLayoutHintFn = fn(
    &ConstraintContext,
    usize,
    &BufferIdentifier,
) -> MessageTypeEvaluation;

impl Inferences {
    fn evaluation(&mut self, key: impl Into<PortRef>) -> &mut MessageTypeInference {
        let key = key.into();
        self.evaluations.entry(key).or_default()
    }

    fn constrain(
        &mut self,
        key: impl Into<PortRef>,
        constraint: impl MessageTypeConstraint,
    ) {
        let port_constraint = &mut self.evaluation(key).constraint;

        // TODO(@mxgrey): Remove this assertion when done testing
        assert!(port_constraint.is_none());
        *port_constraint = Some(Arc::new(constraint));
    }

    fn get_evaluation(&self, key: &PortRef) -> Result<&MessageTypeInference, DiagramErrorCode> {
        self
            .evaluations
            .get(&key)
            .ok_or_else(|| DiagramErrorCode::UnknownPort(key.clone()))
    }

    fn try_infer_types(&self) -> Result<InferredMessageTypes, DiagramErrorCode> {
        let mut inferred = InferredMessageTypes::new();
        for (port, evaluation) in &self.evaluations {
            let Some(message_type) = evaluation.message_type else {
                return Err(DiagramErrorCode::CannotInferType(port.clone()));
            };
            inferred.insert(port.clone(), message_type);
        }
        Ok(inferred)
    }
}

#[derive(Debug)]
struct ExactMatch(PortRef);

impl MessageTypeConstraint for ExactMatch {
    fn dependencies(
        &self,
        _: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.get_inference_of(self.0.clone()).cloned()
    }
}

#[derive(Debug)]
struct CloneInput {
    operation: OperationRef,
    targets: Vec<OperationRef>,
}

impl MessageTypeConstraint for CloneInput {
    fn dependencies(
        &self,
        context: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        self.targets
            .iter()
            .cloned()
            .map(Into::into)
            .chain(context.connections_into(&self.operation).into_iter().map(Into::into))
            .collect()
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_clone_input(&self.operation, &self.targets)
    }
}

#[derive(Debug)]
struct ResultInto {
    operation: OperationRef,
    ok: OperationRef,
    err: OperationRef,
}

impl MessageTypeConstraint for ResultInto {
    fn dependencies(
        &self,
        context: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        context
            .connections_into(&self.operation)
            .into_iter()
            .map(Into::into)
            .chain([self.ok.clone().into(), self.err.clone().into()])
            .collect()
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_result_input(&self.operation, &self.ok, &self.err)
    }
}

#[derive(Debug)]
struct OkFrom(OperationRef);

impl MessageTypeConstraint for OkFrom {
    fn dependencies(
        &self,
        _: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_ok_from(&self.0)
    }
}

#[derive(Debug)]
struct ErrFrom(OperationRef);

impl MessageTypeConstraint for ErrFrom {
    fn dependencies(
        &self,
        _: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_err_from(&self.0)
    }
}

#[derive(Debug)]
struct UnzipInput(OperationRef);

impl MessageTypeConstraint for UnzipInput {
    fn dependencies(
        &self,
        context: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        context.connections_into(&self.0).into_iter().map(Into::into).collect()
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_unzip_input(&self.0)
    }
}

#[derive(Debug)]
struct UnzipOutput {
    op: OperationRef,
    element: usize,
}

impl MessageTypeConstraint for UnzipOutput {
    fn dependencies(
        &self,
        _: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        smallvec![self.op.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_unzip_output(&self.op, self.element)
    }
}

#[derive(Debug)]
struct BufferInput(OperationRef);

impl MessageTypeConstraint for BufferInput {
    fn dependencies(
        &self,
        context: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        let mut deps = SmallVec::new();
        deps.extend(context.connections_into(&self.0).into_iter().map(Into::into));
        if let Some(hints) = context.inferences.buffer_hints.get(&self.0) {
            for hint in hints {
                deps.push(hint.used_by.clone());
            }
        }

        deps
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_buffer_input(&self.0)
    }
}

#[derive(Debug)]
struct BufferAccessInput {
    target: OperationRef,
}

impl MessageTypeConstraint for BufferAccessInput {
    fn dependencies(
        &self,
        _: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        smallvec![self.target.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_buffer_access_input(&self.target)
    }
}

#[derive(Debug)]
struct SplitInput(OperationRef);

impl MessageTypeConstraint for SplitInput {
    fn dependencies(
        &self,
        context: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        context.connections_into(&self.0).into_iter().map(Into::into).collect()
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_split_input(&self.0)
    }
}

#[derive(Debug)]
struct SplitOutput(OperationRef);

impl MessageTypeConstraint for SplitOutput {
    fn dependencies(
        &self,
        _: &ConstraintContext,
    ) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        ctx: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        ctx.evaluate_split_output(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, testing::*, diagram::testing::*};
    use serde_json::json;

    #[test]
    fn test_split_type_inference() {
        let mut fixture = DiagramTestFixture::new();

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "split",
            "ops": {
                "x10-buffer": {
                    "type": "buffer"
                },
                "x100-buffer": {
                    "type": "buffer"
                },
                "split": {
                    "type": "split",
                    "sequential": [
                        "x100",
                        "x10"
                    ]
                },
                "x10": {
                    "type": "node",
                    "builder": "mul",
                    "next": "x10-buffer",
                    "config": 10,
                    "display_text": "x10"
                },
                "join": {
                    "type": "join",
                    "buffers": [
                        "x100-buffer",
                        "x10-buffer"
                    ],
                    "next": "sum"
                },
                "sum": {
                    "type": "node",
                    "builder": "add",
                    "next": {
                        "builtin": "terminate"
                    },
                    "display_text": "sum"
                },
                "x100": {
                    "type": "node",
                    "builder": "mul",
                    "next": "x100-buffer",
                    "config": 100,
                    "display_text": "x100"
                }
            }
        }))
        .unwrap();

        let inference = diagram.infer_message_types::<JsonMessage, JsonMessage, ()>(&fixture.registry).unwrap();
        dbg!(inference);

        for (i, r) in fixture.registry.messages.registration.iter().enumerate() {
            println!("\n - {i}. {}", r.type_info.type_name);
        }

    }

    #[test]
    fn test_fork_clone_type_inference() {
        let mut fixture = DiagramTestFixture::new();

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "fork_clone",
            "ops": {
                "fork_clone": {
                    "type": "fork_clone",
                    "next": ["add_one", "add_two"]
                },
                "add_one": {
                    "type": "node",
                    "builder": "add_to",
                    "config": 1,
                    "next": "buffer_one"
                },
                "add_two": {
                    "type": "node",
                    "builder": "add_to",
                    "config": 2,
                    "next": "buffer_two"
                },
                "buffer_one": { "type": "buffer" },
                "buffer_two": { "type": "buffer" },
                "join": {
                    "type": "join",
                    "buffers": [
                        "buffer_one",
                        "buffer_two"
                    ],
                    "next": "multiply"
                },
                "multiply": {
                    "type": "node",
                    "builder": "mul",
                    "next": { "builtin": "terminate" }
                }
            }
        }))
        .unwrap();

        let inference = diagram.infer_message_types::<JsonMessage, JsonMessage, ()>(&fixture.registry).unwrap();
        dbg!(inference);
    }

}
