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

use crate::{
    DiagramElementRegistry, ScriptEnvironmentBuilderOptions, Script, PythonAccessor, PythonMessage,
    PythonChannel, ScriptEnvironment, ScriptExecution, ScriptInput, ScriptMessage, ConfigExample,
    JsonMessage,
};
use pyo3::{
    prelude::*,
    types::PyDict,
};
use pythonize::{depythonize, pythonize};
use futures::future::{BoxFuture, FutureExt};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use bevy_ecs::prelude::{Entity, World};

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Error as Anyhow};

/// When a node uses a certain Python environment, should the environment and
/// all its variables be reused each time a script is run in the environment,
/// or should the environment be rebuilt each time a script is run?
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PythonEnvironmentOwnership {
    /// All variables will be shared across every node that gets run in the
    /// environment and reused with each run of each node.
    Shared,
    /// Each node has its own copy of the environment, and each copy will be
    /// reused each time its node runs within the same session.
    Reuse,
    /// Each time a script is run in this environment, it will get a fresh copy
    /// of the environment and all its variables.
    ///
    /// Note that if you have a base environment script this setting will rerun
    /// that base script from scratch each time a script operation is run. This
    /// could have consequences on performance, so you may want to consider `Reuse`
    /// instead.
    Isolated,
}

impl Default for PythonEnvironmentOwnership {
    fn default() -> Self {
        Self::Shared
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PythonConfig {
    #[serde(default)]
    pub ownership: PythonEnvironmentOwnership,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<Script>,
}

impl DiagramElementRegistry {
    /// Enable script environment support for Python using pyo3 and the CPython
    /// interpreter.
    pub fn enable_python(&mut self) {
        let interpreter = Python::attach(|py| {
            let sys = py.import("sys")?;
            let version: String = sys.getattr("version")?.extract()?;
            Ok::<_, PyErr>(version)
        })
        .unwrap_or_else(|_| String::from("<unknown>"));

        let description = "Run a Python interpreter directly inside the executor";
        let config_examples = vec![
            ConfigExample::new(
                "Share environment variables across all nodes",
                PythonConfig {
                    ownership: PythonEnvironmentOwnership::Shared,
                    script: None,
                },
            ),
            ConfigExample::new(
                "Reuse variables between runs of each node",
                PythonConfig {
                    ownership: PythonEnvironmentOwnership::Reuse,
                    script: None,
                },
            ),
            ConfigExample::new(
                "Isolate variables per run",
                PythonConfig {
                    ownership: PythonEnvironmentOwnership::Isolated,
                    script: None,
                },
            ),
        ];

        // self.register_script_environment_builder(
        //     ScriptEnvironmentBuilderOptions::new(
        //         "process-bound-python",
        //         "python",
        //         interpreter,
        //     )
        //         .with_description(description)
        //         .with_display_text("Python")
        //         .with_config_examples(config_examples),
        //     |config: PythonConfig| {
        //         let env = match config.ownership {
        //             PythonEnvironmentOwnership::Shared => {
        //                 let shared = SharedPythonEnvironment::new(config.script.as_ref())?;
        //                 Arc::new(PythonEnvironment::Shared(shared))
        //             }
        //             PythonEnvironmentOwnership::Reuse => {
        //                 let reused = ReusedPythonEnvironment::new(config.script.clone());
        //                 Arc::new(PythonEnvironment::Reused(reused))
        //             }
        //             PythonEnvironmentOwnership::Isolated => {
        //                 let isolated = IsolatedPythonEnvironment::new(config.script.clone());
        //                 Arc::new(PythonEnvironment::Isolated(isolated))
        //             }
        //         };

        //         Ok(env)
        //     },
        // );
    }
}

pub struct PythonExecution {
    environment: PythonEnvironment,
    run: Arc<str>,
}

impl ScriptExecution for PythonExecution {
    fn run(&self, input: ScriptInput, world: &mut World) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>> {
        self.environment.run(&self.run, input, world)
    }
}

#[derive(Clone)]
pub struct SharedPythonEnvironment {
    globals: Arc<Py<PyDict>>,
    locals: Arc<Py<PyDict>>,
}

impl SharedPythonEnvironment {
    pub fn new(script: &Script) -> Result<Self, Anyhow> {
        Python::attach(|py| {
            let globals = PyDict::new(py);
            let locals = PyDict::new(py);

            let c_script = script
                .get_cstr()
                .map_err(|err| {
                    anyhow!("unable to convert script: {err}")
                })?;

            py.run(&*c_script, Some(&globals), Some(&locals))
                .map_err(|err| anyhow!("exception while running script: {err}"))?;

            Ok(Self {
                globals: Arc::new(globals.unbind()),
                locals: Arc::new(locals.unbind()),
            })
        })
    }

    pub fn run(&self, run: &Arc<str>, input: ScriptInput) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>> {
        let run = Arc::clone(run);
        let globals = Arc::clone(&self.globals);
        let locals = Arc::clone(&self.locals);

        let future = async move {
            Python::attach(|py| {
                let globals = globals.bind(py);
                let locals = locals.bind(py);

                let callable = locals.get_item(&*run)
                    .map_err(|err| {
                        anyhow!("exception while looking for symbol {run} in local variables: {err}")
                    })?;

                let callable = match callable {
                    Some(callable) => callable,
                    None => {
                        globals.get_item(&*run).map_err(|err| {
                            anyhow!("exception while looking for symbol {run} in global variables: {err}")
                        })?
                        .ok_or_else(|| {
                            anyhow!("symbol {run} does not exist in the local or global variables")
                        })?
                    }
                };

                let ScriptMessage { data, accessors } = input.request;

                let data = pythonize(py, &data)
                    .map_err(|err| {
                        anyhow!("failed to pythonize input data: {err}")
                    })?;

                let accessors = PythonAccessor::new(Arc::new(accessors));
                let channel = PythonChannel::new(Arc::new(input.channel));

                let kwargs = PyDict::new(py);
                kwargs.set_item("data", data)?;
                kwargs.set_item("accessors", accessors)?;
                kwargs.set_item("channel", channel)?;

                let result = callable
                    .call((), Some(&kwargs))
                    .map_err(|err| anyhow!("{err}"))?;

                if let Ok(message) = result.extract::<PythonMessage>() {
                    return Ok(ScriptMessage {
                        data: message.data,
                        accessors: message.accessors.depythonize(),
                    });
                }

                // The user didn't return a PythonMessge, so let's try to
                // depythonize their return value.
                let data: JsonMessage = depythonize(&result)
                    .map_err(|err| {
                        anyhow!("failed to depythonize return value: {err}")
                    })?;

                Ok(ScriptMessage { data, accessors: Default::default() })
            })
        };

        future.boxed()
    }
}

#[derive(Clone)]
pub struct ReusedPythonEnvironment {
    cache: Arc<Mutex<HashMap<NodeSession, SharedPythonEnvironment>>>,
    script: Script,
}

impl ReusedPythonEnvironment {
    pub fn new(script: Script) -> Self {
        Self {
            cache: Default::default(),
            script,
        }
    }

    pub fn run(&self, run: &Arc<str>, input: ScriptInput, world: &mut World) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>> {
        if let Ok(mut guard) = self.cache.lock() {
            // Clear out any finished sessions to avoid unnecessary memory growth
            guard.retain(|e, _| {
                world.get_entity(e.session).is_ok()
            });
        }

        let op = NodeSession {
            source: input.id.source,
            session: input.id.session,
        };

        let mut guard = match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut inner = poisoned.into_inner();
                inner.clear();
                inner
            }
        };

        let env = match guard.entry(op) {
            Entry::Occupied(env) => Ok(env.get().clone()),
            Entry::Vacant(vacant) => {
                match SharedPythonEnvironment::new(&self.script) {
                    Ok(env) => Ok(vacant.insert(env).clone()),
                    Err(err) => Err(err)
                }
            }
        };
        self.cache.clear_poison();

        let run = Arc::clone(run);
        let future = async move {
            let env = env?;
            env.run(&run, input).await
        };

        future.boxed()
    }
}

