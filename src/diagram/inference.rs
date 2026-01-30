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
    JsonMessage,
};

pub struct InferenceContext<'a, 'b> {
    inference: &'b mut Inference,
    diagram_context: DiagramContext<'a>,
    pub registry: &'a DiagramElementRegistry,
}

impl<'a, 'b> InferenceContext<'a, 'b> {
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
        output: OutputRef,
        input: OperationRef,
    ) {
        let output = output.in_namespaces(&self.namespaces);
        let input = input.in_namespaces(&self.namespaces);

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
        input: OperationRef,
        output: OutputRef,
    ) {
        let input = input.in_namespaces(&self.namespaces);
        let output = output.in_namespaces(&self.namespaces);

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
        result: OperationRef,
        ok: OutputRef,
        err: OutputRef,
    ) {
        let result = result.in_namespaces(&self.namespaces);
        let ok = ok.in_namespaces(&self.namespaces);
        let err = err.in_namespaces(&self.namespaces);

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

    pub fn get_inference_of(
        &self,
        port: impl Into<PortRef>,
    ) -> Result<&Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let port = port.into();
        self
            .inference
            .evaluations
            .get(&port)
            .ok_or_else(move || DiagramErrorCode::UnknownPort(port))
            .map(|e| &e.one_of)
    }

    pub fn might_connect_into(
        &self,
        port: &OperationRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            self.might_connect_into_impl(*message_type_index, &mut result)?;
        }

        Ok(Some(result))
    }

    /// This function gets reused across both might_connect_into and
    /// might_convert_into so we use a shared implementation for both.
    fn might_connect_into_impl(
        &self,
        message_type_index: usize,
        result: &mut SmallVec<[usize; 8]>,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        // Simply matching the message type is an option
        result.push(message_type_index);

        let ops = self.operations_of(message_type_index)?;

        // Consider any message types that this target type can be cast
        // from. Note: switching "into" to "from" is intentional because we
        // are backtracking
        for msg in ops.from_impls.keys() {
            result.push(*msg);
        }

        if ops.deserialize_impl.is_some() {
            // If the target type is deserializable then it can be created
            // from a JsonSchema.
            if let Some(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() {
                result.push(json_index);
            }
        }

        Ok(ops)
    }

    pub fn might_connect_from(
        &self,
        port: &OutputRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            self.might_connect_from_impl(*message_type_index, &mut result)?;
        }

        Ok(Some(result))
    }

    /// This function gets reused across both might_connect_from and
    /// might_convert_from so we use a shared implementation for both.
    fn might_connect_from_impl(
        &self,
        message_type_index: usize,
        result: &mut SmallVec<[usize; 8]>,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        // Simply matching the message type is an option
        result.push(message_type_index);

        let ops = self.operations_of(message_type_index)?;

        // Consider any message types that this source type can be cast
        // into. Note: switching "from" to "into" is intentional because we
        // are backtracking.
        for msg in ops.into_impls.keys() {
            result.push(*msg);
        }

        if ops.serialize_impl.is_some() {
            // If the target type is serializable then it can be serialized
            // into a JsonMessage.
            if let Some(json_index) = self.registry.messages.registration.get_index::<JsonMessage>() {
                result.push(json_index);
            }
        }

        Ok(ops)
    }

    pub fn might_convert_into(
        &self,
        port: &OutputRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            // Consider any message types that this can normally connect into.
            let ops = self.might_connect_into_impl(*message_type_index, &mut result)?;

            // Consider any message types that this source type can attempt to
            // convert into. Note: switching "into" to "from" is intentional
            // because we are backtracking.
            for msg in ops.try_from_impls.keys() {
                result.push(*msg);
            }

            if ops.deserialize_impl.is_some() {
                // If the target is deserializable then we should consider any
                // serializable type since we can attempt to convert any
                // serializable type into any deserializable type.
                for (i, msg) in self.registry.messages.registration.iter().enumerate() {
                    if msg.get_operations()?.serialize_impl.is_some() {
                        result.push(i);
                    }
                }
            }
        }

        Ok(Some(result))
    }

    pub fn might_convert_from(
        &self,
        port: &OperationRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in inference {
            // Consider any message types that this can normally connect from.
            let ops = self.might_connect_from_impl(*message_type_index, &mut result)?;

            // Consider any message types that this source type can attempt to
            // convert into. Note: switching "from" to "into" is intentional
            // because we are backtracking.
            for msg in ops.try_into_impls.keys() {
                result.push(*msg);
            }

            if ops.serialize_impl.is_some() {
                // If the source is serializable then we should consider any
                // deserializable type since we can attempt to convert any
                // serializable type into any deserializable type.
                for (i, msg) in self.registry.messages.registration.iter().enumerate() {
                    if msg.get_operations()?.deserialize_impl.is_some() {
                        result.push(i);
                    }
                }
            }
        }

        Ok(Some(result))
    }

    pub fn might_result_into(
        &self,
        ok: &OutputRef,
        err: &OutputRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(ok_inference) = self.get_inference_of(ok.clone())? else {
            return Ok(None);
        };

        let Some(err_inference) = self.get_inference_of(err.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for ok_index in ok_inference {
            for err_index in err_inference {
                let key = [*ok_index, *err_index];
                if let Some(r) = self.registry.messages.registration.lookup.result.get(&key) {
                    result.push(*r);
                }
            }
        }

        Ok(Some(result))
    }

    pub fn might_ok_from(
        &self,
        from_result: &OperationRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in result_inference {
            let r = &self
                .operations_of(*message_type_index)?
                .fork_result;

            if let Some(r) = r {
                let [ok, _] = r.output_types;
                result.push(ok);
            }
        }

        Ok(Some(result))
    }

    pub fn might_err_from(
        &self,
        from_result: &OperationRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(result_inference) = self.get_inference_of(from_result.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for message_type_index in result_inference {
            let r = &self
                .operations_of(*message_type_index)?
                .fork_result;

            if let Some(r) = r {
                let [_, err] = r.output_types;
                result.push(err);
            }
        }

        Ok(Some(result))
    }

    pub fn might_unzip_into(
        &self,
        outputs: &[OutputRef],
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
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
                result.push(*unzip);
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

    pub fn might_unzip_from(
        &self,
        input: &OperationRef,
        element: usize,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(unzip_inference) = self.get_inference_of(input.clone())? else {
            return Ok(None);
        };

        let mut result = SmallVec::new();
        for unzip in unzip_inference {
            if let Some(unzip) = &self.operations_of(*unzip)?.unzip_impl {
                result.push(unzip.output_types[element]);
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

impl<'a, 'b> Deref for InferenceContext<'a, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

pub trait MessageTypeConstraint: std::fmt::Debug {
    fn evaluate(
        &self,
        context: &InferenceContext,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode>;

    fn dependencies(&self) -> SmallVec<[PortRef; 8]>;
}

#[derive(Debug, Default, Clone)]
struct MessageTypeConstraints {
    exact: SmallVec<[PortRef; 8]>,
    into: SmallVec<[OperationRef; 8]>,
    try_into: Option<OperationRef>,
    from: SmallVec<[OutputRef; 8]>,
    try_from: SmallVec<[OutputRef; 8]>,
    result_into: Option<[OperationRef; 2]>,
    unzip_into: Option<SmallVec<[OperationRef; 8]>>,
}

#[derive(Debug, Default, Clone)]
struct MessageTypeInference {
    one_of: Option<SmallVec<[usize; 8]>>,
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

impl From<OperationRef> for PortRef {
    fn from(value: OperationRef) -> Self {
        Self::Input(value)
    }
}

impl From<OutputRef> for PortRef {
    fn from(value: OutputRef) -> Self {
        Self::Output(value)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
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
        context: &InferenceContext,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_connect_into(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_connect_from(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_convert_into(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_convert_from(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_result_into(&self.ok, &self.err)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_ok_from(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_err_from(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_unzip_into(&self.0)
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
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_unzip_from(&self.op, self.element)
    }
}
