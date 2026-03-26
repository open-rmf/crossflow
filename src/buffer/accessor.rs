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

use std::{collections::HashMap, hash::Hash};

use thiserror::Error as ThisError;

use bevy_ecs::{
    prelude::{Entity, World},
    system::SystemState,
};

use crate::{
    Accessing, Buffer, BufferAccessMut, BufferError, BufferKey, BufferMap, BufferMapLayout,
    BufferMut, BufferView, BufferWorldAccess, Builder, Chain, IncompatibleLayout,
    ManageBufferSessions, Node, RequestId, Sendish, Seq, format_vertical_list,
};

use futures_concurrency::future::Race;

pub use crossflow_derive::{Accessor, Joined};

/// Trait to describe a set of buffer keys. This allows [listen][1] and [access][2]
/// to work for arbitrary structs of buffer keys. Structs with this trait can be
/// produced by [`try_listen`][3] and [`try_create_buffer_access`][4].
///
/// Each field in the struct must be some kind of buffer key.
///
/// This does not generally need to be implemented explicitly. Instead you should
/// define a struct where all fields are buffer keys and then apply
/// `#[derive(Accessor)]` to it, e.g.:
///
/// ```
/// use crossflow::prelude::*;
///
/// #[derive(Clone, Accessor)]
/// struct SomeKeys {
///     integer: BufferKey<i64>,
///     string: BufferKey<String>,
///     any: AnyBufferKey,
/// }
/// ```
///
/// The macro will generate a struct of buffers to match the keys. The name of
/// that struct is anonymous by default since you don't generally need to use it
/// directly, but if you want to give it a name you can use `#[accessor(buffers_struct_name = ...)]`:
///
/// ```
/// # use crossflow::prelude::*;
///
/// #[derive(Clone, Accessor)]
/// #[accessor(buffers_struct_name = SomeBuffers)]
/// struct SomeKeys {
///     integer: BufferKey<i64>,
///     string: BufferKey<String>,
///     any: AnyBufferKey,
/// }
/// ```
///
/// [1]: crate::Builder::listen
/// [2]: crate::Builder::create_buffer_access
/// [3]: crate::Builder::try_listen
/// [4]: crate::Builder::try_create_buffer_access
pub trait Accessor: 'static + Send + Sync + Sized + Clone {
    type Buffers: 'static + BufferMapLayout + Accessing<Key = Self> + Send + Sync;

    fn try_listen_from<'w, 's, 'a, 'b>(
        buffers: &BufferMap,
        builder: &'b mut Builder<'w, 's, 'a>,
    ) -> Result<Chain<'w, 's, 'a, 'b, Self>, IncompatibleLayout> {
        let buffers: Self::Buffers = Self::Buffers::try_from_buffer_map(buffers)?;
        Ok(buffers.listen(builder))
    }

    fn try_buffer_access<T: 'static + Send + Sync>(
        buffers: &BufferMap,
        builder: &mut Builder,
    ) -> Result<Node<T, (T, Self)>, IncompatibleLayout> {
        let buffers: Self::Buffers = Self::Buffers::try_from_buffer_map(buffers)?;
        Ok(buffers.access(builder))
    }

    /// Wait for a change to occur in any one of the buffer sessions that this
    /// accessor refers to.
    fn wait_for_change(&mut self) -> impl Future<Output = ()> + Sendish;

    /// A data structure used to indicate which versions of the buffers have
    /// been seen by this accessor.
    type Seen: 'static + Send + Sync;

    /// Mark the buffer as seen if the sequence value of its notifier is equal
    /// to this value. This can be used to reduce churn when doing async
    /// interactions with buffers.
    ///
    /// This is not something users will typically need to handle. The methods
    /// provided by [`crate::Channel`] will automatically take care of this.
    fn seen(&mut self, seen: Self::Seen);

    /// Make a Seen instance based on the current state of the world. This is
    /// used by the [`crate::Channel`] to update remote async keys.
    fn make_seen(&self, world: &mut World) -> Self::Seen;

    /// Check if this accessor is disjoint, meaning each key it contains accesses
    /// a unique buffer instance. Being a unique buffer instance means the
    /// combination of its buffer entity and session entity only appear once within
    /// this Accessor. The same buffer entity may appear multiple times for different
    /// sessions.
    ///
    /// For mutable access, including fetching, the accessor needs to be disjoint,
    /// so this needs to return true. Read-only access can be done with non-disjoint
    /// accessors.
    fn is_disjoint(&self) -> Result<(), OverlapError>;

    /// Check if the buffer is in a state that it's ready to be fetched from.
    fn can_join(&self, world: &World) -> Result<bool, AccessError>;

    type Joined: 'static + Send + Sync;
    /// Fetch a value from the buffer. For a normal [`BufferKey`] this will
    /// pull the oldest value out of the buffer.
    fn join(&self, req: RequestId, world: &mut World) -> Result<Option<Self::Joined>, AccessError>;

    /// Distribute a set of values to a set of buffers. This is the opposite of
    /// join: each value in the Joined struct will be pushed to the buffer in
    /// the accessor that corresponds to it.
    fn distribute(
        &self,
        value: Self::Joined,
        req: RequestId,
        world: &mut World,
    ) -> Result<(), AccessError>;

    type View<'a>;
    /// Get access to a view of the buffer.
    fn view<'a>(&self, req: RequestId, world: &'a mut World)
    -> Result<Self::View<'a>, BufferError>;

    /// Get access to a view of the buffer without tracing this access. This
    /// allows you to view with an immutable world borrow, but
    fn view_untraced<'a>(&self, world: &'a World) -> Result<Self::View<'a>, BufferError>;

    type Access<'w, 's, 'a>;
    /// Get mutable access to the buffers that this Accessor is associated with.
    fn access<U>(
        &self,
        req: RequestId,
        world: &mut World,
        f: impl FnOnce(Self::Access<'_, '_, '_>) -> U,
    ) -> Result<U, AccessError>;
}