#[derive(Clone)]
pub struct IsolatedPythonEnvironment {
    script: Script,
}

impl IsolatedPythonEnvironment {
    pub fn new(script: Script) -> Self {
        Self { script }
    }

    pub fn run(&self, run: &Arc<str>, input: ScriptInput) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>> {
        let script = self.script.clone();
        let run = Arc::clone(run);
        let future = async move {
            let env = SharedPythonEnvironment::new(&script)?;
            env.run(&run, input).await
        };

        future.boxed()
    }
}

#[derive(Clone)]
pub enum PythonEnvironment {
    Shared(SharedPythonEnvironment),
    Reused(ReusedPythonEnvironment),
    Isolated(IsolatedPythonEnvironment),
}

impl PythonEnvironment {
    pub fn run(&self, run: &Arc<str>, input: ScriptInput, world: &mut World) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>> {
        match self {
            Self::Shared(shared) => shared.run(run, input),
            Self::Reused(reused) => reused.run(run, input, world),
            Self::Isolated(isolated) => isolated.run(run, input),
        }
    }
}

impl ScriptEnvironment for PythonEnvironment {
    fn compile(&self, script: &Script) -> Result<Arc<dyn ScriptExecution>, Anyhow> {
        Ok(Arc::new(PythonExecution {
            environment: self.clone(),
            run: Arc::clone(&script.text),
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NodeSession {
    source: Entity,
    session: Entity,
}
