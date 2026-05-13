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
    ScriptEnvironment, ScriptExecution, ArcScriptExecution, ScriptInput, ScriptMessage, ScriptConfigExample,
    JsonMessage, PythonInput,
};
use pyo3::{
    prelude::*,
    types::{PyDict, PyAnyMethods},
};
use pyo3_async_runtimes::TaskLocals as PyTaskLocals;
use pythonize::{depythonize, pythonize};
use futures::future::{BoxFuture, FutureExt};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use bevy_ecs::prelude::World;

use std::sync::Arc;

use anyhow::{anyhow, Context, Error as Anyhow};

#[derive(Clone)]
pub struct PythonEventLoop {
    asyncio_event_loop: Arc<Py<PyAny>>,
}

impl PythonEventLoop {
    pub fn from_any(asyncio_event_loop: Arc<Py<PyAny>>) -> Self {
        Self { asyncio_event_loop }
    }

    /// Create a new Python asyncio event loop instance
    pub fn new() -> Result<Self, PyErr> {
        let asyncio_event_loop = Python::attach(|py| {
            let asyncio = py.import("asyncio")?;
            Ok::<_, PyErr>(asyncio.call_method0("new_event_loop")?.unbind())
        })?;

        Ok(Self { asyncio_event_loop: Arc::new(asyncio_event_loop) })
    }

    fn get_task_locals(&self) -> PyTaskLocals {
        Python::attach(|py| {
            let event_loop = self.asyncio_event_loop.clone_ref(py).into_bound(py);
            pyo3_async_runtimes::TaskLocals::new(event_loop)
        })
    }

    /// Run the event loop indefinitely. This is a blocking function, so it must
    /// be run on a separate thread from the main thread of the Bevy app.
    pub fn run(&self) -> Result<(), PyErr> {
        Python::attach(|py| {
            self.asyncio_event_loop.call_method0(py, "run_forever")
        })
        .map(|_| ())
    }

    /// Spawn a thread for running this event loop.
    pub fn spawn_thread_and_run(&self) -> std::thread::JoinHandle<Result<(), PyErr>> {
        let py_event_loop = self.clone();
        std::thread::spawn(move || {
            py_event_loop.run()
        })
    }

    /// Tell the event loop to stop running. This does not block, so the event
    /// loop could still run for some time after this function returns.
    pub fn stop(&self) -> Result<(), PyErr> {
        Python::attach(|py| {
            self.asyncio_event_loop.call_method0(py, "stop")
        })
        .map(|_| ())
    }
}

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
    /// reused across all calls to its node, so changes to nonlocal variables
    /// will persist across runs.
    Persistent,
    /// Each time a script is run in this environment, it will get a fresh copy
    /// of the environment and all its variables.
    ///
    /// Note that if you have a base environment script this setting will rerun
    /// that base script from scratch each time a script operation is run. This
    /// could have consequences on performance, so you may want to consider
    /// [`Self::Persistent`] instead.
    Isolated,
}

