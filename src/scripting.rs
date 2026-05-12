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

use crate::{JsonMessage, IdentifierRef, AnyBufferKey, Joined};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use std::collections::HashMap;

/// This is a message type designed to be passed in and out of scripting
/// environments, such as Python bindings or CEL operations.
#[derive(Debug, Default, Clone, Joined, Serialize, Deserialize, JsonSchema)]
pub struct ScriptMessage {
    pub data: JsonMessage,
    #[serde(skip)]
    pub accessors: HashMap<IdentifierRef<'static>, AnyBufferKey>,
}

impl From<JsonMessage> for ScriptMessage {
    fn from(data: JsonMessage) -> Self {
        Self {
            data,
            accessors: Default::default(),
        }
    }
}
