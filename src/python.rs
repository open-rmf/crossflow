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
    use crate::{
        Channel, JsonBufferKey, IdentifierRef, AccessError, BufferError, OverlapError,
        JsonBufferMut, JsonMut, JsonRef, JsonMessage,
        Reply, format_vertical_list
    };
    use std::{
        borrow::Cow,
        collections::HashMap,
        sync::{Arc, Mutex, MutexGuard},
    };
    use futures::{
        future::Shared,
        FutureExt,
    };
    use pyo3::{
        prelude::*,
        types::{PySlice, PySliceIndices, PyNone, PyList, PyString, PyStringMethods},
        exceptions::{PyValueError, PyKeyError, PyIndexError, PyRuntimeError},
    };
    use pythonize::{depythonize, pythonize};

    impl From<BufferError> for PyErr {
        fn from(value: BufferError) -> Self {
            PyKeyError::new_err(format!("{value}"))
        }
    }

    impl From<OverlapError> for PyErr {
        fn from(value: OverlapError) -> Self {
            PyValueError::new_err(format!("{value}"))
        }
    }

    impl From<AccessError> for PyErr {
        fn from(value: AccessError) -> Self {
            match value {
                AccessError::NotDisjoint(overlap) => {
                    overlap.into()
                }
                AccessError::Inaccessible(error) => {
                    error.into()
                }
                AccessError::Multiple(multiple) => {
                    pyo3::exceptions::PyValueError::new_err(
                        format!(
                            "Multiple errors encountered:{}",
                            format_vertical_list(&multiple),
                        )
                    )
                }
            }
        }
    }

    #[derive(Clone)]
    #[pyclass(from_py_object)]
    struct PythonAccessor {
        accessors: Arc<HashMap<IdentifierRef<'static>, JsonBufferKey>>,
    }

    #[derive(Clone)]
    #[pyclass(from_py_object)]
    struct PythonChannel {
        channel: Arc<Channel>,
    }

    #[pymethods]
    impl PythonChannel {
        fn access(&self, accessor: PythonAccessor, callback: Py<PyAny>) -> PythonReply {
            let accessor_map = accessor.accessors.as_ref().clone();
            let reply = self.channel.access(accessor_map, move |access| {
                Python::attach(move |py| {
                    Arc::new(callback.call0(py))
                })
            })
            .shared();

            PythonReply { reply }
        }
    }

    /// This wraps the [`Reply`] struct so that Python scripts can await it.
    #[derive(Clone)]
    #[pyclass(from_py_object, name = "Reply")]
    struct PythonReply {
        reply: Shared<Reply<Result<Arc<PyResult<Py<PyAny>>>, AccessError>>>,
    }

    #[derive(Clone)]
    #[pyclass(from_py_object, name = "BufferAccess")]
    struct PythonBufferAccess {
        access: AccessMapRef,
        len: Option<isize>,
    }

    impl PythonBufferAccess {
        fn new(
            mutex: &BufferMutex,
            access_map: &mut HashMap<IdentifierRef<'static>, JsonBufferMut<'_, '_, '_>>,
        ) -> Self {
            let mut access = HashMap::new();
            let mut len = None;
            for (identifier, buffer) in access_map {
                let buffer_ptr: *mut JsonBufferMut<'static, 'static, 'static> = unsafe {
                    std::mem::transmute(buffer)
                };
                let buffer_mut = PythonBufferMut {
                    mutex: mutex.clone(),
                    buffer_ptr,
                };

                access.insert(identifier.clone(), buffer_mut);
                if let Some(index) = identifier.index() {
                    let index = index as isize;
                    if let Some(len) = &mut len {
                        if index > *len {
                            *len = index;
                        }
                    } else {
                        len = Some(index);
                    }
                }
            }

            if let Some(len) = &mut len {
                // Increment the highest index value by 1 to get the "length"
                // of this pseudo-list.
                *len += 1;
            }

            Self {
                access: AccessMapRef::new(access),
                len,
            }
        }
    }

    unsafe impl Send for PythonBufferAccess {}
    unsafe impl Sync for PythonBufferAccess {}

    #[pymethods]
    impl PythonBufferAccess {
        fn __len__(&self) -> PyResult<usize> {
            Ok(self.access.lock()?.len())
        }

        fn __getitem__(&self, py: Python, key: Bound<PyAny>) -> PyResult<Py<PyAny>> {
            if let Ok(name) = key.extract::<String>() {
                let identifier = IdentifierRef::Name(Cow::Owned(name));
                match self.get_item(&identifier)? {
                    Some(buffer) => {
                        return Ok(Py::new(py, buffer)?.into());
                    }
                    None => {
                        return Err(PyKeyError::new_err(
                            format!("name \"{}\" does not exist for this buffer access", identifier)
                        ))
                    }
                }
            }

            if let Ok(original_index) = key.extract::<isize>() {
                if let Some(len) = self.len {
                    return self.get_item_at_index(py, original_index, len);
                }

                Err(PyKeyError::new_err(
                    format!("cannot use index for buffer access that is not a list")
                ))?;
            }

            if let Ok(slice) = key.extract::<Bound<PySlice>>() {
                if let Some(len) = self.len {
                    let mut buffers = Vec::new();
                    for index in PySliceIterator::create(&slice, len)? {
                        buffers.push(self.get_item_at_index(py, index, len)?);
                    }

                    // Return a list when the user requests a slice of accessors.
                    return Ok(PyList::new(py, buffers)?.unbind().into());
                }

                Err(PyKeyError::new_err(
                    format!("cannot use slice for buffer access that is not a list")
                ))?;
            }

            Err(PyKeyError::new_err("unsupported key type - must provide a name, index, or slice").into())
        }

        fn __setitem__(&self, key: Bound<PyAny>, value: PythonBufferMut) -> PyResult<()> {
            if let Ok(name) = key.extract::<String>() {
                let identifier = IdentifierRef::Name(Cow::Owned(name));
                let mut access = self.access.lock()?;
                access.insert(identifier, value);
                return Ok(());
            }

            if let Ok(original_index) = key.extract::<isize>() {
                if original_index < 0 {
                    // The index is negative so we need to resolve it to the
                    // actual index before we can use it.
                    if let Some(len) = self.len {
                        let index = get_index(original_index, len)?;
                        let identifier = IdentifierRef::from_index(index as usize);
                        let mut access = self.access.lock()?;
                        access.insert(identifier, value);

                        return Ok(());
                    }

                    Err(PyKeyError::new_err(
                        format!("cannot use negative index for an buffer access that is not a list")
                    ))?;
                }

                let identifier = IdentifierRef::from_index(original_index as usize);
                let mut access = self.access.lock()?;
                access.insert(identifier, value);
                return Ok(());
            }

            Err(PyKeyError::new_err("unsupported key type - must provide a name or index").into())
        }
    }

    impl PythonBufferAccess {
        fn get_item_at_index(&self, py: Python, original_index: isize, len: isize) -> PyResult<Py<PyAny>> {
            let index = get_index(original_index, len)?;
            match self.get_item(&IdentifierRef::from_index(index as usize))? {
                Some(buffer) => {
                    return Ok(Py::new(py, buffer)?.into());
                }
                None => {
                    // The index is within the valid range, but there is
                    // no entry for this particular index. We will treat
                    // it as a deliberate gap in the list and return a
                    // None value.
                    return Ok(py_none(py));
                }
            }
        }

        fn get_item(&self, identifier: &IdentifierRef<'static>) -> PyResult<Option<PythonBufferMut>> {
            let access = self.access.lock()?;
            Ok(access.get(identifier).cloned())
        }
    }

    type AccessMap = HashMap<IdentifierRef<'static>, PythonBufferMut>;

    #[derive(Clone)]
    struct AccessMapRef(Arc<Mutex<AccessMap>>);

    impl AccessMapRef {
        fn new(map: AccessMap) -> Self {
            Self(Arc::new(Mutex::new(map)))
        }

        fn lock(&self) -> PyResult<MutexGuard<'_, AccessMap>> {
            self.0.lock().map_err(|err| {
                PyRuntimeError::new_err(format!("mutex poisoned: {err}")).into()
            })
        }
    }

    fn get_index(original_index: isize, len: isize) -> PyResult<isize> {
        let mut index = original_index;
        if index < 0 {
            // The user is asking for an item from the back of the
            // list instead of the front. We should add this negative
            // value onto the
            index = len + index;
        }

        if index < 0 || index >= len {
            Err(PyIndexError::new_err(
                format!("index {original_index} is outside the range of the list, len={len}")
            ))?;
        }

        Ok(index)
    }

    #[derive(Clone)]
    #[pyclass(from_py_object, name = "BufferMut")]
    struct PythonBufferMut {
        mutex: BufferMutex,
        buffer_ptr: *mut JsonBufferMut<'static, 'static, 'static>,
    }

    unsafe impl Send for PythonBufferMut {}
    unsafe impl Sync for PythonBufferMut {}

    #[pymethods]
    impl PythonBufferMut {
        fn __len__(&self) -> PyResult<usize> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &*self.buffer_ptr };
            let len = buffer.len();
            drop(lock);
            Ok(len)
        }

        fn __getitem__(&self, py: Python, key: Bound<PyAny>) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &*self.buffer_ptr };
            let len = buffer.len() as isize;

            if let Ok(original_index) = key.extract::<isize>() {
                return self.get_item(py, original_index);
            }

            if let Ok(slice) = key.extract::<Bound<PySlice>>() {
                let mut list = Vec::new();
                for index in PySliceIterator::create(&slice, len)? {
                    list.push(self.get_item(py, index)?);
                }

                return Ok(PyList::new(py, list)?.unbind().into());
            }

            drop(lock);
            Err(PyKeyError::new_err("unsupported key type - must provide an index or slice").into())
        }

        fn __setitem__(&self, original_index: isize, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &mut *self.buffer_ptr };
            let len = buffer.len() as isize;
            let index = get_index(original_index, len)? as usize;

            let Some(mut json) = buffer.get_mut(index) else {
                return Err(PyIndexError::new_err(
                    format!("index {original_index} is outside the range of the buffer, len={len}")
                ).into());
            };

            insert_json_value(&mut json, &value)?;

            drop(lock);
            Ok(())
        }

        #[getter(oldest)]
        fn get_oldest(&self, py: Python) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &*self.buffer_ptr };

            let Some(json) = buffer.oldest() else {
                return Ok(py_none(py));
            };
            let value = get_json_value(py, &json);

            drop(lock);
            value
        }

        #[setter(oldest)]
        fn set_oldest(&self, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &mut *self.buffer_ptr };

            if let Some(mut json) = buffer.oldest_mut() {
                return insert_json_value(&mut json, &value);
            }

            let value: JsonMessage = depythonize(&value)?;
            buffer.push_as_oldest(value).map_err(|err| {
                PyRuntimeError::new_err(
                    format!("unable to serialize input data: {err}")
                )
            })?;

            drop(lock);
            Ok(())
        }

        #[getter(newest)]
        fn get_newest(&self, py: Python) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &*self.buffer_ptr };

            let Some(json) = buffer.newest() else {
                return Ok(py_none(py));
            };
            let value = get_json_value(py, &json);

            drop(lock);
            value
        }

        #[setter(newest)]
        fn set_newest(&self, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &mut *self.buffer_ptr };

            if let Some(mut json) = buffer.newest_mut() {
                return insert_json_value(&mut json, &value);
            }

            let value: JsonMessage = depythonize(&value)?;
            buffer.push(value).map_err(|err| {
                PyRuntimeError::new_err(
                    format!("unable to serialize input data: {err}")
                )
            })?;

            drop(lock);
            Ok(())
        }

        #[pyo3(signature = (index, value = None))]
        fn get(&self, py: Python, index: isize, value: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            let buffer = unsafe { &mut *self.buffer_ptr };
            let len = buffer.len() as isize;

            let index = get_index(index, len)? as usize;
            let Some(json) = buffer.get(index) else {
                match value {
                    Some(value) => return Ok(value),
                    None => return Ok(py_none(py)),
                }
            };

            let data = get_json_value(py, &json)?;
            drop(lock);
            Ok(data)
        }
    }

    impl PythonBufferMut {
        fn get_item(&self, py: Python, original_index: isize) -> PyResult<Py<PyAny>> {
            let buffer = unsafe { &*self.buffer_ptr };
            let len = buffer.len() as isize;

            let index = get_index(original_index, len)? as usize;
            let Some(json) = buffer.get(index) else {
                return Err(PyIndexError::new_err(
                    format!("index {original_index} is outside the range of the buffer, len={len}")
                ).into());
            };

            get_json_value(py, &json)
        }
    }

    /// This is used to keep track of whether a PythonBufferAccessMap still has
    /// valid access to its buffers.
    #[derive(Clone)]
    struct BufferMutex {
        mutex: Arc<Mutex<bool>>,
    }

    struct BufferLocked<'a> {
        #[allow(unused)]
        guard: MutexGuard<'a, bool>,
    }

    impl BufferMutex {
        fn lock(&self) -> PyResult<BufferLocked<'_>> {
            let Some(guard) = self.mutex.lock().ok() else {
                return Err(PyRuntimeError::new_err("buffer access mutex is poisoned").into());
            };

            if *guard {
                Ok(BufferLocked { guard })
            } else {
                Err(PyRuntimeError::new_err("buffer access has expired").into())
            }
        }
    }

    fn py_none(py: Python) -> Py<PyAny> {
        PyNone::get(py).as_ref().clone_ref(py)
    }

    fn insert_json_value(json: &mut JsonMut, value: &Bound<PyAny>) -> PyResult<()> {
        json.insert(depythonize(value)?).map_err(|err| {
            PyRuntimeError::new_err(
                format!("unable to serialize input data: {err}")
            )
            .into()
        })
    }

    fn get_json_value(py: Python, json: &JsonRef) -> PyResult<Py<PyAny>> {
        let data = json.serialize().map_err(|err| {
            PyRuntimeError::new_err(
                format!("unable to serialize buffer data: {err}")
            )
        })?;

        Ok(pythonize(py, &data)?.unbind())
    }

    struct PySliceIterator {
        next: isize,
        indices: PySliceIndices,
    }

    impl PySliceIterator {
        fn create(slice: &Bound<PySlice>, len: isize) -> PyResult<Self> {
            let indices = slice.indices(len)?;
            if indices.step == 0 {
                return Err(PyValueError::new_err("slice step cannot be zero").into());
            }

            let next = get_index(indices.start, len)?;
            Ok(Self { next, indices })
        }
    }

    impl Iterator for PySliceIterator {
        type Item = isize;
        fn next(&mut self) -> Option<Self::Item> {
            if self.indices.step > 0 && self.next >= self.indices.stop {
                return None;
            }

            if self.indices.stop < 0 && self.next <= self.indices.stop {
                return None;
            }

            let next = self.next;
            self.next += self.indices.step;
            Some(next)
        }
    }

    #[cfg(test)]
    mod tests {
        use pyo3::{
            prelude::*,
            types::{PyDict, PySlice, PySliceMethods},
        };

        #[test]
        fn test_running_script() {
            let python_script =
cr###"
a = 0
b = 1
c = 2

s = slice(2, -1, -1)

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

                let b_py = locals.get_item("b").unwrap().unwrap();
                let b: i64 = b_py.extract().unwrap();
                assert_eq!(b, 1);

                let c_py = locals.get_item("c").unwrap().unwrap();
                let c: i64 = c_py.extract().unwrap();
                assert_eq!(c, 2);

                let s_py = locals.get_item("s").unwrap().unwrap();
                let s: Py<PySlice> = s_py.extract().unwrap();
                let indices = s.bind(py).indices(10).unwrap();
                assert_eq!(indices.start, 2);
                assert_eq!(indices.stop, 9);
                dbg!(indices);


                let foo_py = locals.get_item("foo").unwrap().unwrap();
                let foo: i64 = foo_py.call1((b, c)).unwrap().extract().unwrap();
                assert_eq!(foo, 3);
            });
        }
    }
}
