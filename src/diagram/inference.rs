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
};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

use crate::{OperationRef, OutputRef, DiagramElementRegistry, Diagram, DiagramErrorCode, DiagramContext, MessageOperation};

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
        let port_0 = port_0.into();
        let port_1 = port_1.into();

        self
            .inference
            .constraint_level_mut(port_0.clone(), 0)
            .push(Box::new(ExactMatch(port_1.clone())));

        self
            .inference
            .constraint_level_mut(port_1, 0)
            .push(Box::new(ExactMatch(port_0)));
    }

    /// Specify that an output connects into an input.
    pub fn connect_into(
        &mut self,
        output: OutputRef,
        input: OperationRef,
    ) {
        self
            .inference
            .constraint_level_mut(output.clone(), 1)
            .push(Box::new(ConnectInto(input.clone())));

        self
            .inference
            .constraint_level_mut(input, 1)
            .push(Box::new(ConnectFrom(output)));
    }

    pub fn try_convert_into(
        &mut self,
        input: OperationRef,
        output: OutputRef,
    ) {
        self
            .inference
            .constraint_level_mut(input.clone(), 1)
            .push(Box::new(ConvertInto(output.clone())));

        self
            .inference
            .constraint_level_mut(output, 1)
            .push(Box::new(ConvertFrom(input)));
    }

    pub fn result(
        from: OperationRef,
        ok: OutputRef,
        err: OutputRef,
    ) ->
    TODO: Namespaces need to be inserted by these methods that build constraints

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
            let from = self
                .operations_of(*message_type_index)
                // Note: switching "into" to "from" is intentional because we
                // are backtracking
                .from_impls
                .keys();

            for msg in from {
                result.push(*msg);
            }
        }

        Ok(Some(result))
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
            let into = self
                .operations_of(*message_type_index)?
                // Note: switching "from" to "into" is intentional because we
                // are backtracking
                .into_impls
                .keys();

            for msg in into {
                result.push(*msg);
            }
        }

        Ok(Some(result))
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
            let from = self
                .operations_of(*message_type_index)?
                // Note: switching "into" to "from" is intentional because we
                // are backtracking
                .try_from_impls
                .keys();

            for msg in from {
                result.push(*msg);
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
            let into = self
                .operations_of(*message_type_index)?
                // Note: switching "from" to "into" is intentional because we
                // are backtracking
                .try_into_impls
                .keys();

            for msg in into {
                result.push(*msg);
            }
        }

        Ok(Some(result))
    }

    pub fn might_result_into(
        &self,
        ok: &OutputRef,
        err: &OutputRef,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        let Some(ok_inference) = self.get_inference_of(port.clone())? else {
            return Ok(None);
        };

        let Some(err_inference) = self.get_inference_of(port.clone())? else {
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

    pub fn operations_of(
        &self,
        message_type_index: usize,
    ) -> Result<&MessageOperation, DiagramErrorCode> {
        let operations = &self
            .registry
            .messages
            .registration
            .get_by_index(message_type_index)
            .ok_or_else(|| DiagramErrorCode::UnknownMessageTypeIndex {
                index: message_type_index,
                limit: self.registry.messages.registration.len(),
            })?
            .operations;

        Ok(operations)
    }
}

impl<'a, 'b> Deref for InferenceContext<'a, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

pub trait MessageTypeConstraint {
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
    constraints: BTreeMap<usize, Vec<Box<dyn MessageTypeConstraint>>>,
}

/// An input or output port of an operation.
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum PortRef {
    Input(OperationRef),
    Output(OutputRef),
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
    ) -> &mut Vec<Box<dyn MessageTypeConstraint>> {
        self.evaluation(key).constraints.entry(level).or_default()
    }
}

struct ExactMatch(PortRef);

impl MessageTypeConstraint for ExactMatch {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone()]
    }

    fn evaluate(
        &self,
        context: &InferenceContext,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.get_inference_of(&self.0).cloned()
    }
}

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

struct ConvertInto(OutputRef);

impl MessageTypeConstraint for ConvertInto {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone.into()]
    }

    fn evaluate(
        &self,
        context: &InferenceContext,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_convert_into(port)
    }
}

struct ConvertFrom(OperationRef);

impl MessageTypeConstraint for ConvertFrom {
    fn dependencies(&self) -> SmallVec<[PortRef; 8]> {
        smallvec![self.0.clone().into()]
    }

    fn evaluate(
        &self,
        context: &InferenceContext,
    ) -> Result<Option<SmallVec<[usize; 8]>>, DiagramErrorCode> {
        context.might_convert_from(port)
    }
}

struct ResultOf {
    ok: OutputRef,
    err: OutputRef,
}

impl MessageTypeConstraint for ResultOf {
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
