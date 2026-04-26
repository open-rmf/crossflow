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
    DiagramElementRegistry, ScriptEnvironmentBuilderOptions, Script, PythonAccessors, PythonMessage,
    ScriptEnvironment, ScriptExecution, ScriptInput, ScriptMessage, ScriptConfigExample,
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
use bevy_ecs::prelude::World;

use std::{
    collections::HashMap,
    sync::Arc,
};

use anyhow::{anyhow, Context, Error as Anyhow};

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
        Self::Reuse
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PythonConfig {
    #[serde(default)]
    pub ownership: PythonEnvironmentOwnership,
    /// A script that sets the variables for the environment.
    pub script: Script,
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

        let builder_description = "Run a Python interpreter directly inside the executor";
        let script = Script::new(
r###"
from crossflow import *

def execute(data: object, accessors: Accessors, config: object):
    """Execute a node in a workflow

    Keyword arguments:
    :param data: JSON-style data sent into this node as a request
    :param accessors: A collection of buffers that this node has access to
    :param config: JSON-style data set for this node in the original JSON diagram
    :return: either a JSON-style value or a crossflow.Message

    The incoming request will be split into `data` for JSON-style data and
    `accessors` which is a dictionary of buffer accessors. You can refer to the
    accessors by index or by name, as long as you're consistent with how they
    were put into the incoming request message by the `listen` or `buffer_access`
    operation that created the message.

    For the return value, you can return any value that can be converted into
    regular JSON. If you want to also pass along accessors, then you can return
    a `crossflow.Message` with a `data` field and/or an `accessors` field.
    """

    return Message(data = {}, accessors = None)
"###
        );
        let run = Script::new("execute");

        let config_examples = vec![
            ScriptConfigExample::new(
                "Shared Python Environment",
                "All local and global variables in this environment will be \
                shared among all operations run with this environment.",
                PythonConfig {
                    ownership: PythonEnvironmentOwnership::Shared,
                    script: script.clone(),
                },
                run.clone(),
            ),
            ScriptConfigExample::new(
                "Reused Python Environment",
                "The environment will be reused across multiple calls of an \
                operation within the same workflow session, but each operation \
                will have its own copy of the environment.",
                PythonConfig {
                    ownership: PythonEnvironmentOwnership::Reuse,
                    script: script.clone(),
                },
                run.clone(),
            ),
            ScriptConfigExample::new(
                "Isolated Python Environment",
                "The environment will be isolated to each run of each operation. \
                This means the whole environment script will be re-evaluated \
                with each run of an operation, and all variables will be reset \
                with each run, which may reduce performance for bulky environments.",
                PythonConfig {
                    ownership: PythonEnvironmentOwnership::Isolated,
                    script,
                },
                run,
            ),
        ];

        self.register_script_environment_builder(
            ScriptEnvironmentBuilderOptions::new(
                "process-bound-python",
                "python",
                interpreter,
            )
                .with_description(builder_description)
                .with_display_text("Python")
                .with_config_examples(config_examples),
            |config: PythonConfig| {
                let env = match config.ownership {
                    PythonEnvironmentOwnership::Shared => {
                        let shared = SharedPythonEnvironment::new(&config.script)?;
                        Arc::new(PythonEnvironment::Shared(shared))
                    }
                    PythonEnvironmentOwnership::Reuse => {
                        let reused = ReusedPythonEnvironment::new(config.script.clone());
                        Arc::new(PythonEnvironment::Reused(reused))
                    }
                    PythonEnvironmentOwnership::Isolated => {
                        let isolated = IsolatedPythonEnvironment::new(config.script.clone());
                        Arc::new(PythonEnvironment::Isolated(isolated))
                    }
                };

                Ok(env)
            },
        );
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

}

#[derive(Clone)]
pub struct ReusedPythonEnvironment {
    script: Script,
}

impl ReusedPythonEnvironment {
    pub fn new(script: Script) -> Self {
        Self { script }
    }
}

#[derive(Clone)]
pub struct IsolatedPythonEnvironment {
    environment: Script,
}

impl IsolatedPythonEnvironment {
    pub fn new(script: Script) -> Self {
        Self { environment: script }
    }
}

#[derive(Clone)]
pub enum PythonEnvironment {
    Shared(SharedPythonEnvironment),
    Reused(ReusedPythonEnvironment),
    Isolated(IsolatedPythonEnvironment),
}