#[derive(ThisError, Debug, Clone)]
pub enum AccessError {
    #[error("This accessor has keys that are not disjoint: {0}")]
    NotDisjoint(#[from] OverlapError),
    #[error("Failed to access one of the buffers: {0}")]
    Inaccessible(#[from] BufferError),
    #[error("Multiple access errors occurred:{}", format_vertical_list(.0))]
    Multiple(Vec<AccessError>),
}

impl AccessError {
    /// Turn a list of access errors into a Result. If the list is empty, this
    /// will return Ok(()). If there are multiple errors this will collect them
    /// into [`AccessError::Multiple`]. If there is only one error, it will be
    /// extracted and return as the error.
    pub fn from_list(mut errors: Vec<AccessError>) -> Result<(), AccessError> {
        if errors.len() > 1 {
            return Err(AccessError::Multiple(errors));
        }

        if let Some(error) = errors.pop() {
            // Only one error
            return Err(error);
        }

        Ok(())
    }
}

#[derive(ThisError, Debug, Clone)]
#[error("The accessor has duplicate buffer instances")]
pub struct OverlapError {
    /// Each buffer instance with more than one key trying to access it will be
    /// shown here.
    pub duplicates: HashMap<BufferInstanceId, usize>,
}

/// This trait represents a single buffer key whereas Accessor can be one key or
/// a collection of keys.
///
/// A single AccessKey is always itself an Accessor.
pub trait AccessKey: Accessor {
    /// Each key that this is called on should increment its entry in the map by
    /// one.
    fn validate_disjoint(&self, included: &mut HashMap<BufferInstanceId, usize>) -> bool;

    type State;
    type Param<'w, 's>
    where
        'w: 's;

    /// Get the SystemState used to access the buffer
    fn get_state(&self, world: &mut World) -> Self::State;

    /// Get the system parameter for accessing the buffer
    fn get_param<'w, 's>(state: &'s mut Self::State, world: &'w mut World) -> Self::Param<'w, 's>
    where
        'w: 's;

    /// Get the access interface that users interact with
    fn get_mut<'w, 's, 'a>(
        &self,
        req: RequestId,
        param: &'a mut Self::Param<'w, 's>,
    ) -> Result<Self::Access<'w, 's, 'a>, BufferError>
    where
        'w: 's,
        's: 'a;

    /// Apply the SystemState to the world. This is called after the access is
    /// finished.
    fn apply_state(state: &mut Self::State, world: &mut World);
}

/// Used by the Accessor trait to make sure an accessor with multiple keys
/// doesn't attempt to get mutable access to the same buffer instance multiple
/// times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferInstanceId {
    pub buffer: Entity,
    pub session: Entity,
}

