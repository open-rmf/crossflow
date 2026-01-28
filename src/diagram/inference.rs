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
    collections::HashMap,
    ops::Deref,
};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

use crate::{OperationRef, OutputRef, DiagramElementRegistry, Diagram, DiagramError, DiagramContext};

pub struct InferenceContext<'a, 'b> {
    inference: &'b mut Inference,
    diagram_context: DiagramContext<'a>,
}

impl<'a, 'b> InferenceContext<'a, 'b> {

}

impl<'a, 'b> Deref for InferenceContext<'a, 'b> {
    type Target = DiagramContext<'a>;
    fn deref(&self) -> &Self::Target {
        &self.diagram_context
    }
}

#[derive(Debug, Default, Clone)]
struct MessageTypeConstraints {
    exact: SmallVec<[TypeRef; 8]>,
    into: SmallVec<[OperationRef; 8]>,
    try_into: Option<OperationRef>,
    from: SmallVec<[OutputRef; 8]>,
    try_from: SmallVec<[OutputRef; 8]>,
    result_into: Option<[OperationRef; 2]>,
    unzip_into: Option<SmallVec<[OperationRef; 8]>>,
}

#[derive(Debug, Default, Clone)]
struct MessageTypeEvaluation {
    one_of: Option<SmallVec<[usize; 8]>>,
    constraints: MessageTypeConstraints,
}

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum TypeRef {
    Input(OperationRef),
    Output(OutputRef),
}

struct Inference {
    evaluations: HashMap<TypeRef, MessageTypeEvaluation>,
}

fn infer_types(
    registry: &DiagramElementRegistry,
    diagram: &Diagram,
) -> Result<HashMap<TypeRef, usize>, DiagramError> {

}
