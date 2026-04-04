use bevy_app::App;
use crossflow::{
    CrossflowExecutorApp, Diagram, DiagramElementRegistry, JsonMessage, NodeBuilderOptions,
    RequestExt, RunCommandsOnWorldExt,
};
use pyo3::{
    Bound, Py, PyAny, PyResult, Python,
    exceptions::{PyRuntimeError, PyValueError},
    pyclass, pymethods, pymodule,
    types::{PyAnyMethods, PyModule, PyModuleMethods},
};
use pythonize::{depythonize, pythonize};
use std::sync::Arc;

fn value_error(message: impl Into<String>) -> pyo3::PyErr {
    PyValueError::new_err(message.into())
}

fn runtime_error(message: impl Into<String>) -> pyo3::PyErr {
    PyRuntimeError::new_err(message.into())
}

fn json_from_python(obj: &Bound<'_, PyAny>) -> PyResult<JsonMessage> {
    depythonize(obj).map_err(|err| value_error(format!("value must be JSON-compatible: {err}")))
}

fn json_to_python(py: Python<'_>, value: &JsonMessage) -> PyResult<Py<PyAny>> {
    pythonize(py, value)
        .map(Bound::unbind)
        .map_err(|err| runtime_error(format!("failed to convert JSON to Python: {err}")))
}

fn diagram_from_python(obj: &Bound<'_, PyAny>) -> PyResult<Diagram> {
    if let Ok(text) = obj.extract::<String>() {
        return Diagram::from_json_str(&text)
            .map_err(|err| value_error(format!("invalid diagram JSON: {err}")));
    }

    let diagram_json: JsonMessage = depythonize(obj)
        .map_err(|err| value_error(format!("diagram must be JSON-compatible: {err}")))?;
    Diagram::from_json(diagram_json)
        .map_err(|err| value_error(format!("invalid diagram JSON: {err}")))
}

fn call_python_node(
    callback: &Arc<Py<PyAny>>,
    request: JsonMessage,
    config: JsonMessage,
) -> Result<JsonMessage, String> {
    Python::with_gil(|py| {
        let request = pythonize(py, &request).map_err(|err| err.to_string())?;
        let config = pythonize(py, &config).map_err(|err| err.to_string())?;
        let result = callback
            .bind(py)
            .call1((request, config))
            .map_err(|err| err.to_string())?;

        // Detect async callbacks that return coroutine objects and reject them
        // with a clear message instead of a confusing serialization error.
        let is_coroutine = py
            .import("inspect")
            .and_then(|m| m.call_method1("iscoroutine", (&result,)))
            .and_then(|r| r.extract::<bool>())
            .unwrap_or(false);
        if is_coroutine {
            let _ = result.call_method0("close");
            return Err("async callbacks are not supported; use a synchronous function".into());
        }

        depythonize(&result).map_err(|err| err.to_string())
    })
}

#[pyclass(unsendable)]
struct Executor {
    app: App,
    registry: DiagramElementRegistry,
}

impl Executor {
    fn new_inner() -> Self {
        let mut app = App::new();
        app.add_plugins(CrossflowExecutorApp::default());
        Self {
            app,
            registry: DiagramElementRegistry::new(),
        }
    }

    fn metadata_inner(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let metadata = serde_json::to_value(self.registry.metadata())
            .map_err(|err| runtime_error(format!("failed to serialize metadata: {err}")))?;
        json_to_python(py, &metadata)
    }

    fn register_node_inner(
        &mut self,
        name: &str,
        callback: Py<PyAny>,
        description: Option<String>,
    ) -> PyResult<()> {
        if self.registry.get_node_registration(name).is_ok() {
            return Err(value_error(format!(
                "node builder already registered: {name}"
            )));
        }

        let mut options = NodeBuilderOptions::new(Arc::<str>::from(name));
        if let Some(description) = description {
            options = options.with_description(description);
        }

        let callback = Arc::new(callback);
        self.registry
            .register_node_builder(options, move |builder, config: JsonMessage| {
                let callback = Arc::clone(&callback);
                builder.create_io_scope(move |scope, builder| {
                    let callback = Arc::clone(&callback);
                    let config = config.clone();
                    builder
                        .chain(scope.start)
                        .map_block(move |request: JsonMessage| {
                            call_python_node(&callback, request, config.clone())
                        })
                        .fork_result(|ok| ok.connect(scope.terminate), |err| err.then_cancel());
                })
            });

        Ok(())
    }

