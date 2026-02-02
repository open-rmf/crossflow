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
    collections::{HashMap, BTreeMap},
    ops::Deref,
    sync::Arc,
};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

use crate::{
    OperationRef, OutputRef, DiagramElementRegistry, DiagramErrorCode, DiagramContext, MessageOperations,
    JsonMessage, BufferIdentifier, BufferMapLayoutHints, BufferMapLayoutConstraint, AnyMessageBox,
    MessageTypeHint, BufferSelection, NamedOutputRef,
};

pub struct InferenceContext<'a, 'b> {
    inference: &'b mut Inference,
    diagram_context: DiagramContext<'a>,
    pub registry: &'a DiagramElementRegistry,
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
            .entry(port)
            .or_default()
            .one_of = Some(one_of);
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

    pub fn unzip(
        &mut self,
        unzippable: OperationRef,
        elements: Vec<OutputRef>,
    ) {
        let unzippable = unzippable.in_namespaces(&self.namespaces);
        let elements: Vec<OutputRef> = elements.into_iter().map(|e| e.in_namespaces(&self.namespaces)).collect();

        for (i, element) in elements.iter().enumerate() {
            self
                .inference
                // The complexity is O(1) because we can directly infer the
                // element type based on the upstream unzippable.
                .constraint_level_mut(element.clone(), 0)
                .push(Arc::new(UnzipFrom { op: unzippable.clone(), element: i }));
        }

        // The complexity of this constraint is O(N^m) where m is the number
        // of elements in the tuple.
        let complexity = elements.len();
        self
            .inference
            .constraint_level_mut(unzippable, complexity)
            .push(Arc::new(UnzipInto(elements.into())));
    }

    pub fn join(
        &mut self,
        joined: impl Into<PortRef>,
        selection: &BufferSelection,
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
        accessor: impl Into<PortRef>,
        request: impl Into<OutputRef>,
        selection: &BufferSelection,
    ) {
        let accessor = self.into_port_ref(accessor);
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

        let request = self.into_output_ref(request);
        self
            .inference
            .constraint_level_mut(request, 0)
            .push(Arc::new(BufferAccessRequestMessage { accessor }));
    }

    pub fn listen(
        &mut self,
        listener: impl Into<PortRef>,
        selection: &BufferSelection,
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

    pub fn get_inference_of(
        &self,
        port: impl Into<PortRef>,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let port = port.into();
        let one_of = &self
            .inference
            .evaluations
            .get(&port)
            .ok_or_else(move || DiagramErrorCode::UnknownPort(port))?
            .one_of;

        let Some(one_of) = one_of else {
            return Ok(None);
        };

        Ok(Some(one_of.iter().map(|c| c.id).collect()))
    }

    pub fn evaluate_connect_into(
        &self,
        port: &OperationRef,
    ) -> MessageTypeConstraintEvaluation {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            self.evaluate_connect_into_impl(message_type_index, &mut result)?;
        }

        Ok(Some(result))
    }

    /// This function gets reused across both might_connect_into and
    ///
    /// might_convert_into so we use a shared implementation for both.
    fn evaluate_connect_into_impl(
        &self,
        message_type_index: usize,
        result: &mut SmallVec<[MessageTypeChoice; 8]>,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        // Simply matching the message type is an option
        result.push(MessageTypeChoice {
            id: message_type_index,
            cost: 0,
        });

        let ops = self.operations_of(message_type_index)?;

        // Consider any message types that this target type can be cast
        // from. Note: switching "into" to "from" is intentional because we
        // are backtracking
        for msg in ops.from_impls.keys() {
            result.push(MessageTypeChoice {
                id: *msg,
                cost: 1,
            });
        }

        if ops.deserialize.is_some() {
            // If the target type is deserializable then it can be created
            // from a JsonSchema.
            if let Some(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() {
                result.push(MessageTypeChoice {
                    id: json_index,
                    cost: 2,
                });
            }
        }

        Ok(ops)
    }

    pub fn evaluate_connect_from(
        &self,
        port: &OutputRef,
    ) -> MessageTypeConstraintEvaluation {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            self.evaluate_connect_from_impl(message_type_index, &mut result)?;
        }

        Ok(Some(result))
    }

