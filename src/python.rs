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

#[pyo3::pymodule]
mod crossflow {
    use crate::{InnerChannel, JsonBufferKey};
    use std::{
        collections::HashMap,
        sync::Arc
    };
    use pyo3::prelude::*;

    #[derive(Clone)]
    #[pyclass(from_py_object)]
    struct PythonAccessor {
        accessors: HashMap<String, JsonBufferKey>,
    }

    #[derive(Clone)]
    #[pyclass(from_py_object)]
    struct PythonChannel {
        inner: Arc<InnerChannel>,
    }

    #[pymethods]
    impl PythonChannel {
        fn access(&self, accessor: PythonAccessor) -> PyResult<()> {
            Ok(())
        }
    }

    #[pyfunction]
    fn hello_crossflow() {
        println!("Hello, crossflow");
    }

    #[cfg(test)]
    mod tests {
        use pyo3::{
            prelude::*,
            types::PyDict,
        };

        #[test]
        fn test_running_script() {
            let python_script =
cr###"
a = 0
b = 1
c = 2

def foo(b, c):
    return b + c
"###;

            Python::attach(|py| {
                let globals = PyDict::new(py);
                let locals = PyDict::new(py);
                py.run(python_script, Some(&globals), Some(&locals)).unwrap();

                dbg!(&locals);

                let a_py = locals.get_item("a").unwrap().unwrap();
                let a: i64 = a_py.extract().unwrap();
                assert_eq!(a, 0);

                // let b_py = locals.get_item("b").unwrap().unwrap();
                let b_py = globals.get_item("b").unwrap().unwrap();
                let b: i64 = b_py.extract().unwrap();
                assert_eq!(b, 1);

                let foo_py = locals.get_item("foo").unwrap().unwrap();
                let foo: i64 = foo_py.call((), None).unwrap().extract().unwrap();
                assert_eq!(foo, 3);
            });
        }
    }
}