impl<T> AccessKey for BufferKey<T>
where
    T: Send + Sync + 'static,
{
    fn validate_disjoint(&self, included: &mut HashMap<BufferInstanceId, usize>) -> bool {
        let entry = included.entry(self.tag().instance()).or_default();
        *entry += 1;
        *entry == 1
    }

    type State = SystemState<BufferAccessMut<'static, 'static, T>>;
    type Param<'w, 's>
        = BufferAccessMut<'w, 's, T>
    where
        'w: 's;

    fn get_state(&self, world: &mut World) -> Self::State {
        SystemState::<BufferAccessMut<T>>::new(world)
    }

    fn get_param<'w, 's>(state: &'s mut Self::State, world: &'w mut World) -> Self::Param<'w, 's>
    where
        'w: 's,
    {
        state.get_mut(world)
    }

    fn get_mut<'w, 's, 'a>(
        &self,
        req: RequestId,
        param: &'a mut Self::Param<'w, 's>,
    ) -> Result<Self::Access<'w, 's, 'a>, BufferError>
    where
        'w: 's,
        's: 'a,
    {
        Ok(param.get_mut(req, self)?)
    }

    fn apply_state(state: &mut Self::State, world: &mut World) {
        state.apply(world);
    }
}

impl<T> Accessor for BufferKey<T>
where
    T: Send + Sync + 'static,
{
    type Buffers = Buffer<T>;

    async fn wait_for_change(&mut self) {
        let _ = self.body.receiver.changed().await;
    }

    type Seen = Seq;
    fn seen(&mut self, seen: Seq) {
        if self.body.receiver.borrow_and_update().0 != seen {
            // Since the latest value is different from what the user last saw,
            // we'll mark this key as having not seen the latest value.
            self.body.receiver.mark_changed();
        }
    }

    fn make_seen(&self, world: &mut World) -> Self::Seen {
        world.get_buffer_seen(self.tag().instance())
    }

    fn is_disjoint(&self) -> Result<(), OverlapError> {
        // A single buffer key is always disjoint
        Ok(())
    }

    fn can_join(&self, world: &World) -> Result<bool, AccessError> {
        let view = world.buffer_view_untraced::<T>(self.tag())?;
        Ok(view.oldest().is_some())
    }

    type Joined = T;
    fn join(&self, req: RequestId, world: &mut World) -> Result<Option<Self::Joined>, AccessError> {
        Ok(world.buffer_mut(req, self, |mut buffer| buffer.pull())?)
    }

    fn distribute(
        &self,
        value: Self::Joined,
        req: RequestId,
        world: &mut World,
    ) -> Result<(), AccessError> {
        world.buffer_mut(req, self, move |mut buffer| {
            buffer.push(value);
        })?;
        Ok(())
    }

    type View<'a> = BufferView<'a, T>;
    fn view<'a>(
        &self,
        req: RequestId,
        world: &'a mut World,
    ) -> Result<Self::View<'a>, BufferError> {
        world.buffer_view(req, self)
    }

    fn view_untraced<'a>(&self, world: &'a World) -> Result<Self::View<'a>, BufferError> {
        world.buffer_view_untraced::<T>(self.tag())
    }

    type Access<'w, 's, 'a> = BufferMut<'w, 's, 'a, T>;

    fn access<U>(
        &self,
        req: RequestId,
        world: &mut World,
        f: impl FnOnce(BufferMut<T>) -> U,
    ) -> Result<U, AccessError> {
        Ok(world.buffer_mut(req, &self, f)?)
    }
}

