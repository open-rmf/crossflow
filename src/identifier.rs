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

use std::{borrow::Cow, hash::Hash, sync::Arc};

pub use crossflow_derive::{Accessor, Joined};

use crate::{ListSplitKey, MapSplitKey};

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

pub trait Identification<Value> {
    fn to_identifier(value: Value) -> Vec<Identifier>;
}

/// This is a struct that explicitly provides conversions into [`Identifier`] for
/// known Rust primitives. This exists as a workaround for the lack of template
/// specialization in Rust.
///
/// You can also use this trait to implement identification for custom data types.
pub struct BasicIdentification;

impl Identification<Arc<str>> for BasicIdentification {
    fn to_identifier(value: Arc<str>) -> Vec<Identifier> {
        vec![Identifier::Name(value)]
    }
}

impl Identification<Box<str>> for BasicIdentification {
    fn to_identifier(value: Box<str>) -> Vec<Identifier> {
        vec![Identifier::Name(value.into())]
    }
}

impl<'a> Identification<&'a str> for BasicIdentification {
    fn to_identifier(value: &'a str) -> Vec<Identifier> {
        vec![Identifier::Name(value.into())]
    }
}

impl Identification<String> for BasicIdentification {
    fn to_identifier(value: String) -> Vec<Identifier> {
        vec![Identifier::Name(value.into())]
    }
}

impl Identification<u8> for BasicIdentification {
    fn to_identifier(value: u8) -> Vec<Identifier> {
        vec![Identifier::Index(value as usize)]
    }
}

impl Identification<u16> for BasicIdentification {
    fn to_identifier(value: u16) -> Vec<Identifier> {
        vec![Identifier::Index(value as usize)]
    }
}

impl Identification<u32> for BasicIdentification {
    fn to_identifier(value: u32) -> Vec<Identifier> {
        vec![Identifier::Index(value as usize)]
    }
}

impl Identification<u64> for BasicIdentification {
    fn to_identifier(value: u64) -> Vec<Identifier> {
        vec![Identifier::Index(value as usize)]
    }
}

impl Identification<usize> for BasicIdentification {
    fn to_identifier(value: usize) -> Vec<Identifier> {
        vec![Identifier::Index(value as usize)]
    }
}

impl Identification<ListSplitKey> for BasicIdentification {
    fn to_identifier(value: ListSplitKey) -> Vec<Identifier> {
        match value {
            ListSplitKey::Sequential(seq) => {
                vec!["sequential".into(), Identifier::Index(seq)]
            }
            ListSplitKey::Remaining => {
                vec!["remaining".into()]
            }
        }
    }
}

