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
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
};

use crate::{
    Async, DynamicallyNamedStream, JsonMessage, AnyBufferKey, TraceSettings,
    NextOperation, OperationName, IdentifierRef, StreamOf, BuildDiagramOperation,
    Templates, Operations, DiagramErrorCode, BuilderContext, BuildStatus, IntoCallback,
    Node, TraceInfo, InferenceContext, Joined, TypeInfo, DynInputSlot, BasicConnect, TypeMismatch,
    ConnectIntoTarget, DynOutput, Text, ScriptMessage,
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
    /// Specify what happens if an error occurs during the script, such as an
    /// exception or a serialization problem. If you specify a target for
    /// on_error, then an error message will be sent to that target. You can set
    /// this to `{ "builtin": "dispose" }` to simply ignore errors.
    ///
    /// If left unspecified, a failure will be treated like an implicit operation
    /// failure and behave according to the `on_implicit_error` for this operation's
    /// scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_error: Option<NextOperation>,
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
            let running = script.run(input, world);
            async move {
                running.await.map_err(|err| format!("{err}"))
            }
        };

        let Node { input, output, streams } = ctx.builder.create_node(callback.into_callback());
        let (ok, err) = ctx.builder.chain(output).fork_result(|ok| ok.output(), |err| err.output());

        let trace = TraceInfo::new(self, self.trace_settings.trace)?;
        ctx.set_input_for_target(id, input.into(), trace)?;
        ctx.add_output_into_target(&self.next, ok.into());

        let error_target = self
            .on_error
            .as_ref()
            .map(|on_error| ctx.into_operation_ref(on_error))
            .unwrap_or(
                // If no error target was explicitly given then treat this as an
                // implicit error.
                ctx.get_implicit_error_target(),
            );
        ctx.add_output_into_target(error_target, err.into());

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

/// Description of a scripting environment that a diagram uses to run scripts.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ScriptEnvironmentSchema {
    pub builder: OperationName,
    pub config: Arc<JsonMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Script {
    text: Text,
    #[serde(skip)]
    cache: Arc<Mutex<Option<Arc<std::ffi::CStr>>>>,
}

impl Script {
    pub fn new(text: impl Into<Text>) -> Self {
        Self {
            text: text.into(),
            cache: Default::default(),
        }
    }

    pub fn set_text(&mut self, text: impl Into<Text>) {
        self.text = text.into();
        let mut guard = match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
        self.cache.clear_poison();
    }

    pub fn text(&self) -> &Text {
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

pub struct ImplicitScriptMessage {
    incoming_types: HashMap<TypeInfo, DynInputSlot>,
    script_message_input: BasicConnect,
}

impl ImplicitScriptMessage {
    pub fn new(script_message_input: DynInputSlot) -> Result<Self, DiagramErrorCode> {
        if script_message_input.message_info() != &TypeInfo::of::<ScriptMessage>() {
            return Err(TypeMismatch {
                source_type: TypeInfo::of::<ScriptMessage>(),
                target_type: *script_message_input.message_info(),
            }
            .into());
        }

        Ok(Self {
            script_message_input: BasicConnect::new(script_message_input),
            incoming_types: Default::default(),
        })
    }

    pub fn try_implicit_conversion(
        &mut self,
        incoming: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<Result<(), DynOutput>, DiagramErrorCode> {
        if self.script_message_input.is_compatible(incoming.message_info(), ctx)? {
            self.script_message_input.connect_into_target(incoming, ctx)?;
            return Ok(Ok(()));
        }

        let input = match self.incoming_types.entry(*incoming.message_info()) {
            Entry::Occupied(input_slot) => input_slot.get().clone(),
            Entry::Vacant(vacant) => {
                let Some(into_script_message) = ctx
                    .registry
                    .messages
                    .try_into_script_message(incoming.message_info(), ctx.builder)?
                else {
                    // We cannot turn this type into a script message.
                    return Ok(Err(incoming));
                };

                self.script_message_input.connect_into_target(into_script_message.ok, ctx)?;

                let error_target = ctx.get_implicit_error_target();
                ctx.add_output_into_target(error_target, into_script_message.err);

                vacant.insert(into_script_message.input).clone()
            }
        };

        incoming.connect_to(&input, ctx.builder)?;

        Ok(Ok(()))
    }

    pub fn implicit_conversion(
        &mut self,
        incoming: DynOutput,
        ctx: &mut BuilderContext,
    ) -> Result<(), DiagramErrorCode> {
        self.try_implicit_conversion(incoming, ctx)?
            .map_err(|incoming| DiagramErrorCode::NotScriptable(*incoming.message_info()))
    }
}
