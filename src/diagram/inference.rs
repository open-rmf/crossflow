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
    collections::{HashMap, BTreeMap, VecDeque},
    ops::Deref,
    sync::Arc,
    cmp::Ordering,
};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

use crate::{
    OperationRef, OutputRef, DiagramElementRegistry, DiagramErrorCode, DiagramContext, MessageOperations,
    JsonMessage, BufferIdentifier, BufferMapLayoutHints, BufferMapLayoutConstraint, AnyMessageBox,
    MessageTypeHint, BufferSelection, NamedOutputRef, BuildDiagramOperation, MessageTypeInferenceFailure,
    Operations, OperationName, NamespaceList, NextOperation, output_ref, Diagram, StreamPack, StreamAvailability,
    DiagramError,
};

pub type InferredMessageTypes = HashMap<PortRef, usize>;

impl Diagram {
    pub fn infer_message_types<Request, Response, Streams>(
        &self,
        registry: &DiagramElementRegistry,
    ) -> Result<InferredMessageTypes, DiagramErrorCode>
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
                // println!("{id}: {op:#?}");
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
                        namespaces: unfinished.namespaces,
                    },
                    registry,
                    generated_operations: &mut generated_operations,
                };

                unfinished.op.apply_message_type_constraints(&unfinished.id, &mut ctx)?;
            }

            unfinished_operations.extend(generated_operations.drain(..));
        }

        let dependents = {
            let mut dependents = HashMap::<_, Vec<PortRef>>::new();
            for (id, inference) in &inferences.evaluations {
                for constraint in inference.constraints.values().flatten() {
                    let dependencies = constraint.dependencies();
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

        loop {
            // println!("queue: {}", super::format_list(&Vec::from_iter(queue.iter())));
            while let Some(port) = queue.pop_front() {
                println!("\n-----\nqueue: {}", super::format_list(&Vec::from_iter(queue.iter())));
                println!("evaluating {port}");
                let evaluation = inferences.get_evaluation(&port)?;

                if let Some(reduction) = evaluation.evaluate(&inferences, registry)? {
                    let evaluation = inferences.evaluation(port.clone());

                    let previous = as_type_name(&evaluation.one_of, registry)?;

                    if evaluation.reduce(reduction, registry) {
                        let new = as_type_name(&evaluation.one_of, registry)?;
                        println!("reduced {port}: {previous:?} -> {new:?}");
                        // The choices for this port have been reduced. We should
                        // add its dependents to the queue if they're not in already.
                        if let Some(deps) = dependents.get(&port) {
                            for dep in deps {
                                if !queue.contains(&dep) {
                                    queue.push_back(dep.clone());
                                }
                            }
                        }

                        // If there was a reduction, place this operation back
                        // into the queue to re-evaluate it later until there
                        // are no further reductions.
                        queue.push_back(port.clone());
                    }
                } else {
                    println!("no reduction for {port}");
                }

                if inferences.no_choices(&port)? {
                    dbg!(&inferences);
                    return Err(DiagramErrorCode::MessageTypeInferenceFailure(
                        MessageTypeInferenceFailure {
                            no_valid_choice: vec![port],
                            ambiguous_choice: Default::default(),
                            constraints: inferences.into_constraint_map(),
                        }
                    ));
                }
            }

            if let Some(inferred) = inferences.try_infer_types() {
                println!(" ---------------- DONE INFERRING ----------------------- ");
                return Ok(inferred);
            }

            println!(" >>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> ");
            dbg!(&inferences);
            println!(" --------------------------------------------- ");

            // All constraints are satisfied but there is some ambiguity
            // remaining in the message type choices. We can try to peel away
            // the highest cost choices across all the ports. This is not an
            // algorithmically complete method, but it will ensure that outcomes
            // (whether success or failure) are always consistent, always yielding
            // the same selection of message types across all ports.
            inferences.peel_highest_cost_level(&mut queue, &dependents);

            if queue.is_empty() {
                dbg!(&inferences);
                let mut failure = MessageTypeInferenceFailure::default();
                for (port, evaluation) in &inferences.evaluations {
                    if evaluation.no_choices() {
                        failure.no_valid_choice.push(port.clone());
                    }

                    if let Some(ambiguous_indices) = evaluation.is_ambiguous() {
                        let mut ambiguity = Vec::new();
                        for index in ambiguous_indices {
                            let message_type = registry.messages.registration.get_by_index(index)?.type_info;
                            ambiguity.push(message_type.type_name.into());
                        }

                        failure.ambiguous_choice.push((port.clone(), ambiguity));
                    }
                }

                failure.constraints = inferences.into_constraint_map();
                return Err(DiagramErrorCode::MessageTypeInferenceFailure(failure));
            }
        }
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
    ctx.connect_into(OutputRef::start(), &diagram.start);
    let request_msg_index = registry.messages.registration.get_index::<Request>()?;
    ctx.one_of(OutputRef::start(), &[request_msg_index]);

    // Add constraints for the terminate operation
    let response_msg_index = registry.messages.registration.get_index::<Response>()?;
    ctx.one_of(OperationRef::Terminate(Default::default()), &[response_msg_index]);

    let mut streams = StreamAvailability::default();
    Streams::set_stream_availability(&mut streams);
    for (name, stream_type) in streams.named_streams() {
        let stream_msg_index = registry.messages.registration.get_index_dyn(&stream_type)?;
        ctx.one_of(OperationRef::stream_out(&Arc::from(name)), &[stream_msg_index]);
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

fn as_type_name(
    one_of: &Option<MessageTypeChoices>,
    registry: &DiagramElementRegistry,
) -> Result<Option<SmallVec<[DebugMessageTypeChoice; 8]>>, DiagramErrorCode> {
    let Some(one_of) = one_of else {
        return Ok(None);
    };

    Ok(Some(
        one_of
        .iter()
        .map(|e| Ok::<_, DiagramErrorCode>(DebugMessageTypeChoice {
            name: registry.messages.registration.get_by_index(e.id)?.type_info.type_name,
            cost: e.cost,
        }))
        .collect::<Result<SmallVec<[DebugMessageTypeChoice; 8]>, _>>()?
    ))
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
    pub fn one_of(
        &mut self,
        port: impl Into<PortRef>,
        one_of: &[usize],
    ) {
        let port = self.into_port_ref(port);
        let one_of = one_of
            .iter()
            .copied()
            .map(|id| MessageTypeChoice { id, cost: 0 })
            .collect();

        self
            .inference
            .evaluations
            .entry(port.clone())
            .or_default()
            .one_of = Some(one_of);

        let one_of = as_type_name(&self.inference.evaluation(port.clone()).one_of, self.registry);
        // println!("{port} is one of: {:?}", one_of.unwrap());
    }

    /// Specify that two ports have the exact same type with no conversion.
    pub fn exact_match(
        &mut self,
        port_0: impl Into<PortRef>,
        port_1: impl Into<PortRef>,
    ) {
        let port_0 = self.into_port_ref(port_0);
        let port_1 = self.into_port_ref(port_1);

        self
            .inference
            .constraint_level_mut(port_0.clone(), 0)
            .push(Arc::new(ExactMatch(port_1.clone())));

        self
            .inference
            .constraint_level_mut(port_1, 0)
            .push(Arc::new(ExactMatch(port_0)));
    }

    /// Specify that an output connects into an input.
    pub fn connect_into(
        &mut self,
        output: impl Into<OutputRef>,
        input: impl Into<OperationRef>,
    ) {
        let output = self.into_output_ref(output);
        let input = self.into_operation_ref(input);

        self
            .inference
            .constraint_level_mut(output.clone(), 1)
            .push(Arc::new(ConnectInto(input.clone())));

        self
            .inference
            .constraint_level_mut(input, 1)
            .push(Arc::new(ConnectFrom(output)));
    }

    pub fn try_convert_into(
        &mut self,
        input: impl Into<OperationRef>,
        output: impl Into<OutputRef>,
    ) {
        let input = self.into_operation_ref(input);
        let output = self.into_output_ref(output);

        // We set conversions to a complexity level of 2 because this constraint
        // can blow up since any serializable/deserializable types must be considered.
        self
            .inference
            .constraint_level_mut(input.clone(), 2)
            .push(Arc::new(ConvertInto(output.clone())));

        self
            .inference
            .constraint_level_mut(output, 2)
            .push(Arc::new(ConvertFrom(input)));
    }

    pub fn fork_clone(
        &mut self,
        cloneable_id: &OperationName,
        next: &[NextOperation],
    ) {
        let cloneable_port = self.into_port_ref(cloneable_id);

        let mut outputs = Vec::new();
        for (i, _) in next.iter().enumerate() {
            let output = self.into_port_ref(output_ref(cloneable_id).next_index(i));
            outputs.push(output.clone());

            self.inference
                .constraint_level_mut(output, 0)
                .push(Arc::new(CloneFrom(cloneable_port.clone())));
        }

        self.inference
            .constraint_level_mut(cloneable_port, 0)
            .push(Arc::new(CloneInto(outputs)));
    }

    pub fn result(
        &mut self,
        result: impl Into<OperationRef>,
        ok: impl Into<OutputRef>,
        err: impl Into<OutputRef>,
    ) {
        let result = self.into_operation_ref(result);
        let ok = self.into_output_ref(ok);
        let err = self.into_output_ref(err);

        self
            .inference
            // This constraint complexity is 0 because we can directly map from
            // the Result type to its Ok type
            .constraint_level_mut(ok.clone(), 0)
            .push(Arc::new(OkFrom(result.clone())));

        self
            .inference
            .constraint_level_mut(err.clone(), 0)
            .push(Arc::new(ErrFrom(result.clone())));

        self
            .inference
            // This constraint complexity is 2 because it needs to evaluate
            // (ok, err) combinatorially to identify the Result type
            .constraint_level_mut(result, 2)
            .push(Arc::new(ResultInto{ ok, err }));
    }

    pub fn unzip<U: Into<OperationRef>>(
        &mut self,
        unzippable_name: &OperationName,
        elements: impl Iterator<Item=U>,
    ) {
        let unzippable: OperationRef = unzippable_name.into();
        let unzippable = unzippable.in_namespaces(&self.namespaces);

        let mut element_refs = Vec::new();
        for (i, next) in elements.enumerate() {
            let element_ref = output_ref(unzippable_name).next_index(i);
            let element_ref = self.into_output_ref(element_ref);
            let next = self.into_operation_ref(next);

            self
                .inference
                // The complexity is O(1) because we can directly infer the
                // element type based on the upstream unzippable.
                .constraint_level_mut(element_ref.clone(), 0)
                .push(Arc::new(UnzipFrom { op: unzippable.clone(), element: i }));

            element_refs.push(element_ref.clone());
            self.connect_into(element_ref, next);
        }

        // The complexity of this constraint is O(N^m) where m is the number
        // of elements in the tuple.
        let complexity = element_refs.len();
        self
            .inference
            .constraint_level_mut(unzippable, complexity)
            .push(Arc::new(UnzipInto(element_refs.into())));
    }

    pub fn join(
        &mut self,
        selection: &BufferSelection,
        joined: impl Into<PortRef>,
    ) {
        let joined = self.into_port_ref(joined);
        for (identifier, op) in selection.iter() {
            let port = self.into_port_ref(op);
            self
                .inference
                .constraint_level_mut(port, 2)
                .push(Arc::new(JoinInto {
                    member: identifier.to_owned(),
                    joined: joined.clone(),
                }));
        }

        // TODO(@mxgrey): Consider applying a reverse constraint: inferring the
        // Joined type based on its constituent parts. Unfortunately the Joined
        // type is ambiguous when basing it only on the parts that are being
        // joined into it, even accounting for the layout. That makes the
        // reverse inference both weak and expensive to calculate, so we skip it
        // for now.
    }

    pub fn buffer_access(
        &mut self,
        request: impl Into<PortRef>,
        selection: &BufferSelection,
        next: impl Into<PortRef>,
    ) {
        let request = self.into_port_ref(request);
        let accessor = self.into_port_ref(next);

        for (identifier, op) in selection.iter() {
            let port = self.into_port_ref(op);
            self
                .inference
                .constraint_level_mut(port, 2)
                .push(Arc::new(BufferAccessLayoutMember {
                    accessor: accessor.clone(),
                    member: identifier.to_owned(),
                }));
        }

        self
            .inference
            .constraint_level_mut(request, 0)
            .push(Arc::new(BufferAccessRequestMessage { accessor }));
    }

    pub fn listen(
        &mut self,
        selection: &BufferSelection,
        listener: impl Into<PortRef>,
    ) {
        let listener = self.into_port_ref(listener);
        for (identifier, op) in selection.iter() {
            let port = self.into_port_ref(op);
            self
                .inference
                .constraint_level_mut(port, 2)
                .push(Arc::new(ListenMember {
                    listener: listener.clone(),
                    member: identifier.to_owned(),
                }));
        }
    }

    pub fn split<'s>(
        &mut self,
        split: impl Into<OperationRef>,
        elements: impl Iterator<Item = &'s NextOperation>,
    ) {
        let split = split.into();
        let split = split.in_namespaces(&self.namespaces);

        let mut outputs = Vec::new();
        for element in elements {
            let element_port = self.into_port_ref(element);
            outputs.push(element_port.clone());
            self
                .inference
                .constraint_level_mut(element_port, 0)
                .push(Arc::new(SplitFrom(split.clone())));
        }

        self.inference.constraint_level_mut(split, 1).push(Arc::new(SplitInto(outputs)));
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

pub struct ConstraintContext<'a> {
    inference: &'a Inferences,
    pub registry: &'a DiagramElementRegistry,
}

impl<'a> ConstraintContext<'a> {
    pub fn get_inference_of(
        &self,
        port: impl Into<PortRef>,
    ) -> Result<&Option<MessageTypeChoices>, DiagramErrorCode> {
        let port = port.into();
        let one_of = &self
            .inference
            .evaluations
            .get(&port)
            .ok_or_else(move || DiagramErrorCode::UnknownPort(port))?
            .one_of;

        Ok(one_of)
    }

    pub fn evaluate_connect_into(
        &self,
        port: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for choice in inference {
            self.evaluate_connect_into_impl(choice, &mut result)?;
        }

        Ok(Some(result))
    }

    /// This function gets reused across both might_connect_into and
    ///
    /// might_convert_into so we use a shared implementation for both.
    fn evaluate_connect_into_impl(
        &self,
        choice: &MessageTypeChoice,
        result: &mut MessageTypeChoices,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        // Simply matching the message type is an option
        result.push(MessageTypeChoice {
            id: choice.id,
            cost: 1 + choice.cost,
        });

        let ops = self.operations_of(choice.id)?;

        // Consider any message types that this target type can be cast
        // from. Note: switching "into" to "from" is intentional because we
        // are backtracking
        for msg in ops.from_impls.keys() {
            result.push(MessageTypeChoice {
                id: *msg,
                cost: 2 + choice.cost,
            });
        }

        if let Ok(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() {
            if choice.id == json_index {
                // If the target type is JsonMessage, then we can support any
                // type that is serializable.
                for (id, r) in self.registry.messages.registration.iter().enumerate() {
                    if r.operations.as_ref().is_some_and(|ops| ops.serialize.is_some()) {
                        result.push(MessageTypeChoice {
                            id,
                            cost: 4 + choice.cost,
                        });
                    }
                }
            } else if ops.deserialize.is_some() {
                result.push(MessageTypeChoice {
                    id: json_index,
                    cost: 3 + choice.cost,
                });
            }
        }

        Ok(ops)
    }

    pub fn evaluate_connect_from(
        &self,
        port: &OutputRef,
    ) -> MessageTypeEvaluation {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for choice in inference {
            self.evaluate_connect_from_impl(choice, &mut result)?;
        }

        Ok(Some(result))
    }

    /// This function gets reused across both might_connect_from and
    /// might_convert_from so we use a shared implementation for both.
    fn evaluate_connect_from_impl(
        &self,
        choice: &MessageTypeChoice,
        result: &mut MessageTypeChoices,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        // Simply matching the message type is an option
        result.push(MessageTypeChoice {
            id: choice.id,
            cost: 1 + choice.cost,
        });

        let ops = self.operations_of(choice.id)?;

        // Consider any message types that this source type can be cast
        // into. Note: switching "from" to "into" is intentional because we
        // are backtracking.
        for msg in ops.into_impls.keys() {
            result.push(MessageTypeChoice {
                id: *msg,
                cost: 2 + choice.cost,
            });
        }

        if let Ok(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() {
            if choice.id == json_index {
                // If the source type is JsonMessage, then we can support type
                // that is deserializable.
                for (id, r) in self.registry.messages.registration.iter().enumerate() {
                    if r.operations.as_ref().is_some_and(|ops| ops.deserialize.is_some()) {
                        result.push(MessageTypeChoice {
                            id,
                            cost: 4 + choice.cost,
                        });
                    }
                }
            } else if ops.serialize.is_some() {
                // If the target type is serializable then it can be serialized
                // into a JsonMessage.
                result.push(MessageTypeChoice {
                    id: json_index,
                    cost: 3 + choice.cost,
                });
            }
        }

        Ok(ops)
    }

    pub fn evaluate_convert_into(
        &self,
        port: &OutputRef,
    ) -> MessageTypeEvaluation {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            // Consider any message types that this can normally connect into.
            let ops = self.evaluate_connect_into_impl(message_type_index, &mut result)?;

            // Consider any message types that this source type can attempt to
            // convert into. Note: switching "into" to "from" is intentional
            // because we are backtracking.
            for msg in ops.try_from_impls.keys() {
                result.push(MessageTypeChoice {
                    id: *msg,
                    cost: 5,
                });
            }

            if ops.deserialize.is_some() {
                // If the target is deserializable then we should consider any
                // serializable type since we can attempt to convert any
                // serializable type into any deserializable type.
                for (id, msg) in self.registry.messages.registration.iter().enumerate() {
                    if msg.get_operations()?.serialize.is_some() {
                        result.push(MessageTypeChoice {
                            id,
                            cost: 6,
                        });
                    }
                }
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_convert_from(
        &self,
        port: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            // Consider any message types that this can normally connect from.
            let ops = self.evaluate_connect_from_impl(message_type_index, &mut result)?;

            // Consider any message types that this source type can attempt to
            // convert into. Note: switching "from" to "into" is intentional
            // because we are backtracking.
            for msg in ops.try_into_impls.keys() {
                result.push(MessageTypeChoice {
                    id: *msg,
                    cost: 5,
                });
            }

            if ops.serialize.is_some() {
                // If the source is serializable then we should consider any
                // deserializable type since we can attempt to convert any
                // serializable type into any deserializable type.
                for (id, msg) in self.registry.messages.registration.iter().enumerate() {
                    if msg.get_operations()?.deserialize.is_some() {
                        result.push(MessageTypeChoice {
                            id,
                            cost: 6,
                        });
                    }
                }
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_clone_into(
        &self,
        outputs: &[PortRef],
    ) -> MessageTypeEvaluation {
        let mut one_of: Option<MessageTypeChoices> = None;
        for output in outputs {
            if let Some(inference) = self.get_inference_of(output.clone())?.clone() {
                reduce_choices(&mut one_of, inference, &self.registry);
            }
        }

        if let Some(one_of) = &mut one_of {
            one_of.retain(|choice| {
                let Ok(ops) = self.operations_of(choice.id) else {
                    return false;
                };

                // We need to consider the "into" type as a constraint, but we
                // should prioritize cloning the type that gets fed into the
                // cloning operation, so we bias this far away.
                choice.cost += 10;

                ops.fork_clone.is_some()
            })
        }

        Ok(one_of)
    }

    pub fn evaluate_clone_from(
        &self,
        cloneable: &PortRef,
    ) -> MessageTypeEvaluation {
        dbg!(cloneable);
        let Some(mut one_of) = self.get_inference_of(cloneable.clone())?.clone() else {
            return Ok(None);
        };

        one_of.retain(|choice| {
            let Ok(ops) = self.operations_of(choice.id) else {
                return false;
            };

            ops.fork_clone.is_some()
        });

        dbg!(&one_of);
        Ok(Some(one_of))
    }

    pub fn evaluate_result_into(
        &self,
        ok: &OutputRef,
        err: &OutputRef,
    ) -> MessageTypeEvaluation {
        let Some(ok_inference) = self.get_inference_of(ok.clone())? else {
            return Ok(None);
        };

        let Some(err_inference) = self.get_inference_of(err.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for ok_choice in ok_inference {
            for err_choice in err_inference.iter().copied() {
                let key = [ok_choice.id, err_choice.id];
                if let Some(r) = self.registry.messages.registration.lookup.result.get(&key) {
                    result.push(MessageTypeChoice {
                        id: *r,
                        cost: 0 + u32::min(ok_choice.cost, err_choice.cost),
                    });
                }
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_ok_from(
        &self,
        from_result: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for choice in result_inference {
            let r = &self
                .operations_of(choice.id)?
                .fork_result;

            if let Some(r) = r {
                let [ok, _] = r.output_types;
                result.push(MessageTypeChoice {
                    id: ok,
                    cost: 0 + choice.cost,
                });
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_err_from(
        &self,
        from_result: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for choice in result_inference {
            let r = &self
                .operations_of(choice.id)?
                .fork_result;

            if let Some(r) = r {
                let [_, err] = r.output_types;
                result.push(MessageTypeChoice {
                    id: err,
                    cost: 0 + choice.cost,
                });
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_unzip_into(
        &self,
        outputs: &[OutputRef],
    ) -> MessageTypeEvaluation {
        let mut inferences = Vec::new();
        let mut indexes = Vec::new();
        for output in outputs {
            let Some(inference) = self.get_inference_of(output.clone())? else {
                return Ok(None);
            };

            if inference.is_empty() {
                return Err(DiagramErrorCode::CannotInferType(output.clone().into()));
            }
            inferences.push(inference);
            indexes.push(0usize);
        }

        let mut result = SmallVec::new();
        loop {
            let mut key = Vec::new();
            let mut cost = u32::MAX;
            for (index, element) in indexes.iter().zip(&inferences) {
                let e = element[*index];
                key.push(e.id);
                cost = u32::min(cost, e.cost);
            }

            if let Some(unzip) = self.registry.messages.registration.lookup.unzip.get(&key) {
                result.push(MessageTypeChoice {
                    id: *unzip,
                    cost: 0 + cost,
                });
            }

            // Increment the next index that needs to be adjusted.
            for (index, element) in indexes.iter_mut().zip(&inferences) {
                if *index + 1 < element.len() {
                    // The first index that has not reached its limit should be
                    // incremented.
                    *index += 1;
                    break;
                }

                // The current index has reached the highest value that it can.
                // We should reset it to zero and continue the for-loop to
                // increment the next index.
                *index = 0;
            }

            if indexes.iter().all(|index| *index == 0) {
                // This means we have circled all the way back to all zeroes,
                // so we should break out of the loop.
                break;
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_unzip_from(
        &self,
        input: &OperationRef,
        element: usize,
    ) -> MessageTypeEvaluation {
        let Some(unzip_inference) = self.get_inference_of(input.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for unzip in unzip_inference.iter().copied() {
            if let Some(unzip_impl) = &self.operations_of(unzip.id)?.unzip {
                if let Some(id) = unzip_impl.output_types.get(element).copied() {
                    result.push(MessageTypeChoice { id, cost: 0 + unzip.cost });
                } else if unzip_inference.len() == 1 {
                    let message = self.registry.messages.get_type_info_for(unzip.id)?;
                    error = Some(DiagramErrorCode::InvalidUnzip { message, element });
                }
            } else if unzip_inference.len() == 1 {
                // There is only one possible message left, and it cannot be
                // unzipped. This means there is an error in the diagram.
                return Err(DiagramErrorCode::NotUnzippable(
                    self.registry.messages.get_type_info_for(unzip.id)?
                ))
            }
        }

        if result.is_empty() && let Some(error) = error {
            return Err(error);
        }

        Ok(Some(result))
    }

    pub fn evaluate_buffer_layout_member(
        &self,
        target: &PortRef,
        member: &BufferIdentifier,
        get_layout: fn(&Self, usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode>,
    ) -> MessageTypeEvaluation {
        let Some(target_inference) = self.get_inference_of(target.clone())? else {
            return Ok(None);
        };

        let any_index = self.registry.messages.registration.get_index::<AnyMessageBox>();
        let json_index = self.registry.messages.registration.get_index::<JsonMessage>();

        let eval_hint = |hint: &MessageTypeHint<usize>, base_cost: u32, result: &mut SmallVec<_>| {
            match hint {
                MessageTypeHint::Exact(id) => {
                    result.push(
                        MessageTypeChoice {
                            id: *id,
                            cost: 1 + base_cost,
                        }
                    );
                }
                MessageTypeHint::Fallback(id) => {
                    // TODO(@mxgrey): Consider whether we can have a more efficient
                    // representation, e.g. pass back a function/trait that validates
                    // the properties of a message type choice instead of listing all
                    // known valid options.
                    if let Ok(json_index) = json_index && *id == json_index {
                        // This means any serializable and deserializable message
                        // type can be stored in this buffer.
                        for (id, msg) in self.registry.messages.registration.iter().enumerate() {
                            if let Some(ops) = &msg.operations {
                                if ops.serialize.is_some() && ops.deserialize.is_some() {
                                    result.push(MessageTypeChoice {
                                        id,
                                        // Assign a high cost to the type because
                                        // the real decision about the type should
                                        // be made elsewhere.
                                        cost: 10 + base_cost,
                                    })
                                }
                            }
                        }

                        // In the event of ambiguity for the other choices, let
                        // the most general JsonMessage type win.
                        result.push(
                            MessageTypeChoice {
                                id: json_index,
                                cost: 9 + base_cost,
                            }
                        )
                    } else if let Ok(any_index) = any_index && *id == any_index {
                        // This means literally any message type can be stored in
                        // this buffer.
                        for id in 0..self.registry.messages.registration.len() {
                            result.push(MessageTypeChoice {
                                id,
                                // Assign a very high cost to the type because
                                // the real decision about the type should be
                                // made elsewhere.
                                cost: 15 + base_cost,
                            })
                        }
                    } else {
                        result.push(
                            MessageTypeChoice {
                                id: *id,
                                cost: 3 + base_cost,
                            }
                        );
                    }

                    if let Ok(any_index) = any_index && *id != any_index {
                        // Any fallback type can be channeled into an AnyBuffer,
                        // so we should always include it as an option.
                        result.push(
                            MessageTypeChoice {
                                id: any_index,
                                // Assign a high cost since this is a last resort
                                cost: 10 + base_cost,
                            }
                        );
                    }
                }
            }
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for target_choice in target_inference {
            match get_layout(self, target_choice.id) {
                Ok(layout) => {
                    match layout {
                        BufferMapLayoutHints::Dynamic(dynamic) => {
                            if dynamic.is_compatible(member) {
                                match &dynamic.constraint {
                                    BufferMapLayoutConstraint::Any => {
                                        if let Ok(any_index) = any_index {
                                            result.push(MessageTypeChoice {
                                                id: any_index,
                                                cost: 4 + target_choice.cost,
                                            });
                                        }
                                    }
                                    BufferMapLayoutConstraint::AnyOf(hints) | BufferMapLayoutConstraint::OneOf(hints) => {
                                        for hint in hints {
                                            eval_hint(hint, target_choice.cost, &mut result);
                                        }
                                    }
                                }
                            }
                        }
                        BufferMapLayoutHints::Static(hints) => {
                            if let Some(hint) = hints.get(member) {
                                eval_hint(hint, target_choice.cost, &mut result);
                            }
                        }
                    }
                }
                Err(err) => {
                    error = Some(err);
                }
            }
        }

        if result.is_empty() && let Some(error) = error {
            return Err(error);
        }
        Ok(Some(result))
    }

    pub fn evaluate_buffer_access_request_message(
        &self,
        accessor: &PortRef,
    ) -> MessageTypeEvaluation {
        let Some(accessor_inference) = self.get_inference_of(accessor.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for maybe_accessor in accessor_inference {
            if let Some(buffer_access) = &self.operations_of(maybe_accessor.id)?.buffer_access {
                result.push(MessageTypeChoice {
                    id: buffer_access.request_message,
                    cost: 0 + maybe_accessor.cost,
                });
            } else {
                error = Some(DiagramErrorCode::CannotAccessBuffers(
                    self
                    .registry
                    .messages
                    .get_type_info_for(maybe_accessor.id)?
                ));
            }
        }

        if result.is_empty() && let Some(error) = error {
            return Err(error);
        }
        Ok(Some(result))
    }

    pub fn evaluate_split_from(
        &self,
        split: &OperationRef,
    ) -> MessageTypeEvaluation {
        let Some(split_inference) = self.get_inference_of(split.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for maybe_splittable in split_inference {
            if let Some(split) = &self.operations_of(maybe_splittable.id)?.split {
                result.push(MessageTypeChoice {
                    id: split.output_type,
                    cost: 0 + maybe_splittable.cost,
                });
            } else {
                error = Some(DiagramErrorCode::NotSplittable(
                    self
                    .registry
                    .messages
                    .get_type_info_for(maybe_splittable.id)?
                ));
            }
        }

        if result.is_empty() && let Some(error) = error {
            return Err(error);
        }

        Ok(Some(result))
    }

    pub fn evaluate_split_into(
        &self,
        items: &[PortRef],
    ) -> MessageTypeEvaluation {
        let mut item_one_of: Option<MessageTypeChoices> = None;
        for output in items {
            if let Some(inference) = self.get_inference_of(output.clone())?.clone() {
                reduce_choices(&mut item_one_of, inference, &self.registry);
            }
        }

        let Some(item_one_of) = item_one_of else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for item in item_one_of {
            let Some(split_choices) = self.registry.messages.registration.lookup.split.get(&item.id) else {
                continue;
            };

            for id in split_choices {
                result.push(MessageTypeChoice {
                    id: *id,
                    cost: 0 + item.cost,
                });
            }
        }

        Ok(Some(result))
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

#[derive(Debug, Clone, Copy)]
pub struct MessageTypeChoice {
    pub id: usize,
    pub cost: u32,
}

pub type MessageTypeChoices = SmallVec<[MessageTypeChoice; 8]>;

impl<'a, 'b> Deref for InferenceContext<'a, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

pub type MessageTypeEvaluation = Result<Option<MessageTypeChoices>, DiagramErrorCode>;

pub trait MessageTypeConstraint: std::fmt::Debug + 'static + Send + Sync {
    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation;

    fn dependencies(&self) -> SmallVec<[PortRef; 8]>;
}

pub type ConstraintMap = BTreeMap<usize, Vec<Arc<dyn MessageTypeConstraint>>>;

#[derive(Debug, Default, Clone)]
struct MessageTypeInference {
    one_of: Option<MessageTypeChoices>,
    /// A ranked set of constraints that apply to this inference.
    ///
    /// Constraints are ranked by the computational complexity of their evaluation.
    /// Lower keys are less complex to evaluate. Roughly speaking constraints
    /// should be grouped by O(N^i) where i is the index in this map.
    ///
    /// So constraints in index 0 should evaluate with O(1) complexity. Constraints
    /// in index 1 should evaluate with O(N) complexity. Index 2 should be for
    /// O(N^2) complexity, etc.
    constraints: ConstraintMap,
}

impl MessageTypeInference {
    fn infer(&self) -> Option<usize> {
        let mut choices = self.choices()?;
        if choices.len() == 1 {
            choices.pop()
        } else {
            None
        }
    }

    fn evaluate(
        &self,
        inference: &Inferences,
        registry: &DiagramElementRegistry,
    ) -> Result<Option<MessageTypeChoices>, DiagramErrorCode> {
        let mut one_of = self.one_of.clone();
        println!("initial choices: {:?}", as_type_name(&one_of, registry));
        for level in self.constraints.values() {
            let mut changed = false;
            let ctx = ConstraintContext { inference, registry };

            for constraint in level {
                let reduction = constraint.evaluate(&ctx)?.map(|mut reduction| {
                    // Sort by ID and then by cost for items with the same ID
                    reduction.sort_by(|a, b| {
                        match a.id.cmp(&b.id) {
                            Ordering::Equal => return a.cost.cmp(&b.cost),
                            x => return x,
                        }
                    });
                    // Remove any duplicates. Since items with the same ID are
                    // ordered by ascending cost, we should keep the version
                    // that has the lowest cost.
                    reduction.dedup_by(|a, b| a.id == b.id);
                    reduction
                });

                println!("reducing via {constraint:?}: {:?}", as_type_name(&reduction, registry));
                if let Some(reduction) = reduction {
                    changed |= reduce_choices(&mut one_of, reduction, registry);
                }
            }

            if changed {
                // Stop at the lowest level where a change has occurred.
                return Ok(one_of);
            }
        }

        Ok(one_of)
    }

    fn no_choices(&self) -> bool {
        self.one_of.as_ref().is_some_and(|choices| choices.is_empty())
    }

    fn is_ambiguous(&self) -> Option<SmallVec<[usize; 8]>> {
        let choices = self.choices()?;
        if choices.len() > 1 {
            return Some(choices);
        }

        None
    }

    fn choices(&self) -> Option<SmallVec<[usize; 8]>> {
        if let Some(one_of) = &self.one_of {
            return Some(one_of.iter().map(|choice| choice.id).collect());
        }

        None
    }

    /// Remove all choices that do not overlap with the choices inside intersect.
    fn reduce(&mut self, intersect: MessageTypeChoices, registry: &DiagramElementRegistry) -> bool {
        reduce_choices(&mut self.one_of, intersect, registry)
    }

    /// Identify the highest cost level in this inference that leads to ambiguity.
    /// If there is no ambiguity or if there is only one cost level, then this
    /// will return None.
    fn highest_ambiguous_cost(&mut self) -> Option<u32> {
        if let Some(one_of) = &mut self.one_of {
            one_of.sort_by(|a, b| a.cost.cmp(&b.cost));
            if let (Some(lowest), Some(highest)) = (one_of.first(), one_of.last()) {
                if lowest.cost < highest.cost {
                    return Some(highest.cost);
                }
            }
        }

        None
    }

    fn peel_ambiguous_cost(&mut self, cost: u32) -> bool {
        if let Some(one_of) = &mut self.one_of {
            // We should only peel off this cost if it will not erase all choices.
            if one_of.iter().any(|e| e.cost < cost) {
                one_of.retain(|e| e.cost < cost);
                return true;
            }
        }

        false
    }
}

fn reduce_choices(
    one_of: &mut Option<MessageTypeChoices>,
    intersect: MessageTypeChoices,
    registry: &DiagramElementRegistry,
) -> bool {
    let mut changed = false;
    if let Some(one_of) = one_of.as_mut() {
        // let one_of_ids: Vec<usize> = one_of.iter().map(|e| e.id).collect();
        // let intersect_ids: Vec<usize> = intersect.iter().map(|e| e.id).collect();
        println!(
            "intersecting {:?} n {:?}",
            as_type_name(&Some(one_of.clone()), registry),
            as_type_name(&Some(intersect.clone()), registry),
        );


        let original = one_of.len();
        // Reduce the set of options for this message type based
        // on the constraints.
        one_of.retain(|choice| {
            intersect.iter().any(|e| e.id == choice.id)
        });

        let choices_reduced = original != one_of.len();
        changed |= choices_reduced;

        for choice in one_of {
            for e in &intersect {
                if e.id == choice.id {
                    choice.cost = u32::min(choice.cost, e.cost);
                }
            }
        }
    } else {
        changed = true;
        *one_of = Some(intersect);
    }

    changed
}

/// An input or output port of an operation.
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
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
}

impl Inferences {
    fn evaluation(&mut self, key: impl Into<PortRef>) -> &mut MessageTypeInference {
        let key = key.into();
        self.evaluations.entry(key).or_default()
    }

    fn get_evaluation(&self, key: &PortRef) -> Result<&MessageTypeInference, DiagramErrorCode> {
        self
            .evaluations
            .get(&key)
            .ok_or_else(|| DiagramErrorCode::UnknownPort(key.clone()))
    }

    fn no_choices(&self, key: &PortRef) -> Result<bool, DiagramErrorCode> {
        Ok(self.get_evaluation(key)?.no_choices())
    }

    fn constraint_level_mut(
        &mut self,
        key: impl Into<PortRef>,
        level: usize,
    ) -> &mut Vec<Arc<dyn MessageTypeConstraint>> {
        self.evaluation(key).constraints.entry(level).or_default()
    }

    fn into_constraint_map(self) -> HashMap<PortRef, ConstraintMap> {
        let mut map = HashMap::new();
        for (port, infer) in self.evaluations {
            map.insert(port, infer.constraints);
        }

        map
    }

    fn try_infer_types(&self) -> Option<InferredMessageTypes> {
        let mut inferred = InferredMessageTypes::new();
        for (port, evaluation) in &self.evaluations {
            inferred.insert(port.clone(), evaluation.infer()?);
        }
        Some(inferred)
    }

    /// Identify the highest cost level that's creating ambiguity and then remove
    /// that level across all ports that have ambiguity.
    fn peel_highest_cost_level(
        &mut self,
        queue: &mut VecDeque<PortRef>,
        dependents: &HashMap<PortRef, Vec<PortRef>>,
    ) {
        let highest_cost = self
            .evaluations
            .values_mut()
            .map(|e| e.highest_ambiguous_cost())
            .max()
            .flatten();

        let Some(highest_cost) = highest_cost else {
            return;
        };

        for (port, evaluation) in &mut self.evaluations {
            if evaluation.peel_ambiguous_cost(highest_cost) {
                if let Some(deps) = dependents.get(port) {
                    for dep in deps {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct ExactMatch(PortRef);

impl MessageTypeConstraint for ExactMatch {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
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
struct ConnectInto(OperationRef);

impl MessageTypeConstraint for ConnectInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_connect_into(&self.0)
    }
}

#[derive(Debug)]
struct ConnectFrom(OutputRef);

impl MessageTypeConstraint for ConnectFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_connect_from(&self.0)
    }
}

#[derive(Debug)]
struct ConvertInto(OutputRef);

impl MessageTypeConstraint for ConvertInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_convert_into(&self.0)
    }
}

#[derive(Debug)]
struct ConvertFrom(OperationRef);

impl MessageTypeConstraint for ConvertFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_convert_from(&self.0)
    }
}

#[derive(Debug)]
struct CloneInto(Vec<PortRef>);

impl MessageTypeConstraint for CloneInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        self.0.iter().cloned().collect()
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_clone_into(&self.0)
    }
}

#[derive(Debug)]
struct CloneFrom(PortRef);

impl MessageTypeConstraint for CloneFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_clone_from(&self.0)
    }
}


#[derive(Debug)]
struct ResultInto {
    ok: OutputRef,
    err: OutputRef,
}

impl MessageTypeConstraint for ResultInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.ok.clone().into(), self.err.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_result_into(&self.ok, &self.err)
    }
}

#[derive(Debug)]
struct OkFrom(OperationRef);

impl MessageTypeConstraint for OkFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
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
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
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
struct UnzipInto(SmallVec<[OutputRef; 8]>);

impl MessageTypeConstraint for UnzipInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        self.0.clone().into_iter().map(|output| output.into()).collect()
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_unzip_into(&self.0)
    }
}

#[derive(Debug)]
struct UnzipFrom {
    op: OperationRef,
    element: usize,
}

impl MessageTypeConstraint for UnzipFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.op.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_unzip_from(&self.op, self.element)
    }
}

#[derive(Debug)]
struct JoinInto {
    joined: PortRef,
    member: BufferIdentifier<'static>,
}

impl MessageTypeConstraint for JoinInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.joined.clone()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_buffer_layout_member(
            &self.joined,
            &self.member,
            |inference, msg| {
                match &inference.operations_of(msg)?.join {
                    Some(join) => Ok(&join.layout),
                    None => Err(DiagramErrorCode::NotJoinable(
                        inference.registry.messages.get_type_info_for(msg)?
                    )),
                }
            }
        )
    }
}

#[derive(Debug)]
struct BufferAccessLayoutMember {
    accessor: PortRef,
    member: BufferIdentifier<'static>,
}

impl MessageTypeConstraint for BufferAccessLayoutMember {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.accessor.clone()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_buffer_layout_member(
            &self.accessor,
            &self.member,
            |inference, msg| {
                match &inference.operations_of(msg)?.buffer_access {
                    Some(access) => Ok(&access.layout),
                    None => Err(DiagramErrorCode::CannotAccessBuffers(
                        inference.registry.messages.get_type_info_for(msg)?
                    ))
                }
            }
        )
    }
}

#[derive(Debug)]
struct BufferAccessRequestMessage {
    accessor: PortRef,
}

impl MessageTypeConstraint for BufferAccessRequestMessage {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.accessor.clone()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_buffer_access_request_message(&self.accessor)
    }
}

#[derive(Debug)]
struct ListenMember {
    listener: PortRef,
    member: BufferIdentifier<'static>,
}

impl MessageTypeConstraint for ListenMember {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.listener.clone()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_buffer_layout_member(
            &self.listener,
            &self.member,
            |inference, msg| {
                match &inference.operations_of(msg)?.listen {
                    Some(listen) => Ok(&listen.layout),
                    None => Err(DiagramErrorCode::CannotListen(
                        inference.registry.messages.get_type_info_for(msg)?
                    ))
                }
            }
        )
    }
}

#[derive(Debug)]
struct SplitFrom(OperationRef);

impl MessageTypeConstraint for SplitFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        context.evaluate_split_from(&self.0)
    }
}

#[derive(Debug)]
struct SplitInto(Vec<PortRef>);

impl MessageTypeConstraint for SplitInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        self.0.iter().cloned().collect()
    }

    fn evaluate(
        &self,
        ctx: &ConstraintContext,
    ) -> MessageTypeEvaluation {
        ctx.evaluate_split_into(&self.0)
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
        // dbg!(inference);

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
