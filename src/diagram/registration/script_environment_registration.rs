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
    cell::RefCell,
    collections::HashMap,
    sync::Arc,
};

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Serialize};
use anyhow::Error as Anyhow;

use crate::{
    ConfigExample, JsonMessage, ScriptEnvironment, OperationName, BuilderId,
    ScriptEnvironmentSchema,
};

pub type ArcScriptEnvironment = Arc<dyn ScriptEnvironment + Send + Sync>;

type CreateScriptEnvironmentFn =
    dyn FnMut(JsonMessage) -> Result<ArcScriptEnvironment, Anyhow> + Send;

pub struct ScriptEnvironmentRegistration {
    pub(crate) metadata: ScriptEnvironmentMetadata,
    pub(crate) create_environment_impl: RefCell<Box<CreateScriptEnvironmentFn>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScriptEnvironmentMetadata {
    /// The scripting language that will be used for this environment
    pub(crate) language: OperationName,
    /// The interpreter that will be used to process the scripting language
    pub(crate) interpreter: OperationName,
    /// The schema for configuring an environment made by this builder
    pub(crate) config_schema: Schema,
    /// Human-friendly name for the script environment builder
    pub(crate) display_text: Option<Arc<str>>,
    /// A description of what kind of environments are made by this builder
    pub(crate) description: Option<Arc<str>>,
    /// Examples of valid configurations for this builder
    pub(crate) config_examples: Vec<ConfigExample>,
}
