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

use anyhow::Error as Anyhow;
use bevy_ecs::prelude::World;
use futures::future::BoxFuture;
use tokio::sync::oneshot::Receiver;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    Async, DynamicallyNamedStream, JsonMessage, JsonBufferKey, TraceSettings,
    NextOperation, OperationName, IdentifierRef, StreamOf,
    is_default,
};

#[derive(Debug, Default, Clone)]
pub struct ScriptMessage {
    pub data: JsonMessage,
    pub accessors: HashMap<IdentifierRef<'static>, JsonBufferKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ScriptSchema {
    /// Name of the environment that will be used to execute this script operation.
    pub environment: OperationName,
    /// What to run in the environment
    pub run: Arc<str>,
    /// Configured data to pass into the function that `run` refers to. This will
    /// be passed in as a keyword argument named `config`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub config: Arc<JsonMessage>,
    /// The operation that the final output of this Python operation will be passed to
    pub next: NextOperation,
    /// A map from the name of a stream to the operation that its outputs should
    /// be passed to.
    #[serde(default, skip_serializing_if = "is_default")]
    pub stream_out: HashMap<OperationName, NextOperation>,
    #[serde(flatten)]
    pub trace_settings: TraceSettings,
}

/// Description of a scripting environment that a diagram uses to run scripts.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ScriptEnvironmentSchema {
    pub builder: OperationName,
    pub config: Arc<JsonMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Script {
    pub text: Arc<str>,
    #[serde(skip)]
    cache: Arc<Mutex<Option<Arc<std::ffi::CStr>>>>,
}

impl Script {
    /// Get a C-compatible string for the script so it can be passed to a Python
    /// interpreter.
    pub fn get_cstr(&self) -> Result<Arc<std::ffi::CStr>, std::ffi::NulError> {
        let mut guard = match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
                guard
            }
        };

        if let Some(cstr) = (*guard).clone() {
            return Ok(cstr);
        }

        let cstring = std::ffi::CString::new(&*self.text)?;
        let cstr: Arc<std::ffi::CStr> = cstring.into();

        *guard = Some(cstr.clone());
        self.cache.clear_poison();
        Ok(cstr)
    }
}

pub trait ScriptEnvironment {
    fn compile(&self, script: &Script) -> Result<Arc<dyn ScriptExecution>, Anyhow>;
}

pub type ScriptInput = Async<ScriptMessage, DynamicallyNamedStream<StreamOf<ScriptMessage>>>;

pub trait ScriptExecution {
    fn run(&self, input: ScriptInput, world: &mut World) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>>;
}