impl<K: Clone> Identification<MapSplitKey<K>> for BasicIdentification
where
    BasicIdentification: Identification<K>,
{
    fn to_identifier(value: MapSplitKey<K>) -> Vec<Identifier> {
        match value {
            MapSplitKey::Sequential(seq) => {
                vec!["sequential".into(), Identifier::Index(seq)]
            }
            MapSplitKey::Specific(key) => {
                let mut id = vec!["keyed".into()];
                id.extend(BasicIdentification::to_identifier(key.clone()));
                id
            }
            MapSplitKey::Remaining => {
                vec!["remaining".into()]
            }
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
    pub const fn name_str(name: &'a str) -> Self {
        Self::Name(Cow::Borrowed(name))
    }

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

impl<'a> PartialEq<IdentifierRef<'a>> for Identifier {
    fn eq(&self, other: &IdentifierRef<'a>) -> bool {
        match self {
            Self::Name(lhs) => match other {
                IdentifierRef::Name(rhs) => lhs.as_ref().eq(rhs.as_ref()),
                IdentifierRef::Index(_) => false,
            },
            Self::Index(lhs) => match other {
                IdentifierRef::Name(_) => false,
                IdentifierRef::Index(rhs) => *lhs == *rhs,
            },
        }
    }
}

impl<'a> PartialEq<Identifier> for IdentifierRef<'a> {
    fn eq(&self, other: &Identifier) -> bool {
        *other == *self
    }
}

pub type OutputPort<'a> = &'a [IdentifierRef<'a>];

/// The output_port module provides utility functions for easily creating
/// [`OutputPort`] instances that avoid any memory allocations.
pub mod output_port {
    use super::IdentifierRef;

    /// Get an output key
    pub const fn next() -> [IdentifierRef<'static>; 1] {
        name_str("next")
    }

    pub const fn finish() -> [IdentifierRef<'static>; 1] {
        name_str("finish")
    }

    pub const fn inject() -> [IdentifierRef<'static>; 1] {
        name_str("inject")
    }

    pub const fn cancel() -> [IdentifierRef<'static>; 1] {
        name_str("cancel")
    }

    pub const fn broken() -> [IdentifierRef<'static>; 1] {
        name_str("broken")
    }

    pub const fn dispose() -> [IdentifierRef<'static>; 1] {
        name_str("dispose")
    }

    pub const fn unavailable() -> [IdentifierRef<'static>; 1] {
        name_str("unavailable")
    }

    pub const fn stream_out<'a>(stream: &'a str) -> [IdentifierRef<'a>; 2] {
        [
            IdentifierRef::name_str("stream_out"),
            IdentifierRef::name_str(stream),
        ]
    }

    /// This is used as a pseudo output port that refers to all streams coming
    /// out of a node. It's used when reporting unused stream disposals.
    pub const fn all_stream_out() -> [IdentifierRef<'static>; 1] {
        name_str("stream_out")
    }

    pub const fn missing_values() -> [IdentifierRef<'static>; 1] {
        name_str("missing_values")
    }

    pub const fn anonymous_stream<'a>(type_name: &'a str) -> [IdentifierRef<'a>; 2] {
        [
            IdentifierRef::name_str("anonymous_stream"),
            IdentifierRef::name_str(type_name),
        ]
    }

    pub const fn ok() -> [IdentifierRef<'static>; 1] {
        name_str("ok")
    }

    pub const fn err() -> [IdentifierRef<'static>; 1] {
        name_str("err")
    }

    pub const fn listen() -> [IdentifierRef<'static>; 1] {
        name_str("listen")
    }

    pub const fn update() -> [IdentifierRef<'static>; 1] {
        name_str("update")
    }

    pub const fn start() -> [IdentifierRef<'static>; 1] {
        name_str("start")
    }

    pub const fn begin_cleanup() -> [IdentifierRef<'static>; 1] {
        name_str("begin_cleanup")
    }

    pub const fn cleanup_buffers() -> [IdentifierRef<'static>; 1] {
        name_str("cleanup_buffers")
    }

    pub const fn name_str(name: &'static str) -> [IdentifierRef<'static>; 1] {
        [IdentifierRef::name_str(name)]
    }

    pub const fn next_index(index: usize) -> [IdentifierRef<'static>; 2] {
        [IdentifierRef::name_str("next"), IdentifierRef::Index(index)]
    }

    pub const fn sequential(index: usize) -> [IdentifierRef<'static>; 2] {
        [
            IdentifierRef::name_str("sequential"),
            IdentifierRef::Index(index),
        ]
    }

    pub const fn keyed<'a>(key: &'a str) -> [IdentifierRef<'a>; 2] {
        [
            IdentifierRef::name_str("keyed"),
            IdentifierRef::name_str(key),
        ]
    }

    pub const fn remaining() -> [IdentifierRef<'static>; 1] {
        name_str("remaining")
    }

    pub const fn buffer_update() -> [IdentifierRef<'static>; 2] {
        [
            IdentifierRef::name_str("builtin"),
            IdentifierRef::name_str("buffer_update"),
        ]
    }

    pub const fn drop() -> [IdentifierRef<'static>; 1] {
        name_str("drop")
    }
}