    /// Run a diagram workflow to completion. Note: the GIL is held for the
    /// entire execution because `App` is not `Send`. This means other Python
    /// threads are blocked while the workflow runs. Releasing the GIL would
    /// require `unsafe` since `App` does not implement `Ungil`.
    fn run_inner(
        &mut self,
        py: Python<'_>,
        diagram: &Bound<'_, PyAny>,
        request: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let diagram = diagram_from_python(diagram)?;
        let request = json_from_python(request)?;

        let (mut outcome, workflow) = self.app.world_mut().command(|cmds| {
            let workflow = diagram
                .spawn_io_workflow::<JsonMessage, JsonMessage>(cmds, &self.registry)
                .map_err(|err| value_error(format!("failed to build diagram workflow: {err}")))?;
            Ok::<_, pyo3::PyErr>((cmds.request(request, workflow).outcome(), workflow))
        })?;

        while outcome.is_pending() {
            self.app.update();
        }

        self.app.world_mut().despawn(workflow.provider());
        self.app.update();

        match outcome.try_recv() {
            Some(Ok(response)) => json_to_python(py, &response),
            Some(Err(err)) => Err(runtime_error(format!("workflow cancelled: {err}"))),
            None => Err(runtime_error("workflow did not produce an outcome")),
        }
    }
}

#[pymethods]
impl Executor {
    #[new]
    fn new() -> Self {
        Self::new_inner()
    }

    fn metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.metadata_inner(py)
    }

    #[pyo3(signature = (name, callback, *, description=None))]
    fn register_node(
        &mut self,
        name: &str,
        callback: Py<PyAny>,
        description: Option<String>,
    ) -> PyResult<()> {
        self.register_node_inner(name, callback, description)
    }

    fn run(
        &mut self,
        py: Python<'_>,
        diagram: &Bound<'_, PyAny>,
        request: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.run_inner(py, diagram, request)
    }
}