impl<A: AccessKey> Accessor for Vec<A>
where
    Vec<A::Buffers>: 'static + BufferMapLayout + Accessing<Key = Vec<A>> + Send + Sync,
{
    type Buffers = Vec<A::Buffers>;

    async fn wait_for_change(&mut self) {
        let futures: Vec<_> = self.iter_mut().map(|a| a.wait_for_change()).collect();
        futures.race().await;
    }

    type Seen = Vec<A::Seen>;
    fn seen(&mut self, seen: Self::Seen) {
        for (key, seen) in self.iter_mut().zip(seen.into_iter()) {
            key.seen(seen);
        }
    }

    fn make_seen(&self, world: &mut World) -> Self::Seen {
        let mut seen = Vec::new();
        for key in self {
            seen.push(key.make_seen(world));
        }

        seen
    }

    fn is_disjoint(&self) -> Result<(), OverlapError> {
        let mut duplicates = HashMap::new();
        let mut is_disjoint = true;
        for key in self {
            is_disjoint &= key.validate_disjoint(&mut duplicates);
        }

        if !is_disjoint {
            duplicates.retain(|_, count| *count > 1);
            return Err(OverlapError { duplicates });
        }

        return Ok(());
    }

    fn can_join(&self, world: &World) -> Result<bool, AccessError> {
        self.is_disjoint()?;
        for key in self {
            if !key.can_join(world)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    type Joined = Vec<A::Joined>;
    fn join(&self, req: RequestId, world: &mut World) -> Result<Option<Self::Joined>, AccessError> {
        if !self.can_join(world)? {
            return Ok(None);
        }

        let mut errors = Vec::new();
        let mut fetched = Vec::new();
        for key in self {
            match key.join(req, world) {
                Ok(Some(value)) => fetched.push(value),
                Ok(None) => {
                    // Note: This can't happen unless there's a flaw in the
                    // implementation of can_join
                    return Ok(None);
                }
                Err(err) => errors.push(err),
            }
        }

        AccessError::from_list(errors)?;
        Ok(Some(fetched))
    }

    /// The values to be distributed will be zipped with the buffers in the Vec,
    /// and as many values will be distributed to as many buffers as possible.
    /// Fewer buffers than values means some values will not be pushed. Fewer
    /// values than buffers means some buffers will not receive anything.
    ///
    /// If any one of the buffers cannot be accessed, the value corresponding to
    /// it will be skipped and the rest of the buffers will still receive their
    /// values. The error that gets returned will reflect the last access error
    /// that was encountered.
    ///
    /// If this behavior is not appropriate for your use case, make sure to
    /// check that the number of values is equal to the number of buffers before
    /// calling this.
    fn distribute(
        &self,
        value: Self::Joined,
        req: RequestId,
        world: &mut World,
    ) -> Result<(), AccessError> {
        let mut errors = Vec::new();
        for (value, buffer) in value.into_iter().zip(self) {
            if let Err(err) = buffer.distribute(value, req, world) {
                errors.push(err);
            }
        }

        AccessError::from_list(errors)
    }

    type View<'a> = Vec<A::View<'a>>;
    fn view<'a>(
        &self,
        req: RequestId,
        world: &'a mut World,
    ) -> Result<Self::View<'a>, BufferError> {
        let mut view = Vec::new();
        let world_cell = world.as_unsafe_world_cell();
        for key in self {
            view.push(key.view(req, unsafe {
                // SAFETY: We require a &mut World as input to this function,
                // so we know that nothing else is interacting with the world
                // right now. We only need mutability for the tracing to be
                // performed. After that all access is read-only.
                world_cell.world_mut()
            })?);
        }

        Ok(view)
    }

    fn view_untraced<'a>(&self, world: &'a World) -> Result<Self::View<'a>, BufferError> {
        let mut view = Vec::new();
        for key in self {
            view.push(key.view_untraced(world)?);
        }

        Ok(view)
    }

    type Access<'w, 's, 'a> = Vec<A::Access<'w, 's, 'a>>;
    fn access<U>(
        &self,
        req: RequestId,
        world: &mut World,
        f: impl FnOnce(Vec<A::Access<'_, '_, '_>>) -> U,
    ) -> Result<U, AccessError> {
        self.is_disjoint()?;

        let mut states = Vec::new();
        let world_cell = world.as_unsafe_world_cell();
        for key in self {
            let state = key.get_state(unsafe {
                // SAFETY: We make sure the accessor is disjoint at the start
                // of the function. After that there is no overlap in the mutable
                // world access needed by the system states. Their commands will
                // be flushed serially at the end of this function.
                world_cell.world_mut()
            });
            states.push(state);
        }

        let r = {
            let mut params = Vec::new();
            for state in &mut states {
                let accessor = A::get_param(state, unsafe {
                    // SAFETY: Same rationale as earlier in this function.
                    world_cell.world_mut()
                });

                params.push(accessor);
            }

            let mut accesses = Vec::new();
            for (key, param) in self.iter().zip(params.iter_mut()) {
                let access = A::get_mut(key, req, param)?;
                accesses.push(access);
            }

            f(accesses)
        };

        for state in &mut states {
            A::apply_state(state, unsafe {
                // SAFETY: Same rationale as earlier in this function
                world_cell.world_mut()
            });
        }

        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, testing::*};

    #[derive(Clone, Accessor)]
    // #[accessor(buffers_struct_name = SameTypeBuffers)]
    #[accessor(joined_struct_name = SameTypeJoined)]
    struct SameTypeKeys<T: 'static + Send + Sync + Clone> {
        a: BufferKey<T>,
        b: BufferKey<T>,
        c: BufferKey<T>,
    }

    #[test]
    fn test_struct_accessor() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = SameTypeKeys::select_buffers(
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
            );

            builder
                .chain(scope.start)
                .with_access(buffers.a)
                .then(spread_into_buffer.into_callback())
                .with_access(buffers)
                .then(transfer_a_to_b.into_callback())
                .with_access(buffers)
                .then(transfer_b_to_c.into_callback())
                .with_access(buffers.c)
                .then(drain_buffer.into_callback())
                .connect(scope.terminate);
        });

        let values = context.resolve_request(vec![0, 1, 2, 3, 4, 5], workflow);
        assert_eq!(values, vec![0, 1, 2, 3, 4, 5]);
    }

    fn spread_into_buffer(
        Blocking {
            request: (values, key),
            id,
            ..
        }: Blocking<(Vec<i64>, BufferKey<i64>)>,
        world: &mut World,
    ) {
        world
            .buffer_mut(id, &key, move |mut buffer| {
                for value in values {
                    buffer.push(value);
                }
            })
            .unwrap();
    }

    fn transfer_a_to_b(
        Blocking {
            request: (_, keys),
            id,
            ..
        }: Blocking<((), SameTypeKeys<i64>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |mut access| {
            for value in access.a.drain(..) {
                access.b.push(value);
            }
        })
        .unwrap();
    }

    fn transfer_b_to_c(
        Blocking {
            request: (_, keys),
            id,
            ..
        }: Blocking<((), SameTypeKeys<i64>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |mut access| {
            for value in access.b.drain(..) {
                access.c.push(value);
            }
        })
        .unwrap();
    }

    fn drain_buffer(
        Blocking {
            request: (_, key),
            id,
            ..
        }: Blocking<((), BufferKey<i64>)>,
        world: &mut World,
    ) -> Vec<i64> {
        world
            .buffer_mut(id, &key, |mut buffer| buffer.drain(..).collect())
            .unwrap()
    }

    #[test]
    fn test_vec_accessor() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = vec![
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
            ];

            // - Insert the input value into buffer at index 1
            // - shift the buffer values until the input value is in the buffer at index 4
            // - drain the value from the buffer at index 4 and return it
            let shift_vec = shift_vec.into_callback();
            builder
                .chain(scope.start)
                .with_access(buffers[1])
                .then(spread_into_buffer.into_callback())
                .with_access(buffers.clone())
                .then(shift_vec.clone())
                .with_access(buffers.clone())
                .then(shift_vec.clone())
                .with_access(buffers.clone())
                .then(shift_vec)
                .with_access(buffers[4])
                .then(drain_buffer.into_callback())
                .connect(scope.terminate);
        });

        let values = context.resolve_request(vec![10], workflow);
        assert_eq!(values, vec![10]);
    }

    fn shift_vec(
        Blocking {
            request: (_, keys),
            id,
            ..
        }: Blocking<((), Vec<BufferKey<i64>>)>,
        world: &mut World,
    ) {
        world
            .buffers_mut(id, &keys, |access| {
                let mut previous_value = None;
                for mut buffer in access {
                    let next_value = buffer.pull();
                    if let Some(previous_value) = previous_value.take() {
                        buffer.push(previous_value);
                    }

                    previous_value = next_value;
                }
            })
            .unwrap();
    }

    #[test]
    fn test_accessor_vec_join() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = vec![
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
            ];

            builder
                .chain(scope.start)
                .with_access(buffers.clone())
                .then(clone_to_buffers.into_callback())
                .with_access(buffers)
                .then(join_from_buffers.into_callback())
                .connect(scope.terminate);
        });

        let values = context.resolve_request(5, workflow);
        assert_eq!(values, vec![5, 5, 5, 5, 5]);

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = vec![
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
            ];

            let shift_vec = shift_vec.into_callback();
            builder
                .chain(scope.start)
                .with_access(buffers[0])
                .then(spread_into_buffer.into_callback()) // filled to [0]
                .with_access(buffers.clone())
                .then(shift_vec.clone()) // filled to [1]
                .with_access(buffers.clone())
                .then(shift_vec.clone()) // filled to [2]
                .with_access(buffers.clone())
                .then(shift_vec.clone()) // filled to [3]
                .with_access(buffers.clone())
                .then(shift_vec.clone()) // filled to [4]
                .with_access(buffers)
                .then(join_from_buffers.into_callback())
                .connect(scope.terminate);
        });

        let values = context.resolve_request(vec![0, 1, 2, 3, 4, 5, 6, 7], workflow);
        assert_eq!(values, vec![4, 3, 2, 1, 0]);

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = vec![
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
                builder.create_buffer::<i64>(Default::default()),
            ];

            builder
                .chain(scope.start)
                .with_access(buffers.clone())
                .then(distribute_to_buffers.into_callback())
                .with_access(buffers)
                .then(join_from_buffers.into_callback())
                .connect(scope.terminate);
        });

        let values = context.resolve_request(vec![0, 1, 2, 3, 4], workflow);
        assert_eq!(values, vec![0, 1, 2, 3, 4]);
    }

    fn clone_to_buffers<T: 'static + Send + Sync + Clone>(
        Blocking {
            request: (value, keys),
            id,
            ..
        }: Blocking<(T, Vec<BufferKey<T>>)>,
        world: &mut World,
    ) {
        world
            .buffers_mut(id, &keys, |access| {
                for mut buffer in access {
                    buffer.push(value.clone());
                }
            })
            .unwrap();
    }

    fn join_from_buffers<A: Accessor>(
        Blocking {
            request: ((), keys),
            id,
            ..
        }: Blocking<((), A)>,
        world: &mut World,
    ) -> A::Joined {
        world.join_from_buffers(id, &keys).unwrap().unwrap()
    }

    fn distribute_to_buffers<A: Accessor>(
        Blocking {
            request: (value, keys),
            id,
            ..
        }: Blocking<(A::Joined, A)>,
        world: &mut World,
    ) {
        world.distribute_to_buffers(value, id, &keys).unwrap();
    }

    #[cfg(feature = "diagram")]
    mod json_tests {
        use super::*;
        use crate::AddBufferToMap;
        use std::collections::HashMap;

        #[derive(Clone, Accessor)]
        #[accessor(
            buffers_struct_name = TestBuffers,
            use_as_joined = TestJoined,
        )]
        struct TestKeys<T: 'static + Send + Sync + Clone> {
            integer: BufferKey<i64>,
            float: BufferKey<f64>,
            string: BufferKey<String>,
            generic: BufferKey<T>,
            json: JsonBufferKey,
        }

        #[derive(Clone)]
        struct TestJoined<T> {
            integer: i64,
            float: f64,
            string: String,
            generic: T,
            json: JsonMessage,
        }

        #[test]
        fn test_accessor_struct_join() {
            let mut context = TestingContext::minimal_plugins();

            let workflow = context.spawn_io_workflow(|scope, builder| {
                let buffers = TestKeys::select_buffers(
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer::<HashMap<String, String>>(Default::default()),
                );

                builder
                    .chain(scope.start)
                    .with_access(buffers.clone())
                    .then(distribute_to_buffers.into_callback())
                    .with_access(buffers)
                    .then(join_from_buffers.into_callback())
                    .connect(scope.terminate);
            });

            let values = TestJoined {
                integer: 7,
                float: 3.14159,
                string: String::from("hello"),
                generic: (2.171828, 4),
                json: serde_json::json!({
                    "hello": "json",
                }),
            };

            let resolved_values = context.resolve_request(values.clone(), workflow);
            assert_eq!(resolved_values.integer, values.integer);
            assert_eq!(resolved_values.float, values.float);
            assert_eq!(resolved_values.string, values.string);
            assert_eq!(resolved_values.generic, values.generic);
            assert_eq!(resolved_values.json, values.json);

            // This specifically tests that the Accessor macro correctly generates the
            // Joining trait impl needed for its Buffer struct to make the join operation for
            // its Joined struct.
            let workflow = context.spawn_io_workflow(|scope, builder| {
                let buffers = TestKeys::select_buffers(
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer::<HashMap<String, String>>(Default::default()),
                );

                builder
                    .chain(scope.start)
                    .with_access(buffers.clone())
                    .then(distribute_to_buffers.into_callback())
                    .unused();

                builder.join(buffers).connect(scope.terminate);
            });

            let resolved_values = context.resolve_request(values.clone(), workflow);
            assert_eq!(resolved_values.integer, values.integer);
            assert_eq!(resolved_values.float, values.float);
            assert_eq!(resolved_values.string, values.string);
            assert_eq!(resolved_values.generic, values.generic);
            assert_eq!(resolved_values.json, values.json);

            // This specifically tests that the Accessor macro correctly generates the
            // Joined trait impl needed for its Joined struct to make the try_join
            // operation.
            let workflow = context.spawn_io_workflow(|scope, builder| {
                let buffers = TestKeys::select_buffers(
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer(Default::default()),
                    builder.create_buffer::<HashMap<String, String>>(Default::default()),
                );

                let mut buffer_map = BufferMap::default();
                buffer_map.insert_buffer("integer", buffers.integer);
                buffer_map.insert_buffer("float", buffers.float);
                buffer_map.insert_buffer("string", buffers.string);
                buffer_map.insert_buffer("generic", buffers.generic);
                buffer_map.insert_buffer("json", buffers.json);

                builder
                    .chain(scope.start)
                    .with_access(buffers.clone())
                    .then(distribute_to_buffers.into_callback())
                    .unused();

                builder
                    .try_join::<TestJoined<(f64, i32)>>(&buffer_map)
                    .unwrap()
                    .connect(scope.terminate);
            });

            let resolved_values = context.resolve_request(values.clone(), workflow);
            assert_eq!(resolved_values.integer, values.integer);
            assert_eq!(resolved_values.float, values.float);
            assert_eq!(resolved_values.string, values.string);
            assert_eq!(resolved_values.generic, values.generic);
            assert_eq!(resolved_values.json, values.json);
        }
    }
}
