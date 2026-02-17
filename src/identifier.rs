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
    borrow::Cow,
    hash::Hash,
    sync::Arc,
};

pub use crossflow_derive::{Accessor, Joined};

#[cfg(feature = "diagram")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "diagram")]
use schemars::JsonSchema;


#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "diagram",
    derive(Serialize, Deserialize, JsonSchema),
    serde(untagged)
)]
pub enum Identifier {
    Name(Arc<str>),
    Index(usize),
}

impl<T: std::borrow::Borrow<str>> From<T> for Identifier {
    fn from(value: T) -> Self {
        Identifier::Name(value.borrow().into())
    }
}

impl<'a> From<IdentifierRef<'a>> for Identifier {
    fn from(value: IdentifierRef<'a>) -> Self {
        match value {
            IdentifierRef::Name(name) => Self::Name(name.as_ref().into()),
            IdentifierRef::Index(index) => Self::Index(index),
        }
    }
}

/// Uniquely identify something by a borrowed name or index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "diagram",
    derive(Serialize, Deserialize, JsonSchema),
    serde(untagged)
)]
pub enum IdentifierRef<'a> {
    /// Identify by a name
    Name(Cow<'a, str>),
    /// Identify by an index value
    Index(usize),
}

impl<'a> IdentifierRef<'a> {
    pub fn is_name(&self) -> bool {
        matches!(self, Self::Name(_))
    }

    pub fn is_index(&self) -> bool {
        matches!(self, Self::Index(_))
    }

    pub fn to_owned(&self) -> IdentifierRef<'static> {
        match self {
            Self::Index(index) => IdentifierRef::Index(*index),
            Self::Name(name) => match name {
                Cow::Borrowed(name) => IdentifierRef::Name(Cow::Owned((*name).into())),
                Cow::Owned(name) => IdentifierRef::Name(Cow::Owned(name.clone())),
            },
        }
    }
}

impl<'a> std::fmt::Display for IdentifierRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => write!(f, "\"{name}\""),
            Self::Index(index) => write!(f, "#{index}"),
        }
    }
}

impl IdentifierRef<'static> {
    /// Clone a name to use as an identifier.
    pub fn clone_name(name: &str) -> Self {
        IdentifierRef::Name(Cow::Owned(name.to_owned()))
    }

    /// Borrow a string literal name to use as an identifier.
    pub fn literal_name(name: &'static str) -> Self {
        IdentifierRef::Name(Cow::Borrowed(name))
    }

    /// Use an index as an identifier.
    pub fn index(index: usize) -> Self {
        IdentifierRef::Index(index)
    }
}

impl<'a> From<&'a str> for IdentifierRef<'a> {
    fn from(value: &'a str) -> Self {
        IdentifierRef::Name(Cow::Borrowed(value))
    }
}

impl From<String> for IdentifierRef<'static> {
    fn from(value: String) -> Self {
        IdentifierRef::Name(Cow::Owned(value))
    }
}

impl<'a> From<usize> for IdentifierRef<'a> {
    fn from(value: usize) -> Self {
        IdentifierRef::Index(value)
    }
}

impl<'a> From<Identifier> for IdentifierRef<'a> {
    fn from(value: Identifier) -> Self {
        match value {
            Identifier::Name(name) => Self::Name(Cow::Owned(name.to_string())),
            Identifier::Index(index) => Self::Index(index),
        }
    }
}

impl<'a> From<&'a Identifier> for IdentifierRef<'a> {
    fn from(value: &'a Identifier) -> Self {
        match value {
            Identifier::Name(name) => Self::Name(Cow::Borrowed(name.as_ref())),
            Identifier::Index(index) => Self::Index(*index),
        }
    }
}
