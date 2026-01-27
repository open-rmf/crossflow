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

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

use crate::{OperationRef, OutputRef, DiagramElementRegistry, Diagram, DiagramError};

#[derive(Debug, Default, Clone)]
pub struct MessageTypeConstraints {
    pub into: SmallVec<[OperationRef; 8]>,
    pub try_into: Option<OperationRef>,
    pub from: SmallVec<[OutputRef; 8]>,
    pub try_from: SmallVec<[OutputRef; 8]>,
    pub result: Option<[OperationRef; 2]>,
}

#[derive(Debug, Default, Clone)]
pub struct MessageTypeEvaluation {
    pub one_of: Option<SmallVec<[usize; 8]>>,
    pub constraints: MessageTypeConstraints,
}

impl MessageTypeEvaluation {
    pub fn exact(exact: usize) -> Self {
        Self {
            one_of: Some(smallvec![exact]),
            constraints: Default::default(),
        }
    }
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
