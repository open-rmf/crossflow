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
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    Async, DynamicallyNamedStream, JsonMessage, AnyBufferKey, TraceSettings,
    NextOperation, OperationName, IdentifierRef, StreamOf, BuildDiagramOperation,
    Templates, Operations, DiagramErrorCode, BuilderContext, BuildStatus, IntoCallback,
    Node, TraceInfo, InferenceContext, Joined,
    is_default,
};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ScriptSchema {
    /// Name of the environment that will be used to execute this script operation.
    pub environment: OperationName,
    /// What to run in the environment
    pub run: Script,
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

impl BuildDiagramOperation for ScriptSchema {
    fn build_diagram_operation<'a, 'c>(
        &self,
        id: &OperationName,
        ctx: &mut BuilderContext,
    ) -> Result<BuildStatus, DiagramErrorCode> {
        let env = ctx.get_script_environment(&self.environment)?;
        let script = env.compile(&self.run, &self.config)
            .map_err(|error| DiagramErrorCode::ScriptCompileError {
                environment: self.environment.clone(),
                error: Arc::new(error),
            })?;

        let callback = move |input: ScriptInput, world: &mut World| {
            script.run(input, world)
        };

        let Node { input, output, streams } = ctx.builder.create_node(callback.into_callback());

        let trace = TraceInfo::new(self, self.trace_settings.trace)?;
        ctx.set_input_for_target(id, input.into(), trace)?;
        ctx.add_output_into_target(&self.next, output.into());

        if !self.stream_out.is_empty() {
            let mut outputs = Vec::new();
            streams
                .chain(ctx.builder)
                .split(|mut split| {
                    for (name, target) in &self.stream_out {
                        let name: Cow<'static, str> = Cow::Owned(name.as_ref().to_owned());
                        let output = split.specific_chain(
                            name,
                            |chain| chain.map_block(|(_, value)| value).output(),
                        )?;

                        outputs.push((target, output));
                    }

                    Ok::<_, DiagramErrorCode>(())
                })?;

            for (target, output) in outputs {
                ctx.add_output_into_target(target, output.into());
            }
        }

        Ok(BuildStatus::Finished)
    }

    fn apply_message_type_constraints(
        &self,
        id: &OperationName,
        ctx: &mut InferenceContext,
    ) -> Result<(), DiagramErrorCode> {
        ctx.script(id, self)
    }

    fn child_operations(&self, _: &Templates) -> Result<Option<Operations>, DiagramErrorCode> {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone, Joined, Serialize, Deserialize, JsonSchema)]
pub struct ScriptMessage {
    pub data: JsonMessage,
    #[serde(skip)]
    pub accessors: HashMap<IdentifierRef<'static>, AnyBufferKey>,
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
    text: Arc<str>,
    #[serde(skip)]
    cache: Arc<Mutex<Option<Arc<std::ffi::CStr>>>>,
}

impl Script {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self {
            text: text.into(),
            cache: Default::default(),
        }
    }

    pub fn set_text(&mut self, text: impl Into<Arc<str>>) {
        self.text = text.into();
        let mut guard = match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
        self.cache.clear_poison();
    }

    pub fn text(&self) -> &Arc<str> {
        &self.text
    }

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

impl Default for Script {
    fn default() -> Self {
        Self {
            text: String::new().into(),
            cache: Default::default(),
        }
    }
}

pub type ArcScriptExecution = Arc<dyn ScriptExecution + Send + Sync>;

pub trait ScriptEnvironment {
    fn compile(
        &self,
        run: &Script,
        config: &Arc<JsonMessage>,
    ) -> Result<ArcScriptExecution, Anyhow>;
}

pub type ScriptInput = Async<ScriptMessage, DynamicallyNamedStream<StreamOf<ScriptMessage>>>;

pub trait ScriptExecution {
    fn run(&self, input: ScriptInput, world: &mut World) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>>;
}