impl ScriptEnvironment for PythonEnvironment {
    fn compile(
        &self,
        run: &Script,
        config: &Arc<JsonMessage>,
    ) -> Result<Arc<dyn ScriptExecution>, Anyhow> {
        let execution = match self {
            Self::Shared(shared) => {
                let exec = SharedPythonExecution::new(shared, run, &*config)?;
                PythonExecution::Shared(exec)
            },
            Self::Reused(reused) => {
                let env = SharedPythonEnvironment::new(&reused.script)?;
                let exec = SharedPythonExecution::new(&env, run, &*config)?;
                PythonExecution::Shared(exec)
            },
            Self::Isolated(isolated) => {
                let exec = IsolatedPythonExecution::new(
                    isolated.environment.clone(),
                    run.clone(),
                    Arc::clone(config),
                )?;
                PythonExecution::Isolated(exec)
            }
        };

        Ok(Arc::new(execution))
    }
}

pub struct SharedPythonExecution {
    run: Arc<Py<PyAny>>,
    config: Arc<Py<PyAny>>,
}

impl SharedPythonExecution {
    fn new(
        env: &SharedPythonEnvironment,
        run: &Script,
        config: &JsonMessage,
    ) -> Result<Self, Anyhow> {
        let config = Python::attach(|py| {
            pythonize(py, config)
                .map(|c| c.unbind())
                .context("Failed to pythonize the script config")
        })?;

        let run = Python::attach(|py| {
            let globals = env.globals.bind(py);
            let locals = env.locals.bind(py);

            let c_run = run
                .get_cstr()
                .with_context(|| format!("Failed to bind the run script [{}]", run.text()))?;

            let callable = py.eval(&*c_run, Some(globals), Some(locals))
                .with_context(|| format!("Exception while evaluating [{}]", run.text()))?;

            if !callable.is_callable() {
                return Err(anyhow!("Run script [{}] did not refer to a callable", run.text()));
            }

            Ok(callable.unbind())
        })?;

        Ok(Self {
            run: Arc::new(run),
            config: Arc::new(config),
        })
    }

    pub fn run(
        &self,
        input: ScriptInput,
    ) -> impl Future<Output = Result<ScriptMessage, Anyhow>> + 'static {
        let run = Arc::clone(&self.run);
        let config = Arc::clone(&self.config);

        let future = async move {
            Python::attach(|py| {
                let run = run.bind(py);
                let config = config.bind(py);

                let ScriptMessage { data, accessors } = input.request;

                let data = pythonize(py, &data)
                    .map_err(|err| {
                        anyhow!("failed to pythonize input data: {err}")
                    })?;

                let accessors = PythonAccessors::new(Arc::new(accessors), Arc::new(input.channel));

                let kwargs = PyDict::new(py);
                kwargs.set_item("data", data)?;
                kwargs.set_item("accessors", accessors)?;
                kwargs.set_item("config", config)?;

                let result = run
                    .call((), Some(&kwargs))
                    .map_err(|err| anyhow!("{err}"))?;

                if let Ok(message) = result.extract::<PythonMessage>() {
                    return Ok(ScriptMessage {
                        data: message.data,
                        accessors: message.accessors.map(|a| a.depythonize()).unwrap_or(HashMap::new()),
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

pub struct IsolatedPythonExecution {
    environment: Script,
    run: Script,
    config: Arc<JsonMessage>,
}

impl IsolatedPythonExecution {
    pub fn new(
        environment: Script,
        run: Script,
        config: Arc<JsonMessage>,
    ) -> Result<Self, Anyhow> {
        // Test that the overall configuration is valid while we compile the
        // environment to be run
        let env = SharedPythonEnvironment::new(&environment)?;
        SharedPythonExecution::new(&env, &run, &*config)?;

        // If the above worked okay then we'll assume that the compilation is valid
        Ok(Self {
            environment,
            run,
            config,
        })
    }

    fn run(
        &self,
        input: ScriptInput,
    ) -> impl Future<Output = Result<ScriptMessage, Anyhow>> + 'static {
        let environment = self.environment.clone();
        let run = self.run.clone();
        let config = Arc::clone(&self.config);
        async move {
            let env = SharedPythonEnvironment::new(&environment)?;
            let exec = SharedPythonExecution::new(&env, &run, &*config)?;
            exec.run(input).await
        }
    }
}

pub enum PythonExecution {
    Shared(SharedPythonExecution),
    Isolated(IsolatedPythonExecution),
}

impl ScriptExecution for PythonExecution {
    fn run(
        &self,
        input: ScriptInput,
        _: &mut World,
    ) -> BoxFuture<'static, Result<ScriptMessage, Anyhow>> {
        match self {
            Self::Shared(shared) => shared.run(input).boxed(),
            Self::Isolated(isolated) => isolated.run(input).boxed(),
        }
    }
}
