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

use std::{
    borrow::{Borrow, Cow},
    ops::Deref,
    sync::Arc,
};
use smallvec::{smallvec, SmallVec};

use serde::{Serialize, Deserialize};
use schemars::{json_schema, JsonSchema};

use crate::{NamespaceList, OperationName, NameOrIndex};

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum OutputRef {
    Named(NamedOutputRef),
    Start(NamespaceList),
}

impl OutputRef {
    pub fn in_namespaces(self, parent_namespaces: &[Arc<str>]) -> Self {
        match self {
            Self::Named(named) => Self::Named(named.in_namespaces(parent_namespaces)),
            Self::Start(namespaces) => Self::Start(namespaces.with_parent_namespaces(parent_namespaces)),
        }
    }

    pub fn start() -> Self {
        Self::Start(Default::default())
    }
}

impl From<NamedOutputRef> for OutputRef {
    fn from(value: NamedOutputRef) -> Self {
        Self::Named(value)
    }
}

impl std::fmt::Display for OutputRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputRef::Named(named) => {
                named.fmt(f)
            }
            OutputRef::Start(namespaces) => {
                write!(f, "{namespaces}(start)")
            }
        }
    }
}

pub fn output_ref(operation: &OperationName) -> NamedOutputBuilder {
    NamedOutputBuilder { operation: Arc::clone(operation) }
}

pub struct NamedOutputBuilder {
    operation: OperationName,
}

impl NamedOutputBuilder {
    pub fn next(self) -> NamedOutputRef {
        self.key(["next"])
    }

    pub fn stream_out(self, stream: &dyn Borrow<str>) -> NamedOutputRef {
        self.key(OutputKey(smallvec!["stream_out".into(), stream.borrow().into()]))
    }

    pub fn ok(self) -> NamedOutputRef {
        self.key(["ok"])
    }

    pub fn err(self) -> NamedOutputRef {
        self.key(["err"])
    }

    pub fn next_index(self, index: usize) -> NamedOutputRef {
        self.key(OutputKey(smallvec!["next".into(), NameOrIndex::Index(index)]))
    }

    pub fn sequential(self, index: usize) -> NamedOutputRef {
        self.key(OutputKey(smallvec!["sequential".into(), NameOrIndex::Index(index)]))
    }

    pub fn keyed(self, key: &OperationName) -> NamedOutputRef {
        self.key(OutputKey(smallvec!["keyed".into(), NameOrIndex::Name(Arc::clone(key))]))
    }

    pub fn remaining(self) -> NamedOutputRef {
        self.key(OutputKey(smallvec!["remaining".into()]))
    }

    pub fn section_output(self, output: &dyn Borrow<str>) -> NamedOutputRef {
        self.key(OutputKey(smallvec!["connect".into(), output.borrow().into()]))
    }

    pub fn key(self, key: impl Into<OutputKey>) -> NamedOutputRef {
        NamedOutputRef {
            namespaces: Default::default(),
            operation: self.operation,
            key: key.into(),
        }
    }
}

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct NamedOutputRef {
    /// A list of names that uniquely identify the scope of the output's operation
    pub namespaces: NamespaceList,
    /// The name of the output's operation within the scope of the namespaces
    pub operation: OperationName,
    /// The unique key of the output within its operation
    pub key: OutputKey,
}

impl NamedOutputRef {
    pub fn in_namespaces(mut self, parent_namespaces: &[Arc<str>]) -> Self {
        self.namespaces.apply_parent_namespaces(parent_namespaces);
        self
    }
}

/// A key that uniquely identifies a specific output belonging to an operation.
/// For example nodes may have OutputKeys such as
/// - ["next"]
/// - ["stream_out", "log"]
/// - ["stream_out", "canceller"]
///
/// Other operations may have different keys, such as fork_clone:
/// - ["next", 0]
/// - ["next", 1]
/// - ["next", 2]
///
/// or fork_result:
/// - ["ok"]
/// - ["err"]
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct OutputKey(pub SmallVec<[NameOrIndex; 4]>);

impl Deref for OutputKey {
    type Target = [NameOrIndex];
    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl<I: IntoIterator> From<I> for OutputKey
where
    I::Item: Into<NameOrIndex>,
{
    fn from(value: I) -> Self {
        let inner = value.into_iter().map(|value| value.into()).collect();
        OutputKey(inner)
    }
}

impl std::fmt::Display for &'_ OutputKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, key) in self.iter().enumerate() {
            match key {
                NameOrIndex::Name(name) => {
                    write!(f, "\"{name}\"")?;
                }
                NameOrIndex::Index(index) => {
                    write!(f, "{index}")?;
                }
            }

            if i+1 < self.0.len() {
                write!(f, ".")?;
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for &'_ NamedOutputRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for namespace in &self.namespaces {
            write!(f, "{namespace}:")?;
        }

        write!(f, "{}.{}", &self.operation, &self.key)?;
        Ok(())
    }
}

impl JsonSchema for OutputKey {
    fn schema_name() -> Cow<'static, str> {
        "OutputKey".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::OutputKey").into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "array",
            "items": {
                "oneOf": [
                    { "type": "string" },
                    { "type": "number" }
                ]
            }
        })
    }
}
