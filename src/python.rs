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
        JsonBufferMut, JsonMut, JsonRef, JsonMessage, format_vertical_list,
    };
    use std::{
        borrow::Cow,
        collections::HashMap,
        sync::{Arc, Mutex, MutexGuard, atomic::AtomicBool},
    };
    use futures::{
        future::Shared,
        FutureExt,
    };
    use tokio::sync::oneshot;
    use pyo3::{
        prelude::*,
        types::{PySlice, PySliceIndices, PyNone, PyList},
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

    /// A dictionary of buffer keys that grant access to buffers in the workflow.
    ///
    /// The key can be referenced by index or by name, depending on how the
    /// `listen` or `buffer_access` operation constructed it.
    #[derive(Clone)]
    #[pyclass(from_py_object, name = "Accessors")]
    pub struct PythonAccessors {
        accessors: Arc<HashMap<IdentifierRef<'static>, JsonBufferKey>>,
        channel: Arc<Channel>,
    }

    impl PythonAccessors {
        pub fn new(
            accessors: Arc<HashMap<IdentifierRef<'static>, JsonBufferKey>>,
            channel: Arc<Channel>,
        ) -> Self {
            Self { accessors, channel }
        }

        pub fn depythonize(self) -> HashMap<IdentifierRef<'static>, JsonBufferKey> {
            match Arc::try_unwrap(self.accessors) {
                Ok(accessors) => accessors,
                Err(this) => (*this).clone(),
            }
        }
    }

    #[pymethods]
    impl PythonAccessors {
        pub fn access(&self, callback: Py<PyAny>) -> PythonReply {
            let accessor_map = self.accessors.as_ref().clone();
            let reply = self.channel.access(accessor_map, move |mut access| {
                let r = Python::attach(move |py| {
                    let mutex = BufferMutex::new();
                    let py_access = PythonBufferAccess::new(&mutex, &mut access);
                    let r = callback.call(py, (py_access,), None);
                    mutex.close();
                    r
                });
                Arc::new(r)
            });
            let (future, detached) = reply.into_parts();
            let future = future.shared();

            PythonReply { future, detached }
        }
    }


    #[derive(Clone)]
    #[pyclass(from_py_object, name = "Accessor")]
    pub struct PythonAccessor {
        key: JsonBufferKey,
        channel: Arc<Channel>,
    }

    #[pymethods]
    impl PythonAccessor {
        pub fn access(&self, callback: Py<PyAny>) -> PythonReply {
            let reply = self.channel.access(self.key.clone(), move |access| {
                let r = Python::attach(move |py| {
                    let mutex = BufferMutex::new();
                    let buffer_ptr: *mut JsonBufferMut<'static, 'static, 'static> = unsafe {
                        std::mem::transmute(&access)
                    };
                    let py_buffer_mut = PythonBufferMut {
                        mutex: mutex.clone(),
                        buffer_ptr,
                    };
                    let r = callback.call(py, (py_buffer_mut,), None);
                    mutex.close();
                    r
                });
                Arc::new(r)
            });

            let (future, detached) = reply.into_parts();
            let future = future.shared();

            PythonReply { future, detached }
        }
    }

    /// Get the reply of a request sent to crossflow channel. Await this object
    /// to get the return value.
    ///
    /// Some commands sent to the channel might get cancelled if you drop this
    /// object before the command is finished. To prevent the drop from happening
    /// you can explicitl call `.detach()` on this object, and the command will
    /// continue until it finishes, no matter what you do with this Reply.
    #[derive(Clone)]
    #[pyclass(from_py_object, name = "Reply")]
    pub struct PythonReply {
        future: Shared<oneshot::Receiver<Result<Arc<PyResult<Py<PyAny>>>, AccessError>>>,
        detached: Arc<AtomicBool>,
    }

    #[pymethods]
    impl PythonReply {
        fn __await__(&self, py: Python) -> PyResult<Py<PyAny>> {
            let future = self.future.clone();
            pyo3_async_runtimes::async_std::future_into_py(py, async move {
                match future.await {
                    Ok(Ok(result)) => {
                        match result.as_ref() {
                            Ok(result) => {
                                Python::attach(|py| {
                                    Ok(result.clone_ref(py))
                                })
                            }
                            Err(err) => {
                                Python::attach(|py| {
                                    Err(err.clone_ref(py))
                                })
                            }
                        }
                    }
                    Ok(Err(err)) => {
                        return Err(PyRuntimeError::new_err(
                            format!("failed to access buffer: {err}")
                        ));
                    }
                    Err(err) => {
                        return Err(PyRuntimeError::new_err(
                            format!("unable to receive reply: {err}")
                        ));
                    }
                }
            })
            .map(|bound| bound.unbind())
        }

        fn detach(&self) -> PyResult<()> {
            self.detached.store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        }
    }

    #[derive(Clone)]
    #[pyclass(from_py_object, name = "BufferAccess")]
    pub struct PythonBufferAccess {
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

    #[derive(Clone)]
    #[pyclass(from_py_object, name = "Message")]
    pub struct PythonMessage {
        pub data: JsonMessage,
        pub accessors: Option<PythonAccessors>,
    }

    #[pymethods]
    impl PythonMessage {
        #[new]
        #[pyo3(signature = (data=None, accessors=None))]
        pub fn py_new(data: Option<&Bound<PyAny>>, accessors: Option<PythonAccessors>) -> PyResult<Self> {
            let data: JsonMessage = if let Some(data) = data {
                depythonize(data)?
            } else {
                JsonMessage::Null
            };

            Ok(Self { data, accessors })
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

        /// Whenever the values in a buffer are modified, listeners for that
        /// buffer will be notified that a change has occurred. If a listener
        /// of that buffer made that modification, there's a risk of an endlessly
        /// recurring loop within the workflow.
        ///
        /// In most cases, we expect this type of loop to be a mistake since a
        /// listener should not need to be notified about a change that it made
        /// itself. Therefore changes made via buffer access will not notify the
        /// listener whose key was used to make the change.
        ///
        /// There may be some cases where you do want the listener to be notified
        /// of a change caused by its own key, such as if the change was made by
        /// a downstream operation and the original listener needs to be made
        /// aware of that change. For those cases, you can enable closed loops
        /// here.
        fn enable_closed_loops(&self) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer valid is locked
            let buffer = unsafe { &mut *self.buffer_ptr };
            buffer.enable_closed_loops();
            drop(lock);
            Ok(())
        }

        /// Look at the "oldest" message in this buffer. This might not really
        /// be the oldest message since the value of the oldest message can be
        /// manipulated, but this is the message in the "oldest" position, which
        /// means it will be pulled first during a join operation.
        ///
        /// Making modifications to the object that you receive will not affect
        /// the data in the buffer. To change the value of the oldest message,
        /// use `set_oldest(_)`.
        fn get_oldest(&self, py: Python) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer valid is locked
            let buffer = unsafe { &*self.buffer_ptr };

            let Some(json) = buffer.oldest() else {
                return Ok(py_none(py));
            };
            let value = get_json_value(py, &json);

            drop(lock);
            value
        }

        /// Set the "oldest" message in this buffer to the specified value.
        ///
        /// If the buffer is empty, the value will be inserted and the buffer
        /// will be left with one entry. If the buffer already contained one or
        /// more messages, the "oldest" message will be replaced with this new
        /// value, and this new value will be considered the "oldest" message,
        /// even though it was newly introduced.
        fn set_oldest(&self, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer valid is locked
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

        /// Look at the "newest" message in this buffer. This might not really
        /// be the newest message since the value of the newest message can be
        /// manipulated, but this is the message in the "newest" position, which
        /// means it will be pulled first during a join operation.
        ///
        /// Making modifications to the object that you receive will not affect
        /// the data in the buffer. To change the value of the newest message,
        /// use `set_newest(_)`.
        fn get_newest(&self, py: Python) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer valid is locked
            let buffer = unsafe { &*self.buffer_ptr };

            let Some(json) = buffer.newest() else {
                return Ok(py_none(py));
            };
            let value = get_json_value(py, &json);

            drop(lock);
            value
        }

        /// Set the "newest" message in this buffer to the specified value.
        ///
        /// If the buffer is empty, the value will be inserted and the buffer
        /// will be left with one entry. If the buffer already contained one or
        /// more messages, the "newest" message will be replaced with this new
        /// value, and this new value will be considered the "newest" message,
        /// even though it was newly introduced.
        fn set_newest(&self, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer valid is locked
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

        /// Get the value at a certain position within the buffer. 0 is the
        /// oldest position, and `len(buffer) - 1` is the newest position.
        #[pyo3(signature = (index, value = None))]
        fn get(&self, py: Python, index: isize, value: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer valid is locked
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

        /// Pull the oldest value out of the buffer. By default this is what
        /// the join operation does. Used together with `push(_)`, you will get
        /// FIFO behavior from the buffer.
        fn pull(&self, py: Python) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer access valid is locked
            let buffer = unsafe { &mut *self.buffer_ptr };

            let data = match buffer.pull() {
                Some(Ok(data)) => data,
                Some(Err(err)) => {
                    return Err(PyRuntimeError::new_err(
                        format!("failed to deserialize message: {err}")
                    ));
                }
                None => {
                    return Ok(py_none(py));
                }
            };

            let data = pythonize(py, &data)?.unbind();

            drop(lock);
            Ok(data)
        }

        /// Pull the newest value out of the buffer. Used together with `push(_)`
        /// you will get LIFO behavior from the buffer.
        fn pull_newest(&self, py: Python) -> PyResult<Py<PyAny>> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer access valid is locked
            let buffer = unsafe { &mut *self.buffer_ptr };

            let data = match buffer.pull_newest() {
                Some(Ok(data)) => data,
                Some(Err(err)) => {
                    return Err(PyRuntimeError::new_err(
                        format!("failed to deserialize message: {err}")
                    ));
                }
                None => {
                    return Ok(py_none(py));
                }
            };

            let data = pythonize(py, &data)?.unbind();

            drop(lock);
            Ok(data)
        }

        /// Push a new value into the buffer. The new value will go to the
        /// "newest" message position.
        fn push(&self, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock()?;
            // SAFETY: The mutex that keeps this buffer access valid is locked
            let buffer = unsafe { &mut *self.buffer_ptr };

            let value: JsonMessage = depythonize(&value)?;
            if let Err(err) = buffer.push_json(value) {
                return Err(PyRuntimeError::new_err(
                    format!("failed to deserialize message: {err}")
                ));
            }

            drop(lock);
            Ok(())
        }

        /// Push a new value into the buffer, but put it at the "oldest" message
        /// position.
        fn push_as_oldest(&self, value: Bound<PyAny>) -> PyResult<()> {
            let lock = self.mutex.lock();
            // SAFETY: The mutex that keeps this buffer access valid is locked
            let buffer = unsafe { &mut *self.buffer_ptr };

            let value: JsonMessage = depythonize(&value)?;
            if let Err(err) = buffer.push_json_as_oldest(value) {
                return Err(PyRuntimeError::new_err(
                    format!("failed to deserialize message: {err}")
                ));
            }

            drop(lock);
            Ok(())
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

    impl BufferMutex {
        fn new() -> Self {
            Self {
                mutex: Arc::new(Mutex::new(true))
            }
        }

        fn close(&self) {
            let Ok(mut guard) = self.mutex.lock() else {
                return;
            };

            *guard = false;
        }
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

                let foo_py = py.eval(c"foo", Some(&globals), Some(&locals)).unwrap();
                let r: i64 = foo_py.call1((b, c)).unwrap().extract().unwrap();
                assert_eq!(r, 3);

                let run_script =
cr###"
def bar(a, b):
    return a * b;
"###;
                py.run(run_script, Some(&globals), Some(&locals)).unwrap();
                let (_, bar_py) = locals.iter().last().unwrap();
                let r: i64 = bar_py.call1((2, 3)).unwrap().extract().unwrap();
                assert_eq!(r, 6);

                let foo_py = py.eval(c"foo", Some(&globals), Some(&locals)).unwrap();
                let r: i64 = foo_py.call1((5, 6)).unwrap().extract().unwrap();
                assert_eq!(r, 11);
            });
        }
    }
}

pub use crate::python::crossflow::*;