impl Default for PythonEnvironmentOwnership {
    fn default() -> Self {
        Self::Persistent
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
    pub fn enable_python(&mut self, event_loop: &PythonEventLoop) -> pyo3::PyResult<()> {

        crate::register_crossflow_pymod()?;

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

async def execute(input: Input):
    """Execute a node in a workflow

    Arguments:
    :param input.data: JSON-style data sent into this node as a request
    :param input.accessors: A collection of buffers that this node has access to
    :param input.config: JSON-style data set for this node in the original JSON diagram
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
                    ownership: PythonEnvironmentOwnership::Persistent,
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

        let task_locals = Arc::new(event_loop.get_task_locals());
        self.register_script_environment_builder(
            ScriptEnvironmentBuilderOptions::new(
                "process-bound-python",
                "python",
                interpreter,
            )
                .with_description(builder_description)
                .with_display_text("Python")
                .with_config_examples(config_examples),
            move |config: PythonConfig| {
                let env = match config.ownership {
                    PythonEnvironmentOwnership::Shared => {
                        let shared = SharedPythonEnvironment::new(&config.script, &task_locals)?;
                        Arc::new(PythonEnvironment::Shared(shared))
                    }
                    PythonEnvironmentOwnership::Persistent => {
                        let reused = PersistentPythonEnvironment::new(config.script.clone(), Arc::clone(&task_locals));
                        Arc::new(PythonEnvironment::Persistent(reused))
                    }
                    PythonEnvironmentOwnership::Isolated => {
                        let isolated = IsolatedPythonEnvironment::new(config.script.clone(), Arc::clone(&task_locals));
                        Arc::new(PythonEnvironment::Isolated(isolated))
                    }
                };

                Ok(env)
            },
        );

        Ok(())
    }
}

#[derive(Clone)]
pub struct SharedPythonEnvironment {
    py_vars: Arc<Py<PyDict>>,
    task_locals: Arc<PyTaskLocals>,
}

impl SharedPythonEnvironment {
    pub fn new(script: &Script, task_locals: &Arc<PyTaskLocals>) -> Result<Self, Anyhow> {
        Python::attach(|py| {
            let py_vars = PyDict::new(py);

            let c_script = script
                .get_cstr()
                .map_err(|err| {
                    anyhow!("unable to convert script: {err}")
                })?;

            py.run(&*c_script, Some(&py_vars), None)
                .map_err(|err| anyhow!("exception while running script: {err}"))?;

            Ok(Self {
                py_vars: Arc::new(py_vars.unbind()),
                task_locals: Arc::clone(&task_locals),
            })
        })
    }

}

#[derive(Clone)]
pub struct PersistentPythonEnvironment {
    script: Script,
    task_locals: Arc<PyTaskLocals>,
}

impl PersistentPythonEnvironment {
    pub fn new(script: Script, task_locals: Arc<PyTaskLocals>) -> Self {
        Self { script, task_locals }
    }
}

#[derive(Clone)]
pub struct IsolatedPythonEnvironment {
    environment: Script,
    task_locals: Arc<PyTaskLocals>,
}

impl IsolatedPythonEnvironment {
    pub fn new(script: Script, task_locals: Arc<PyTaskLocals>) -> Self {
        Self { environment: script, task_locals }
    }
}

#[derive(Clone)]
pub enum PythonEnvironment {
    Shared(SharedPythonEnvironment),
    Persistent(PersistentPythonEnvironment),
    Isolated(IsolatedPythonEnvironment),
}

impl ScriptEnvironment for PythonEnvironment {
    fn compile(
        &self,
        run: &Script,
        config: &Arc<JsonMessage>,
    ) -> Result<ArcScriptExecution, Anyhow> {
        let execution = match self {
            Self::Shared(shared) => {
                let exec = SharedPythonExecution::new(shared, run, &*config)?;
                PythonExecution::Shared(exec)
            },
            Self::Persistent(persistent) => {
                let env = SharedPythonEnvironment::new(&persistent.script, &persistent.task_locals)?;
                let exec = SharedPythonExecution::new(&env, run, &*config)?;
                PythonExecution::Shared(exec)
            },
            Self::Isolated(isolated) => {
                let exec = IsolatedPythonExecution::new(
                    isolated.environment.clone(),
                    run.clone(),
                    Arc::clone(config),
                    Arc::clone(&isolated.task_locals),
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
    task_locals: Arc<PyTaskLocals>,
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
            let py_vars = env.py_vars.bind(py);

            let c_run = run
                .get_cstr()
                .with_context(|| format!("Failed to bind the run script [{}]", run.text()))?;

            let callable = py.eval(&*c_run, Some(py_vars), None)
                .with_context(|| format!("Exception while evaluating [{}]", run.text()))?;

            if !callable.is_callable() {
                return Err(anyhow!("Run script [{}] did not refer to a callable", run.text()));
            }

            Ok(callable.unbind())
        })?;

        Ok(Self {
            run: Arc::new(run),
            config: Arc::new(config),
            task_locals: Arc::clone(&env.task_locals),
        })
    }

    pub fn run(
        &self,
        input: ScriptInput,
    ) -> impl Future<Output = Result<ScriptMessage, Anyhow>> + 'static {
        let run = Arc::clone(&self.run);
        let config = Arc::clone(&self.config);
        let task_locals = Arc::clone(&self.task_locals);

        let future = async move {
            let (result, is_async) = Python::attach(|py| {
                let run = run.bind(py);

                let ScriptMessage { data, accessors } = input.request;
                // input.streams.send(data);

                let data = pythonize(py, &data)
                    .map_err(|err| {
                        anyhow!("failed to pythonize input data: {err}")
                    })?;

                let accessors = PythonAccessors::new(Arc::new(accessors), Arc::new(input.channel));

                let input = PythonInput {
                    data: Arc::new(data.unbind()),
                    streams: input.streams,
                    accessors,
                    config,
                };

                let result = run
                    .call((input,), None)?;

                let is_async = result.hasattr("__await__")?;
                Ok::<_, Anyhow>((result.unbind(), is_async))
            })?;

            let result = if is_async {
                let result = Python::attach(move |py| {
                    let result = result.into_bound(py);
                    pyo3_async_runtimes::into_future_with_locals(&task_locals, result)
                        .map_err(|err| anyhow!("{err}"))
                });

                result?.await.map_err(|err| anyhow!("{err}"))?
            } else {
                result
            };

            Python::attach(|py| {
                let result = result.into_bound(py);
                if let Ok(message) = result.extract::<PythonMessage>() {
                    return Ok(ScriptMessage::from(message));
                }

                // The user didn't return a PythonMessge, so let's try to
                // depythonize their return value.
                let data: JsonMessage = depythonize(&result)
                    .map_err(|err| {
                        anyhow!("failed to depythonize return value: {err}")
                    })?;

                Ok(ScriptMessage::from(data))
            })
        };

        future.boxed()
    }
}

pub struct IsolatedPythonExecution {
    environment: Script,
    run: Script,
    config: Arc<JsonMessage>,
    task_locals: Arc<PyTaskLocals>,
}

impl IsolatedPythonExecution {
    pub fn new(
        environment: Script,
        run: Script,
        config: Arc<JsonMessage>,
        task_locals: Arc<PyTaskLocals>,
    ) -> Result<Self, Anyhow> {
        // Test that the overall configuration is valid while we compile the
        // environment to be run
        let env = SharedPythonEnvironment::new(&environment, &task_locals)?;
        SharedPythonExecution::new(&env, &run, &*config)?;

        // If the above worked okay then we'll assume that the compilation is valid
        Ok(Self {
            environment,
            run,
            config,
            task_locals,
        })
    }

    fn run(
        &self,
        input: ScriptInput,
    ) -> impl Future<Output = Result<ScriptMessage, Anyhow>> + 'static {
        let environment = self.environment.clone();
        let run = self.run.clone();
        let config = Arc::clone(&self.config);
        let task_locals = Arc::clone(&self.task_locals);
        async move {
            let env = SharedPythonEnvironment::new(&environment, &task_locals)?;
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

#[cfg(test)]
mod tests {
    use crate::{prelude::*, diagram::testing::*};
    use serde_json::json;

    #[test]
    fn test_script_message_conversion() {
        let mut fixture = DiagramTestFixture::new();

        let py_event_loop = PythonEventLoop::new().unwrap();
        fixture.registry.enable_python(&py_event_loop).unwrap();
        py_event_loop.spawn_thread_and_run();

        let env_script =
r###"
def run_config_test(input):
    if input.config is not None:
        num_accessors = input.config.get('test_num_accessors')
        if num_accessors is not None:
            assert(len(input.accessors) == num_accessors)

def execute_blocking(input):
    run_config_test(input)
    return input.data

async def execute_async(input):
    run_config_test(input)
    return input.data
"###;

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "script_environments": {
                "test_env": {
                    "builder": "process-bound-python",
                    "config": {
                        "script": env_script,
                    }
                }
            },
            "start": "script_basic",
            "ops": {
                "script_basic": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "execute_blocking",
                    "config": {
                        "test_num_accessors": 0
                    },
                    "next": "script_async"
                },
                "script_async": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "execute_async",
                    "config": {
                        "test_num_accessors": 0
                    },
                    "next": { "builtin": "terminate" }
                }
            }
        }))
        .unwrap();

        let r: f32 = fixture.spawn_and_run(&diagram, 10.0).unwrap();
        assert_eq!(r, 10.0);

        py_event_loop.stop().unwrap();
    }

    #[test]
    fn test_python_script_streams() {
        let mut fixture = DiagramTestFixture::new();

        let py_event_loop = PythonEventLoop::new().unwrap();
        fixture.registry.enable_python(&py_event_loop).unwrap();
        py_event_loop.spawn_thread_and_run();

        let env_script =
r###"
from crossflow import *

def stream_out_values(input: Input):
    for value in input.data:
        input.stream_out('values', value)

def filter_values(input: Input):
    value = input.data
    limit = input.config
    if value > limit:
        input.stream_out('high', value)
    else:
        input.stream_out('low', value)
"###;

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "script_environments": {
                "test_env": {
                    "builder": "process-bound-python",
                    "config": {
                        "script": env_script,
                    }
                }
            },
            "start": "streaming_script",
            "ops": {
                "streaming_script": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "stream_out_values",
                    "stream_out": {
                        "values": "filter"
                    },
                    "next": { "builtin": "dispose" }
                },
                "filter": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "filter_values",
                    "config": 4,
                    "stream_out": {
                        "high": { "builtin": "terminate" }
                    },
                    "next": { "builtin" : "dispose" }
                }
            }
        }))
        .unwrap();

        fixture.registry.register_message::<Vec<i32>>();

        let values = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let r: i32 = fixture.spawn_and_run(&diagram, values).unwrap();
        assert!(r > 4);

        py_event_loop.stop().unwrap();
    }

    #[test]
    fn test_python_buffer_listen() {
        let mut fixture = DiagramTestFixture::new();

        let py_event_loop = PythonEventLoop::new().unwrap();
        fixture.registry.enable_python(&py_event_loop).unwrap();
        py_event_loop.spawn_thread_and_run();

        let env_script =
r###"
import asyncio
from crossflow import *

async def slow_stream(input: Input):
    delay = input.config['delay']
    for value in input.data:
        input.stream_out('value', value)
        await asyncio.sleep(delay)

async def success_when_equal(input: Input):
    equal_value = await input.accessors.access(check_equal)
    if equal_value is not None:
        input.stream_out('equal', Message(data = equal_value))

def check_equal(access):
    a = access[0].get_oldest()
    b = access[1].get_oldest()
    if a is not None and b is not None:
        if a == b:
            return a

    # When the values are not equal, return None
    return None
"###;

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "script_environments": {
                "test_env": {
                    "builder": "process-bound-python",
                    "config": {
                        "script": env_script
                    }
                }
            },
            "start": "split",
            "ops": {
                "split": {
                    "type": "split",
                    "sequential": [
                        "left_stream",
                        "right_stream",
                    ]
                },
                "left_stream": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "slow_stream",
                    "config": {
                        "delay": 0.002
                    },
                    "stream_out": {
                        "value": "left_buffer"
                    },
                    "next": { "builtin": "dispose" }
                },
                "left_buffer": { "type": "buffer" },
                "right_stream": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "slow_stream",
                    "config": {
                        "delay": 0.002
                    },
                    "stream_out": {
                        "value": "right_buffer"
                    },
                    "next": { "builtin": "dispose" }
                },
                "right_buffer": { "type": "buffer" },
                "listen": {
                    "type": "listen",
                    "buffers": ["left_buffer", "right_buffer"],
                    "next": "success_when_equal"
                },
                "success_when_equal": {
                    "type": "script",
                    "environment": "test_env",
                    "run": "success_when_equal",
                    "stream_out": {
                        "equal": { "builtin": "terminate" }
                    },
                    "next": { "builtin": "dispose" }
                }
            }
        }))
        .unwrap();

        let input = json!([
            [0, 1, 2, 3, 4, 5],
            [10, 9, 8, 7, 6, 5],
        ]);
        let r: i32 = fixture.spawn_and_run(&diagram, input).unwrap();
        assert_eq!(r, 5);

        py_event_loop.stop().unwrap();
    }
}