#[pymodule]
fn crossflow_python(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Executor>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::{
        ffi::c_str,
        types::{PyAnyMethods, PyDict, PyList, PyString},
    };
    use serde_json::json;

    fn sample_diagram(builder: &str, config: JsonMessage) -> JsonMessage {
        json!({
            "version": "0.1.0",
            "start": "callback",
            "ops": {
                "callback": {
                    "type": "node",
                    "builder": builder,
                    "config": config,
                    "next": { "builtin": "terminate" }
                }
            }
        })
    }

    #[test]
    fn metadata_returns_python_objects() {
        Python::with_gil(|py| {
            let executor = Executor::new_inner();
            let metadata = executor.metadata_inner(py).unwrap();
            assert!(metadata.bind(py).is_instance_of::<PyDict>());
        });
    }

    #[test]
    fn run_executes_a_basic_diagram() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(
                    c_str!("lambda request, config: request + config['delta']"),
                    None,
                    None,
                )
                .unwrap();
            executor
                .register_node_inner("add_delta", callback.unbind(), None)
                .unwrap();

            let diagram =
                json_to_python(py, &sample_diagram("add_delta", json!({ "delta": 3 }))).unwrap();
            let request = json_to_python(py, &json!(5)).unwrap();
            let result = executor
                .run_inner(py, diagram.bind(py), request.bind(py))
                .unwrap();

            assert_eq!(result.bind(py).extract::<i64>().unwrap(), 8);
        });
    }

    #[test]
    fn register_node_receives_request_and_config() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(
                    c_str!("lambda request, config: {'request': request, 'config': config}"),
                    None,
                    None,
                )
                .unwrap();
            executor
                .register_node_inner("echo_both", callback.unbind(), None)
                .unwrap();

            let diagram =
                json_to_python(py, &sample_diagram("echo_both", json!({ "mode": "x" }))).unwrap();
            let request = json_to_python(py, &json!({ "value": 7 })).unwrap();
            let result = executor
                .run_inner(py, diagram.bind(py), request.bind(py))
                .unwrap();
            let result_json: JsonMessage = depythonize(result.bind(py)).unwrap();

            assert_eq!(
                result_json,
                json!({
                    "request": { "value": 7 },
                    "config": { "mode": "x" }
                })
            );
        });
    }

    #[test]
    fn repeated_runs_succeed() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(c_str!("lambda request, config: request * 2"), None, None)
                .unwrap();
            executor
                .register_node_inner("double", callback.unbind(), None)
                .unwrap();

            let diagram = json_to_python(py, &sample_diagram("double", json!(null))).unwrap();
            for expected in [2_i64, 6_i64] {
                let request = json_to_python(py, &json!(expected / 2)).unwrap();
                let result = executor
                    .run_inner(py, diagram.bind(py), request.bind(py))
                    .unwrap();
                assert_eq!(result.bind(py).extract::<i64>().unwrap(), expected);
            }
        });
    }

    #[test]
    fn duplicate_registration_is_rejected() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(c_str!("lambda request, config: request"), None, None)
                .unwrap();
            executor
                .register_node_inner("echo", callback.unbind(), None)
                .unwrap();

            let duplicate = py
                .eval(c_str!("lambda request, config: request"), None, None)
                .unwrap();
            let err = executor
                .register_node_inner("echo", duplicate.unbind(), None)
                .unwrap_err();
            assert!(err.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn invalid_diagram_raises_value_error() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let diagram = PyList::empty(py);
            let request = json_to_python(py, &json!(1)).unwrap();
            let err = executor
                .run_inner(py, &diagram.into_any(), request.bind(py))
                .unwrap_err();
            assert!(err.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn callback_exception_raises_runtime_error() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(
                    c_str!("lambda request, config: (_ for _ in ()).throw(RuntimeError('boom'))"),
                    None,
                    None,
                )
                .unwrap();
            executor
                .register_node_inner("boom", callback.unbind(), None)
                .unwrap();

            let diagram = json_to_python(py, &sample_diagram("boom", json!(null))).unwrap();
            let request = json_to_python(py, &json!(1)).unwrap();
            let err = executor
                .run_inner(py, diagram.bind(py), request.bind(py))
                .unwrap_err();
            assert!(err.is_instance_of::<PyRuntimeError>(py));
            assert!(err.to_string().contains("boom"));
        });
    }

    #[test]
    fn run_accepts_string_diagram() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(c_str!("lambda request, config: request * 2"), None, None)
                .unwrap();
            executor
                .register_node_inner("double", callback.unbind(), None)
                .unwrap();

            let diagram_str =
                serde_json::to_string(&sample_diagram("double", json!(null))).unwrap();
            let diagram = PyString::new(py, &diagram_str);
            let request = json_to_python(py, &json!(5)).unwrap();
            let result = executor
                .run_inner(py, &diagram.into_any(), request.bind(py))
                .unwrap();

            assert_eq!(result.bind(py).extract::<i64>().unwrap(), 10);
        });
    }

    #[test]
    fn async_callback_raises_runtime_error() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let locals = PyDict::new(py);
            py.run(
                c_str!("async def _acb(request, config): return request"),
                None,
                Some(&locals),
            )
            .unwrap();
            let callback = locals.get_item("_acb").unwrap();
            executor
                .register_node_inner("async_node", callback.unbind(), None)
                .unwrap();

            let diagram = json_to_python(py, &sample_diagram("async_node", json!(null))).unwrap();
            let request = json_to_python(py, &json!(1)).unwrap();
            let err = executor
                .run_inner(py, diagram.bind(py), request.bind(py))
                .unwrap_err();
            assert!(err.is_instance_of::<PyRuntimeError>(py));
            assert!(err.to_string().contains("async"));
        });
    }

    #[test]
    fn non_json_callback_return_raises_runtime_error() {
        Python::with_gil(|py| {
            let mut executor = Executor::new_inner();
            let callback = py
                .eval(c_str!("lambda request, config: object()"), None, None)
                .unwrap();
            executor
                .register_node_inner("bad_return", callback.unbind(), None)
                .unwrap();

            let diagram = json_to_python(py, &sample_diagram("bad_return", json!(null))).unwrap();
            let request = json_to_python(py, &json!(1)).unwrap();
            let err = executor
                .run_inner(py, diagram.bind(py), request.bind(py))
                .unwrap_err();
            assert!(err.is_instance_of::<PyRuntimeError>(py));
            assert!(err.to_string().contains("object"));
        });
    }
}