    /// This function gets reused across both might_connect_from and
    /// might_convert_from so we use a shared implementation for both.
    fn evaluate_connect_from_impl(
        &self,
        message_type_index: usize,
        result: &mut SmallVec<[MessageTypeChoice; 8]>,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        // Simply matching the message type is an option
        result.push(MessageTypeChoice {
            id: message_type_index,
            cost: 0,
        });

        let ops = self.operations_of(message_type_index)?;

        // Consider any message types that this source type can be cast
        // into. Note: switching "from" to "into" is intentional because we
        // are backtracking.
        for msg in ops.into_impls.keys() {
            result.push(MessageTypeChoice {
                id: *msg,
                cost: 1,
            });
        }

        if ops.serialize.is_some() {
            // If the target type is serializable then it can be serialized
            // into a JsonMessage.
            if let Some(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() {
                result.push(MessageTypeChoice {
                    id: json_index,
                    cost: 2,
                });
            }
        }

        Ok(ops)
    }

    pub fn evaluate_convert_into(
        &self,
        port: &OutputRef,
    ) -> MessageTypeConstraintEvaluation {
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
                    cost: 3,
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
                            cost: 4,
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
    ) -> MessageTypeConstraintEvaluation {
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
                    cost: 3,
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
                            cost: 4,
                        });
                    }
                }
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_result_into(
        &self,
        ok: &OutputRef,
        err: &OutputRef,
    ) -> MessageTypeConstraintEvaluation {
        let Some(ok_inference) = self.get_inference_of(ok.clone())? else {
            return Ok(None);
        };

        let Some(err_inference) = self.get_inference_of(err.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for ok_index in ok_inference {
            for err_index in err_inference.iter().copied() {
                let key = [ok_index, err_index];
                if let Some(r) = self.registry.messages.registration.lookup.result.get(&key) {
                    result.push(MessageTypeChoice {
                        id: *r,
                        cost: 0,
                    });
                }
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_ok_from(
        &self,
        from_result: &OperationRef,
    ) -> MessageTypeConstraintEvaluation {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in result_inference {
            let r = &self
                .operations_of(message_type_index)?
                .fork_result;

            if let Some(r) = r {
                let [ok, _] = r.output_types;
                result.push(MessageTypeChoice {
                    id: ok,
                    cost: 0,
                });
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_err_from(
        &self,
        from_result: &OperationRef,
    ) -> MessageTypeConstraintEvaluation {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in result_inference {
            let r = &self
                .operations_of(message_type_index)?
                .fork_result;

            if let Some(r) = r {
                let [_, err] = r.output_types;
                result.push(MessageTypeChoice {
                    id: err,
                    cost: 0,
                });
            }
        }

        Ok(Some(result))
    }

    pub fn evaluate_unzip_into(
        &self,
        outputs: &[OutputRef],
    ) -> MessageTypeConstraintEvaluation {
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
            for (index, element) in indexes.iter().zip(&inferences) {
                key.push(element[*index]);
            }

            if let Some(unzip) = self.registry.messages.registration.lookup.unzip.get(&key) {
                result.push(MessageTypeChoice {
                    id: *unzip,
                    cost: 0,
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
    ) -> MessageTypeConstraintEvaluation {
        let Some(unzip_inference) = self.get_inference_of(input.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for unzip in unzip_inference.iter().copied() {
            if let Some(unzip_impl) = &self.operations_of(unzip)?.unzip {
                if let Some(id) = unzip_impl.output_types.get(element).copied() {
                    result.push(MessageTypeChoice { id, cost: 0 });
                } else if unzip_inference.len() == 1 {
                    let message = self.registry.messages.get_type_info_for(unzip)?;
                    error = Some(DiagramErrorCode::InvalidUnzip { message, element });
                }
            } else if unzip_inference.len() == 1 {
                // There is only one possible message left, and it cannot be
                // unzipped. This means there is an error in the diagram.
                return Err(DiagramErrorCode::NotUnzippable(
                    self.registry.messages.get_type_info_for(unzip)?
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
    ) -> MessageTypeConstraintEvaluation {
        let Some(target_inference) = self.get_inference_of(target.clone())? else {
            return Ok(None);
        };

        let eval_hint = |hint: &MessageTypeHint<usize>| {
            match hint {
                MessageTypeHint::Exact(id) => {
                    MessageTypeChoice {
                        id: *id,
                        cost: 1,
                    }
                }
                MessageTypeHint::Fallback(id) => {
                    MessageTypeChoice {
                        id: *id,
                        cost: 3,
                    }
                }
            }
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for target_msg_index in target_inference {
            // if let Some(join) = &self.operations_of(*target_msg_index)?.join_impl {
            match get_layout(self, target_msg_index) {
                Ok(layout) => {
                    match layout {
                        BufferMapLayoutHints::Dynamic(dynamic) => {
                            if dynamic.is_compatible(member) {
                                match &dynamic.constraint {
                                    BufferMapLayoutConstraint::Any => {
                                        let any_index = self.registry.messages.registration.get_index::<AnyMessageBox>();
                                        if let Some(any_index) = any_index {
                                            result.push(MessageTypeChoice {
                                                id: any_index,
                                                cost: 4,
                                            });
                                        }
                                    }
                                    BufferMapLayoutConstraint::AnyOf(hints) | BufferMapLayoutConstraint::OneOf(hints) => {
                                        for hint in hints {
                                            result.push(eval_hint(hint));
                                        }
                                    }
                                }
                            }
                        }
                        BufferMapLayoutHints::Static(hints) => {
                            if let Some(hint) = hints.get(member) {
                                result.push(eval_hint(hint));
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
    ) -> MessageTypeConstraintEvaluation {
        let Some(accessor_inference) = self.get_inference_of(accessor.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for maybe_accessor in accessor_inference {
            if let Some(buffer_access) = &self.operations_of(maybe_accessor)?.buffer_access {
                result.push(MessageTypeChoice {
                    id: buffer_access.request_message,
                    cost: 0,
                });
            } else {
                error = Some(DiagramErrorCode::CannotAccessBuffers(
                    self
                    .registry
                    .messages
                    .get_type_info_for(maybe_accessor)?
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
    ) -> MessageTypeConstraintEvaluation {
        let Some(split_inference) = self.get_inference_of(split.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        let mut error = None;
        for maybe_splittable in split_inference {
            if let Some(split) = &self.operations_of(maybe_splittable)?.split {
                result.push(MessageTypeChoice {
                    id: split.output_type,
                    cost: 0,
                });
            } else {
                error = Some(DiagramErrorCode::NotSplittable(
                    self
                    .registry
                    .messages
                    .get_type_info_for(maybe_splittable)?
                ));
            }
        }

        if result.is_empty() && let Some(error) = error {
            return Err(error);
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

#[derive(Debug, Clone, Copy)]
pub struct MessageTypeChoice {
    pub id: usize,
    pub cost: u32,
}

impl<'a, 'b> Deref for InferenceContext<'a, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

pub type MessageTypeConstraintEvaluation = Result<Option<SmallVec<[MessageTypeChoice; 8]>>, DiagramErrorCode>;

pub trait MessageTypeConstraint: std::fmt::Debug {
    fn evaluate(
        &self,
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation;

    fn dependencies(&self) -> SmallVec<[PortRef; 8]>;
}

#[derive(Debug, Default, Clone)]
struct MessageTypeInference {
    one_of: Option<SmallVec<[MessageTypeChoice; 8]>>,
    /// A ranked set of constraints that apply to this inference.
    ///
    /// Constraints are ranked by the computational complexity of their evaluation.
    /// Lower keys are less complex to evaluate. Roughly speaking constraints
    /// should be grouped by O(N^i) where i is the index in this map.
    ///
    /// So constraints in index 0 should evaluate with O(1) complexity. Constraints
    /// in index 1 should evaluate with O(N) complexity. Index 2 should be for
    /// O(N^2) complexity, etc.
    constraints: BTreeMap<usize, Vec<Arc<dyn MessageTypeConstraint>>>,
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

struct Inference {
    evaluations: HashMap<PortRef, MessageTypeInference>,
}

impl Inference {
    fn evaluation(&mut self, key: impl Into<PortRef>) -> &mut MessageTypeInference {
        let key = key.into();
        self.evaluations.entry(key).or_default()
    }

    fn constraint_level_mut(
        &mut self,
        key: impl Into<PortRef>,
        level: usize,
    ) -> &mut Vec<Arc<dyn MessageTypeConstraint>> {
        self.evaluation(key).constraints.entry(level).or_default()
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
        context
        .get_inference_of(self.0.clone())
        .map(|r| r.as_ref().map(|inferences|
            inferences
            .iter()
            .map(|id| MessageTypeChoice {
                id: *id,
                cost: 0,
            })
            .collect()
        ))
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
        context.evaluate_convert_from(&self.0)
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
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
        context: &InferenceContext,
    ) -> MessageTypeConstraintEvaluation {
        context.evaluate_split_from(&self.0)
    }
}
