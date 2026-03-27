/*
 * Copyright (C) 2025 Open Source Robotics Foundation
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

// TODO(@mxgrey): Add module-level documentation describing how to use AnyBuffer

use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet, hash_map::Entry},
    ops::RangeBounds,
    sync::{Arc, Mutex, OnceLock},
};

use bevy_ecs::{
    prelude::{Commands, Entity, EntityRef, World},
    system::SystemState,
};

use thiserror::Error as ThisError;

use smallvec::SmallVec;

use crate::{
    AccessError, AccessKey, Accessing, Accessor, BMut, BMutTracer, Buffer, BufferAccessMut,
    BufferAccessors, BufferEntry, BufferError, BufferInstanceId, BufferKey, BufferKeyBody,
    BufferKeyBuilder, BufferKeyLifecycle, BufferKeyTag, BufferLocation, BufferManager, BufferMap,
    BufferMapLayout, BufferMapLayoutHints, BufferStorage, BufferView, BufferWorldAccess,
    Bufferable, Buffering, Builder, CloneFromBuffer, DrainBuffer, FetchFromBuffer, Gate, GateState,
    IdentifierRef, IncompatibleLayout, InspectBufferSessions, IterBufferView, Joining,
    ManageBufferSessions, MessageTypeHint, MessageTypeHintEvaluation, MessageTypeHintMap,
    NotifyBufferUpdate, OperationError, OperationResult, OperationRoster, OrBroken, RequestId, Seq,
    TypeInfo, add_listener_to_source,
};

#[cfg(feature = "trace")]
use crate::{BufferAccessRecord, BufferTracer};

/// A [`Buffer`] whose message type has been anonymized. Joining with this buffer
/// type will yield an [`AnyMessageBox`].
#[derive(Clone, Copy)]
pub struct AnyBuffer {
    pub(crate) location: BufferLocation,
    pub(crate) join_behavior: FetchBehavior,
    pub(crate) interface: &'static (dyn AnyBufferAccessInterface + Send + Sync),
}

impl AnyBuffer {
    /// Specify that you want this buffer to join by pulling an element. This is
    /// always supported.
    pub fn join_by_pulling(mut self) -> AnyBuffer {
        self.join_behavior = FetchBehavior::Pull;
        self
    }

    /// Specify that you want this buffer to join by cloning an element. This is
    /// only supported for underlying message types that support the [`Clone`]
    /// trait.
    ///
    /// If you are using the diagram workflow builder, make sure the message type
    /// stored by this buffer has registered its [`Clone`] trait.
    pub fn join_by_cloning(mut self) -> Option<AnyBuffer> {
        self.interface.clone_for_join_fn()?;
        self.join_behavior = FetchBehavior::Clone;
        Some(self)
    }

    /// The buffer ID for this key.
    pub fn id(&self) -> Entity {
        self.location.source
    }

    /// ID of the workflow that this buffer is associated with.
    pub fn scope(&self) -> Entity {
        self.location.scope
    }

    /// Get the type ID of the messages that this buffer supports.
    pub fn message_type_id(&self) -> TypeId {
        self.interface.message_type_id()
    }

    /// Get the type name of the messages that this buffer supports.
    pub fn message_type_name(&self) -> &'static str {
        self.interface.message_type_name()
    }

    /// Get the [`TypeInfo`] of this buffer's messages.
    pub fn message_type(&self) -> TypeInfo {
        TypeInfo {
            type_id: self.message_type_id(),
            type_name: self.message_type_name(),
        }
    }

    /// Get the [`AnyBufferAccessInterface`] for this specific instance of [`AnyBuffer`].
    pub fn get_interface(&self) -> &'static (dyn AnyBufferAccessInterface + Send + Sync) {
        self.interface
    }

    /// Get the [`AnyBufferAccessInterface`] for a concrete message type.
    pub fn interface_for<T: 'static + Send + Sync>()
    -> &'static (dyn AnyBufferAccessInterface + Send + Sync) {
        static INTERFACE_MAP: OnceLock<
            Mutex<HashMap<TypeId, &'static (dyn AnyBufferAccessInterface + Send + Sync)>>,
        > = OnceLock::new();
        let interfaces = INTERFACE_MAP.get_or_init(|| Mutex::default());

        // SAFETY: This will leak memory exactly once per type, so the leakage is bounded.
        // Leaking this allows the interface to be shared freely across all instances.
        let mut interfaces_mut = interfaces.lock().unwrap();
        *interfaces_mut
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::leak(Box::new(AnyBufferAccessImpl::<T>::new())))
    }
}

impl std::fmt::Debug for AnyBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyBuffer")
            .field("scope", &self.location.scope)
            .field("source", &self.location.source)
            .field("join_behavior", &self.join_behavior)
            .field("message_type_name", &self.interface.message_type_name())
            .finish()
    }
}

impl AnyBuffer {
    /// Downcast this into a concrete [`Buffer`] for the specified message type.
    ///
    /// To downcast into a specialized kind of buffer, use [`Self::downcast_buffer`] instead.
    pub fn downcast_for_message<Message: 'static>(&self) -> Option<Buffer<Message>> {
        if TypeId::of::<Message>() == self.interface.message_type_id() {
            Some(Buffer {
                location: self.location,
                _ignore: Default::default(),
            })
        } else {
            None
        }
    }

    /// Downcast this into a different special buffer representation, such as a
    /// `JsonBuffer`.
    pub fn downcast_buffer<BufferType: 'static>(&self) -> Option<BufferType> {
        self.interface.buffer_downcast(TypeId::of::<BufferType>())?(*self)
            .ok()?
            .downcast::<BufferType>()
            .ok()
            .map(|x| *x)
    }
}

impl<T: 'static + Send + Sync> From<Buffer<T>> for AnyBuffer {
    fn from(value: Buffer<T>) -> Self {
        let interface = AnyBuffer::interface_for::<T>();
        AnyBuffer {
            location: value.location,
            join_behavior: FetchBehavior::Pull,
            interface,
        }
    }
}

impl<T: 'static + Send + Sync + Clone> From<CloneFromBuffer<T>> for AnyBuffer {
    fn from(value: CloneFromBuffer<T>) -> Self {
        let interface = AnyBuffer::interface_for::<T>();
        AnyBuffer {
            location: value.location,
            join_behavior: FetchBehavior::Clone,
            interface,
        }
    }
}

/// What should the behavior be for this buffer when it gets joined? You can
/// make copies of the [`Buffer`] reference and give each copy a different behavior
/// so that it gets used differently for each join operation that it takes part in.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FetchBehavior {
    /// Pull a value from the buffer while joining
    #[default]
    Pull,
    /// Clone a value from the buffer while joining
    Clone,
}

/// A trait for turning a buffer into an [`AnyBuffer`]. It is expected that all
/// buffer types implement this trait.
pub trait AsAnyBuffer {
    /// Convert this buffer into an [`AnyBuffer`].
    fn as_any_buffer(&self) -> AnyBuffer;

    /// What would be the message type hint for this kind of buffer?
    fn message_type_hint() -> MessageTypeHint;
}

impl AsAnyBuffer for AnyBuffer {
    fn as_any_buffer(&self) -> AnyBuffer {
        *self
    }

    fn message_type_hint() -> MessageTypeHint {
        MessageTypeHint::fallback::<AnyMessageBox>()
    }
}

impl<T: 'static + Send + Sync> AsAnyBuffer for Buffer<T> {
    fn as_any_buffer(&self) -> AnyBuffer {
        (*self).into()
    }

    fn message_type_hint() -> MessageTypeHint {
        MessageTypeHint::exact::<T>()
    }
}

impl<T: 'static + Send + Sync + Clone> AsAnyBuffer for CloneFromBuffer<T> {
    fn as_any_buffer(&self) -> AnyBuffer {
        (*self).into()
    }

    fn message_type_hint() -> MessageTypeHint {
        MessageTypeHint::exact::<T>()
    }
}

/// Similar to a [`BufferKey`] except it can be used for any buffer without
/// knowing the buffer's message type at compile time.
///
/// This can key be used with a [`World`][1] to directly view or manipulate the
/// contents of a buffer through the [`AnyBufferWorldAccess`] interface.
///
/// [1]: bevy_ecs::prelude::World
#[derive(Clone)]
pub struct AnyBufferKey {
    pub(crate) body: BufferKeyBody,
    pub(crate) interface: &'static (dyn AnyBufferAccessInterface + Send + Sync),
}

impl AnyBufferKey {
    /// Downcast this into a concrete [`BufferKey`] for the specified message type.
    ///
    /// To downcast to a specialized kind of key, use [`Self::downcast_buffer_key`] instead.
    pub fn downcast_for_message<Message: 'static>(self) -> Option<BufferKey<Message>> {
        if TypeId::of::<Message>() == self.interface.message_type_id() {
            Some(BufferKey {
                body: self.body,
                _ignore: Default::default(),
            })
        } else {
            None
        }
    }

    /// Downcast this into a different special buffer key representation, such
    /// as a `JsonBufferKey`.
    pub fn downcast_buffer_key<KeyType: 'static>(self) -> Option<KeyType> {
        self.interface.key_downcast(TypeId::of::<KeyType>())?(self.body)
            .downcast::<KeyType>()
            .ok()
            .map(|x| *x)
    }

    /// The buffer ID of this key.
    pub fn id(&self) -> Entity {
        self.body.tag.buffer
    }

    /// The session that this key belongs to.
    pub fn session(&self) -> Entity {
        self.body.tag.session
    }

    pub fn tag(&self) -> &BufferKeyTag {
        &self.body.tag
    }
}

impl BufferKeyLifecycle for AnyBufferKey {
    type TargetBuffer = AnyBuffer;

    fn create_key(buffer: &AnyBuffer, builder: &mut BufferKeyBuilder) -> OperationResult<Self> {
        Ok(AnyBufferKey {
            body: builder.make_body(buffer.id())?,
            interface: buffer.interface,
        })
    }

    fn is_in_use(&self) -> bool {
        self.body.is_in_use()
    }

    fn deep_clone(&self) -> Self {
        Self {
            body: self.body.deep_clone(),
            interface: self.interface,
        }
    }
}

impl std::fmt::Debug for AnyBufferKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyBufferKey")
            .field("message_type_name", &self.interface.message_type_name())
            .field("body", &self.body)
            .finish()
    }
}

impl<T: 'static + Send + Sync + Any> From<BufferKey<T>> for AnyBufferKey {
    fn from(value: BufferKey<T>) -> Self {
        let interface = AnyBuffer::interface_for::<T>();
        AnyBufferKey {
            body: value.body,
            interface,
        }
    }
}

impl AccessKey for AnyBufferKey {
    fn validate_disjoint(&self, included: &mut HashMap<BufferInstanceId, usize>) -> bool {
        let entry = included.entry(self.tag().instance()).or_default();
        *entry += 1;
        *entry == 1
    }

    type State = Box<dyn AnyBufferAccessMutState>;
    type Param<'w, 's>
        = Box<dyn AnyBufferAccessMut<'w, 's> + 's>
    where
        'w: 's;

    fn get_state(&self, world: &mut World) -> Self::State {
        self.interface.create_any_buffer_access_mut_state(world)
    }

    fn get_param<'w, 's>(state: &'s mut Self::State, world: &'w mut World) -> Self::Param<'w, 's>
    where
        'w: 's,
    {
        state.get_any_buffer_access_mut(world)
    }

    fn get_mut<'w, 's, 'a>(
        &self,
        req: RequestId,
        param: &'a mut Self::Param<'w, 's>,
    ) -> Result<AnyBufferMut<'w, 's, 'a>, BufferError>
    where
        'w: 's,
        's: 'a,
    {
        param.as_any_buffer_mut(req, self)
    }

    fn apply_state(state: &mut Self::State, world: &mut World) {
        state.any_apply(world);
    }
}

impl Accessor for AnyBufferKey {
    type Buffers = AnyBuffer;

    async fn wait_for_change(&mut self) {
        let _ = self.body.receiver.changed().await;
    }

    type Seen = Seq;
    fn seen(&mut self, seen: Self::Seen) {
        if self.body.receiver.borrow_and_update().0 != seen {
            // Since the latest value is different from what the user last saw,
            // we'll mark this key as having not seen the latest value.
            self.body.receiver.mark_changed();
        }
    }

    fn make_seen(&self, world: &mut World) -> Self::Seen {
        world.get_buffer_seen(self.tag().instance())
    }

    fn is_disjoint(&self) -> Result<(), super::OverlapError> {
        // A single buffer key is always disjoint
        Ok(())
    }

    fn can_join(&self, world: &World) -> Result<bool, AccessError> {
        let view = world.any_buffer_view_untraced(self)?;
        Ok(view.oldest().is_some())
    }

    type Joined = AnyMessageBox;
    fn join(&self, req: RequestId, world: &mut World) -> Result<Option<Self::Joined>, AccessError> {
        Ok(world.any_buffer_mut(req, self, |mut buffer| buffer.pull())?)
    }

    fn distribute(
        &self,
        value: Self::Joined,
        req: RequestId,
        world: &mut World,
    ) -> Result<(), AccessError> {
        world.any_buffer_mut(req, self, |mut buffer| {
            let _ = buffer.push(value);
        })?;

        Ok(())
    }

    type View<'a> = AnyBufferView<'a>;
    fn view<'a>(
        &self,
        req: RequestId,
        world: &'a mut World,
    ) -> Result<Self::View<'a>, BufferError> {
        world.any_buffer_view(req, self)
    }

    fn view_untraced<'a>(&self, world: &'a World) -> Result<Self::View<'a>, BufferError> {
        world.any_buffer_view_untraced(self)
    }

    type Access<'w, 's, 'a> = AnyBufferMut<'w, 's, 'a>;
    fn access<U>(
        &self,
        req: RequestId,
        world: &mut World,
        f: impl FnOnce(AnyBufferMut) -> U,
    ) -> Result<U, AccessError> {
        Ok(world.any_buffer_mut(req, &self, f)?)
    }
}

impl BufferMapLayout for AnyBuffer {
    fn try_from_buffer_map(buffers: &BufferMap) -> Result<Self, IncompatibleLayout> {
        let mut compatibility = IncompatibleLayout::default();

        if let Ok(any_buffer) = compatibility.require_buffer_for_identifier::<AnyBuffer>(0, buffers)
        {
            return Ok(any_buffer);
        }

        Err(compatibility)
    }

    fn get_buffer_message_type_hints(
        identifiers: HashSet<IdentifierRef<'static>>,
    ) -> Result<MessageTypeHintMap, IncompatibleLayout> {
        let mut evaluation = MessageTypeHintEvaluation::new(identifiers);
        evaluation.fallback::<AnyMessageBox>(0);
        evaluation.evaluate()
    }

    fn get_layout_hints() -> BufferMapLayoutHints {
        BufferMapLayoutHints::Static(
            [(
                IdentifierRef::Index(0),
                MessageTypeHint::Fallback(TypeInfo::of::<AnyMessageBox>()),
            )]
            .into(),
        )
    }
}

/// Similar to [`BufferView`], but this can be unlocked with
/// an [`AnyBufferKey`], so it can work for any buffer whose message types
/// support serialization and deserialization.
#[derive(Clone)]
pub struct AnyBufferView<'a> {
    viewing: Arc<dyn AnyBufferViewing<'a> + 'a>,
    gate: &'a GateState,
    session: Entity,
}

impl<'a> AnyBufferView<'a> {
    /// Look at the oldest message in the buffer.
    pub fn oldest(&self) -> Option<AnyMessageRef<'a>> {
        self.viewing.any_oldest()
    }

    /// Look at the newest message in the buffer.
    pub fn newest(&self) -> Option<AnyMessageRef<'a>> {
        self.viewing.any_newest()
    }

    /// Borrow a message from the buffer. Index 0 is the oldest message in the buffer
    /// while the highest index is the newest message in the buffer.
    pub fn get(&self, index: usize) -> Option<AnyMessageRef<'a>> {
        self.viewing.any_get(index)
    }

    /// Iterate through the contents of this buffer.
    pub fn iter(&self) -> IterAnyBuffer<'a> {
        IterAnyBuffer {
            interface: self.viewing.any_iter(),
        }
    }

    /// Get how many messages are in this buffer.
    pub fn len(&self) -> usize {
        self.viewing.any_count()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check whether the gate of this buffer is open or closed.
    pub fn gate(&self) -> Gate {
        self.gate
            .map
            .get(&self.session)
            .copied()
            .unwrap_or(Gate::Open)
    }

    /// Get the type of the message stored in this buffer
    pub fn message_type(&self) -> TypeInfo {
        self.viewing.any_message_type()
    }
}

/// Similar to [`BufferMut`][crate::BufferMut], but this can be unlocked with an
/// [`AnyBufferKey`], so it can work for any buffer regardless of the data type
/// inside.
pub struct AnyBufferMut<'w, 's, 'a> {
    manager: Box<dyn AnyBufferManagement + 'a>,
    buffer: Entity,
    req: RequestId,
    session: Entity,
    accessor: Option<Entity>,
    // TODO(@mxgrey): We use a raw pointer here to escape an HRTB bug in the
    // Rust compiler: https://github.com/rust-lang/rust/issues/100013
    // When that issue is resolved we should try to revert this to a regular
    // safe borrow.
    commands: *mut Commands<'w, 's>,
    modified: bool,
}

impl<'w, 's, 'a> AnyBufferMut<'w, 's, 'a> {
    /// Same as [BufferMut::allow_closed_loops][1].
    ///
    /// [1]: crate::BufferMut::allow_closed_loops
    pub fn allow_closed_loops(mut self) -> Self {
        self.accessor = None;
        self
    }

    /// Look at the oldest message in the buffer.
    pub fn oldest(&self) -> Option<AnyMessageRef<'_>> {
        self.manager.any_oldest()
    }

    /// Look at the newest message in the buffer.
    pub fn newest(&self) -> Option<AnyMessageRef<'_>> {
        self.manager.any_newest()
    }

    /// Borrow a message from the buffer. Index 0 is the oldest message in the buffer
    /// while the highest index is the newest message in the buffer.
    pub fn get(&self, index: usize) -> Option<AnyMessageRef<'_>> {
        self.manager.any_get(index)
    }

    /// Iterate through the contents of this buffer.
    pub fn iter(&self) -> IterAnyBuffer<'_> {
        IterAnyBuffer {
            interface: self.manager.any_iter(),
        }
    }

    /// Get how many messages are in this buffer.
    pub fn len(&self) -> usize {
        self.manager.any_count()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Modify the oldest message in the buffer.
    pub fn oldest_mut(&mut self) -> Option<AnyMut<'_>> {
        self.modified = true;
        self.manager.any_oldest_mut()
    }

    /// Modify the newest message in the buffer.
    pub fn newest_mut(&mut self) -> Option<AnyMut<'_>> {
        self.modified = true;
        self.manager.any_newest_mut()
    }

    /// Modify a message in the buffer. Index 0 is the oldest message in the buffer
    /// with the highest index being the newest message in the buffer.
    pub fn get_mut(&mut self, index: usize) -> Option<AnyMut<'_>> {
        self.modified = true;
        self.manager.any_get_mut(index)
    }

    /// Drain a range of messages out of the buffer.
    pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) -> DrainAnyBuffer<'_> {
        self.modified = true;
        DrainAnyBuffer {
            interface: self.manager.any_drain(AnyRange::new(range)),
        }
    }

    /// Pull the oldest message from the buffer.
    pub fn pull(&mut self) -> Option<AnyMessageBox> {
        self.modified = true;
        self.manager.any_pull()
    }

    /// Pull the message that was most recently put into the buffer (instead of the
    /// oldest, which is what [`Self::pull`] gives).
    pub fn pull_newest(&mut self) -> Option<AnyMessageBox> {
        self.modified = true;
        self.manager.any_pull_newest()
    }

    /// Attempt to push a new value into the buffer.
    ///
    /// If the input value matches the message type of the buffer, this will
    /// return [`Ok`]. If the buffer is at its limit before a successful push, this
    /// will return the value that needed to be removed.
    ///
    /// If the input value does not match the message type of the buffer, this
    /// will return [`Err`] and give back the message that you tried to push.
    pub fn push<T: 'static + Send + Sync + Any>(&mut self, value: T) -> Result<Option<T>, T> {
        if TypeInfo::of::<T>() != self.manager.any_message_type() {
            return Err(value);
        }

        self.modified = true;

        // SAFETY: We checked that T matches the message type for this buffer,
        // so pushing and downcasting should not exhibit any errors.
        let removed = self
            .manager
            .any_push(Box::new(value))
            .unwrap()
            .map(|value| *value.downcast::<T>().unwrap());

        Ok(removed)
    }

    /// Attempt to push a new value of any message type into the buffer.
    ///
    /// If the input value matches the message type of the buffer, this will
    /// return [`Ok`]. If the buffer is at its limit before a successful push, this
    /// will return the value that needed to be removed.
    ///
    /// If the input value does not match the message type of the buffer, this
    /// will return [`Err`] and give back an error with the message that you
    /// tried to push and the type information for the expected message type.
    pub fn push_any(
        &mut self,
        value: AnyMessageBox,
    ) -> Result<Option<AnyMessageBox>, AnyMessageError> {
        self.manager.any_push(value)
    }

    /// Attempt to push a value into the buffer as if it is the oldest value of
    /// the buffer.
    ///
    /// The result follows the same rules as [`Self::push`].
    pub fn push_as_oldest<T: 'static + Send + Sync + Any>(
        &mut self,
        value: T,
    ) -> Result<Option<T>, T> {
        if TypeInfo::of::<T>() != self.manager.any_message_type() {
            return Err(value);
        }

        self.modified = true;

        // SAFETY: We checked that T matches the message type for this buffer,
        // so pushing and downcasting should not exhibit any errors.
        let removed = self
            .manager
            .any_push_as_oldest(Box::new(value))
            .unwrap()
            .map(|value| *value.downcast::<T>().unwrap());

        Ok(removed)
    }

    /// Attempt to push a value into the buffer as if it is the oldest value of
    /// the buffer.
    ///
    /// The result follows the same rules as [`Self::push_any`].
    pub fn push_any_as_oldest(
        &mut self,
        value: AnyMessageBox,
    ) -> Result<Option<AnyMessageBox>, AnyMessageError> {
        self.manager.any_push_as_oldest(value)
    }

    /// Trigger the listeners for this buffer to wake up even if nothing in the
    /// buffer has changed. This could be used for timers or timeout elements
    /// in a workflow.
    pub fn pulse(&mut self) {
        self.modified = true;
    }

    /// Get the type of the message stored in this buffer
    pub fn message_type(&self) -> TypeInfo {
        self.manager.any_message_type()
    }
}

impl<'w, 's, 'a> Drop for AnyBufferMut<'w, 's, 'a> {
    fn drop(&mut self) {
        if self.modified {
            // SAFETY: The commands pointer comes from a valid reference that
            // outlives this AnyBufferMut, so it is safe to dereference.
            unsafe { &mut *self.commands }.queue(NotifyBufferUpdate::new(
                self.buffer,
                self.req,
                self.session,
                self.accessor,
            ));
        }
    }
}

/// This trait allows [`World`] to give you access to any buffer using an
/// [`AnyBufferKey`].
pub trait AnyBufferWorldAccess {
    /// Call this to get read-only access to any buffer.
    ///
    /// For technical reasons this requires direct [`World`] access, but you can
    /// do other read-only queries on the world while holding onto the
    /// [`AnyBufferView`].
    ///
    /// It requires mutable world access because it traces access to the buffer
    /// if the tracing feature is enabled. If you want to view the buffer with
    /// non-mutable access, you can use `any_buffer_view_untraced`, but the
    /// viewing activity will not be traced.
    fn any_buffer_view<'a>(
        &'a mut self,
        req: RequestId,
        key: &AnyBufferKey,
    ) -> Result<AnyBufferView<'a>, BufferError>;

    /// Call this to get read-only access to any buffer.
    ///
    /// This buffer viewing will not be traced, which may be confusing if you
    /// review a log of the workflow activity.
    fn any_buffer_view_untraced<'a>(
        &'a self,
        key: &AnyBufferKey,
    ) -> Result<AnyBufferView<'a>, BufferError>;

    /// Call this to get mutable access to any buffer.
    ///
    /// Pass in a callback that will receive a [`AnyBufferMut`], allowing it to
    /// view and modify the contents of the buffer.
    fn any_buffer_mut<U>(
        &mut self,
        req: impl Into<RequestId>,
        key: &AnyBufferKey,
        f: impl FnOnce(AnyBufferMut) -> U,
    ) -> Result<U, BufferError>;
}

impl AnyBufferWorldAccess for World {
    fn any_buffer_view<'a>(
        &'a mut self,
        _req: RequestId,
        key: &AnyBufferKey,
    ) -> Result<AnyBufferView<'a>, BufferError> {
        #[cfg(feature = "trace")]
        {
            let mut tracer_state: SystemState<BufferTracer> = SystemState::new(self);
            let mut tracer = tracer_state.get_mut(self);
            tracer.trace(_req.into(), key.tag(), BufferAccessRecord::Viewed);
            tracer_state.apply(self);
        }

        key.interface.create_any_buffer_view(key, self)
    }

    fn any_buffer_view_untraced<'a>(
        &'a self,
        key: &AnyBufferKey,
    ) -> Result<AnyBufferView<'a>, BufferError> {
        key.interface.create_any_buffer_view(key, self)
    }

    fn any_buffer_mut<U>(
        &mut self,
        req: impl Into<RequestId>,
        key: &AnyBufferKey,
        f: impl FnOnce(AnyBufferMut) -> U,
    ) -> Result<U, BufferError> {
        let interface = key.interface;
        let mut state = interface.create_any_buffer_access_mut_state(self);
        let r = {
            let mut access = state.get_any_buffer_access_mut(self);
            let buffer_mut = access.as_any_buffer_mut(req.into(), key)?;
            f(buffer_mut)
        };

        state.any_apply(self);
        Ok(r)
    }
}

trait AnyBufferViewing<'a> {
    fn any_count(&self) -> usize;
    fn any_oldest(&self) -> Option<AnyMessageRef<'a>>;
    fn any_newest(&self) -> Option<AnyMessageRef<'a>>;
    fn any_get(&self, index: usize) -> Option<AnyMessageRef<'a>>;
    fn any_iter(&self) -> Box<dyn IterAnyBufferInterface<'a> + 'a>;
    fn any_message_type(&self) -> TypeInfo;
}

trait AnyBufferManagement {
    fn any_count(&self) -> usize;
    fn any_oldest(&self) -> Option<AnyMessageRef<'_>>;
    fn any_newest(&self) -> Option<AnyMessageRef<'_>>;
    fn any_get(&self, index: usize) -> Option<AnyMessageRef<'_>>;
    fn any_iter(&self) -> Box<dyn IterAnyBufferInterface<'_> + '_>;
    fn any_message_type(&self) -> TypeInfo;

    fn any_push(&mut self, value: AnyMessageBox) -> AnyMessagePushResult;
    fn any_push_as_oldest(&mut self, value: AnyMessageBox) -> AnyMessagePushResult;
    fn any_pull(&mut self) -> Option<AnyMessageBox>;
    fn any_pull_newest(&mut self) -> Option<AnyMessageBox>;
    fn any_oldest_mut(&mut self) -> Option<AnyMut<'_>>;
    fn any_newest_mut(&mut self) -> Option<AnyMut<'_>>;
    fn any_get_mut(&mut self, index: usize) -> Option<AnyMut<'_>>;
    fn any_drain(&mut self, range: AnyRange) -> Box<dyn DrainAnyBufferInterface + '_>;
}

pub(crate) struct AnyRange {
    start_bound: std::ops::Bound<usize>,
    end_bound: std::ops::Bound<usize>,
}

impl AnyRange {
    pub(crate) fn new<T: std::ops::RangeBounds<usize>>(range: T) -> Self {
        AnyRange {
            start_bound: deref_bound(range.start_bound()),
            end_bound: deref_bound(range.end_bound()),
        }
    }
}

fn deref_bound(bound: std::ops::Bound<&usize>) -> std::ops::Bound<usize> {
    match bound {
        std::ops::Bound::Included(v) => std::ops::Bound::Included(*v),
        std::ops::Bound::Excluded(v) => std::ops::Bound::Excluded(*v),
        std::ops::Bound::Unbounded => std::ops::Bound::Unbounded,
    }
}

impl std::ops::RangeBounds<usize> for AnyRange {
    fn start_bound(&self) -> std::ops::Bound<&usize> {
        self.start_bound.as_ref()
    }

    fn end_bound(&self) -> std::ops::Bound<&usize> {
        self.end_bound.as_ref()
    }

    fn contains<U>(&self, item: &U) -> bool
    where
        usize: PartialOrd<U>,
        U: ?Sized + PartialOrd<usize>,
    {
        match self.start_bound {
            std::ops::Bound::Excluded(lower) => {
                if *item <= lower {
                    return false;
                }
            }
            std::ops::Bound::Included(lower) => {
                if *item < lower {
                    return false;
                }
            }
            _ => {}
        }

        match self.end_bound {
            std::ops::Bound::Excluded(upper) => {
                if upper <= *item {
                    return false;
                }
            }
            std::ops::Bound::Included(upper) => {
                if upper < *item {
                    return false;
                }
            }
            _ => {}
        }

        return true;
    }
}

pub type AnyMessageRef<'a> = &'a (dyn Any + 'static + Send + Sync);

impl<'a, T: 'static + Send + Sync + Any> AnyBufferViewing<'a> for BufferView<'a, T> {
    fn any_count(&self) -> usize {
        self.len()
    }

    fn any_oldest(&self) -> Option<AnyMessageRef<'a>> {
        self.oldest().map(to_any_ref)
    }

    fn any_newest(&self) -> Option<AnyMessageRef<'a>> {
        self.newest().map(to_any_ref)
    }

    fn any_get(&self, index: usize) -> Option<AnyMessageRef<'a>> {
        self.get(index).map(to_any_ref)
    }

    fn any_iter(&self) -> Box<dyn IterAnyBufferInterface<'a> + 'a> {
        Box::new(self.iter())
    }

    fn any_message_type(&self) -> TypeInfo {
        TypeInfo::of::<T>()
    }
}

pub type AnyMessageMut<'a> = &'a mut (dyn Any + 'static + Send + Sync);
pub struct AnyMut<'a> {
    entry: &'a mut dyn AnyMutEntry,
    tracer: BMutTracer<'a>,
}

impl<'a> AnyMut<'a> {
    /// View the message inside the buffer via the [`Any`] interface. This will
    /// not be traced as a modification.
    pub fn get(&self) -> &dyn Any {
        self.entry.get_any()
    }

    /// Get mutable access to the message inside the buffer via the [`Any`]
    /// interface. This will be traced as a modification even if you do not change
    /// the value.
    pub fn get_mut(&mut self) -> &mut dyn Any {
        self.entry.get_any_mut(&self.tracer)
    }
}

impl<'a> std::ops::Deref for AnyMut<'a> {
    type Target = dyn Any;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<'a> std::ops::DerefMut for AnyMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

pub type AnyMessageBox = Box<dyn Any + 'static + Send + Sync>;

#[derive(ThisError, Debug)]
#[error("failed to convert a message")]
pub struct AnyMessageError {
    /// The original value provided
    pub value: AnyMessageBox,
    /// The ID of the type expected by the buffer
    pub type_id: TypeId,
    /// The name of the type expected by the buffer
    pub type_name: &'static str,
}

pub type AnyMessagePushResult = Result<Option<AnyMessageBox>, AnyMessageError>;

impl<T: 'static + Send + Sync + Any> AnyBufferManagement for BufferManager<'_, '_, '_, T> {
    fn any_count(&self) -> usize {
        self.len()
    }

    fn any_oldest(&self) -> Option<AnyMessageRef<'_>> {
        self.oldest().map(to_any_ref)
    }

    fn any_newest(&self) -> Option<AnyMessageRef<'_>> {
        self.newest().map(to_any_ref)
    }

    fn any_get(&self, index: usize) -> Option<AnyMessageRef<'_>> {
        self.get(index).map(to_any_ref)
    }

    fn any_iter(&self) -> Box<dyn IterAnyBufferInterface<'_> + '_> {
        Box::new(self.iter())
    }

    fn any_message_type(&self) -> TypeInfo {
        TypeInfo::of::<T>()
    }

    fn any_push(&mut self, value: AnyMessageBox) -> AnyMessagePushResult {
        let value = from_any_message::<T>(value)?;
        Ok(self.push(value).map(to_any_message))
    }

    fn any_push_as_oldest(&mut self, value: AnyMessageBox) -> AnyMessagePushResult {
        let value = from_any_message::<T>(value)?;
        Ok(self.push_as_oldest(value).map(to_any_message))
    }

    fn any_pull(&mut self) -> Option<AnyMessageBox> {
        self.pull().map(to_any_message)
    }

    fn any_pull_newest(&mut self) -> Option<AnyMessageBox> {
        self.pull_newest().map(to_any_message)
    }

    fn any_oldest_mut<'a>(&'a mut self) -> Option<AnyMut<'a>> {
        self.oldest_mut()
            .map(|BMut { entry, tracer }| AnyMut { entry, tracer })
    }

    fn any_newest_mut<'a>(&'a mut self) -> Option<AnyMut<'a>> {
        self.newest_mut()
            .map(|BMut { entry, tracer }| AnyMut { entry, tracer })
    }

    fn any_get_mut<'a>(&'a mut self, index: usize) -> Option<AnyMut<'a>> {
        self.get_mut(index)
            .map(|BMut { entry, tracer }| AnyMut { entry, tracer })
    }

    fn any_drain<'a>(&'a mut self, range: AnyRange) -> Box<dyn DrainAnyBufferInterface + 'a> {
        Box::new(self.drain(range))
    }
}

fn to_any_ref<'a, T: 'static + Send + Sync + Any>(x: &'a T) -> AnyMessageRef<'a> {
    x
}

pub(crate) fn to_any_message<T: 'static + Send + Sync + Any>(x: T) -> AnyMessageBox {
    Box::new(x)
}

fn from_any_message<T: 'static + Send + Sync + Any>(
    value: AnyMessageBox,
) -> Result<T, AnyMessageError>
where
    T: 'static,
{
    let value = value.downcast::<T>().map_err(|value| AnyMessageError {
        value,
        type_id: TypeId::of::<T>(),
        type_name: std::any::type_name::<T>(),
    })?;

    Ok(*value)
}

trait AnyMutEntry {
    fn get_any(&self) -> &(dyn Any + 'static + Send + Sync);
    fn get_any_mut(&mut self, tracer: &BMutTracer) -> &mut (dyn Any + 'static + Send + Sync);
}

impl<T: 'static + Send + Sync> AnyMutEntry for BufferEntry<T> {
    fn get_any(&self) -> &(dyn Any + 'static + Send + Sync) {
        &self.message
    }

    fn get_any_mut(&mut self, tracer: &BMutTracer) -> &mut (dyn Any + 'static + Send + Sync) {
        tracer.trace_mut(self);
        &mut self.message
    }
}

pub trait AnyBufferAccessMutState {
    fn get_any_buffer_access_mut<'s, 'w: 's>(
        &'s mut self,
        world: &'w mut World,
    ) -> Box<dyn AnyBufferAccessMut<'w, 's> + 's>;

    fn any_apply(&mut self, world: &mut World);
}

impl<T: 'static + Send + Sync + Any> AnyBufferAccessMutState
    for SystemState<BufferAccessMut<'_, '_, T>>
{
    fn get_any_buffer_access_mut<'s, 'w: 's>(
        &'s mut self,
        world: &'w mut World,
    ) -> Box<dyn AnyBufferAccessMut<'w, 's> + 's> {
        Box::new(self.get_mut(world))
    }

    fn any_apply(&mut self, world: &mut World) {
        self.apply(world);
    }
}

pub trait AnyBufferAccessMut<'w, 's> {
    fn as_any_buffer_mut<'a>(
        &'a mut self,
        req: RequestId,
        key: &AnyBufferKey,
    ) -> Result<AnyBufferMut<'w, 's, 'a>, BufferError>;
}

impl<'w, 's, T: 'static + Send + Sync + Any> AnyBufferAccessMut<'w, 's>
    for BufferAccessMut<'w, 's, T>
{
    fn as_any_buffer_mut<'a>(
        &'a mut self,
        req: RequestId,
        key: &AnyBufferKey,
    ) -> Result<AnyBufferMut<'w, 's, 'a>, BufferError> {
        let BufferAccessMut { inner, commands } = self;
        let tag = key.tag();
        let manager = inner.get_manager(req, tag)?;
        Ok(AnyBufferMut {
            manager: Box::new(manager),
            req,
            buffer: tag.buffer,
            session: tag.session,
            accessor: Some(tag.accessor),
            commands: commands as *mut _,
            modified: false,
        })
    }
}

pub trait AnyBufferAccessInterface {
    fn message_type_id(&self) -> TypeId;

    fn message_type_name(&self) -> &'static str;

    fn buffered_count(&self, entity: &EntityRef, session: Entity) -> Result<usize, OperationError>;

    fn ensure_session(&self, id: BufferInstanceId, world: &mut World) -> OperationResult;

    fn register_buffer_downcast(&self, buffer_type: TypeId, f: BufferDowncastBox);

    /// Allows AnyBuffer to support join_by_cloning
    fn register_cloning(
        &self,
        clone_for_any_join: CloneForAnyFn,
        clone_for_join_fn: &'static (dyn Any + Send + Sync),
    );

    fn buffer_downcast(&self, buffer_type: TypeId) -> Option<BufferDowncastRef>;

    fn register_key_downcast(&self, key_type: TypeId, f: KeyDowncastBox);

    fn key_downcast(&self, key_type: TypeId) -> Option<KeyDowncastRef>;

    fn pull(
        &self,
        req: RequestId,
        key: &BufferKeyTag,
        world: &mut World,
    ) -> Result<AnyMessageBox, OperationError>;

    fn clone_from_buffer(
        &self,
        req: RequestId,
        key: &BufferKeyTag,
        world: &mut World,
    ) -> Result<AnyMessageBox, OperationError>;

    fn clone_for_join_fn(&self) -> Option<&'static (dyn Any + Send + Sync)>;

    fn create_any_buffer_view<'a>(
        &self,
        key: &AnyBufferKey,
        world: &'a World,
    ) -> Result<AnyBufferView<'a>, BufferError>;

    fn create_any_buffer_access_mut_state(
        &self,
        world: &mut World,
    ) -> Box<dyn AnyBufferAccessMutState>;
}

pub type AnyMessageResult = Result<AnyMessageBox, OperationError>;
// TODO(@mxgrey): Consider changing this trait box into a function pointer
pub type BufferDowncastBox = Box<dyn Fn(AnyBuffer) -> AnyMessageResult + Send + Sync>;
pub type BufferDowncastRef = &'static (dyn Fn(AnyBuffer) -> AnyMessageResult + Send + Sync);
pub type KeyDowncastBox = Box<dyn Fn(BufferKeyBody) -> AnyMessageBox + Send + Sync>;
pub type KeyDowncastRef = &'static (dyn Fn(BufferKeyBody) -> AnyMessageBox + Send + Sync);
pub type CloneForAnyFn = fn(RequestId, &BufferKeyTag, &mut World) -> AnyMessageResult;

struct AnyBufferAccessImpl<T> {
    buffer_downcasts: Mutex<HashMap<TypeId, BufferDowncastRef>>,
    key_downcasts: Mutex<HashMap<TypeId, KeyDowncastRef>>,
    cloning: Mutex<Option<CloneInterfaces>>,
    _ignore: std::marker::PhantomData<fn(T)>,
}

struct CloneInterfaces {
    clone_for_any_join: CloneForAnyFn,
    /// Contains a function pointer that can be downcast into a type-specific
    /// fetch_for_join function pointer for [`FetchFromBuffer`].
    clone_for_join_fn: &'static (dyn Any + Send + Sync),
}

impl<T: 'static + Send + Sync> AnyBufferAccessImpl<T> {
    fn new() -> Self {
        let mut buffer_downcasts: HashMap<_, BufferDowncastRef> = HashMap::new();

        // SAFETY: These leaks are okay because we will only ever instantiate
        // AnyBufferAccessImpl once per generic argument T, which puts a firm
        // ceiling on how many of these callbacks will get leaked.

        // Automatically register a downcast into AnyBuffer
        buffer_downcasts.insert(
            TypeId::of::<AnyBuffer>(),
            Box::leak(Box::new(|buffer: AnyBuffer| -> AnyMessageResult {
                Ok(Box::new(AnyBuffer {
                    location: buffer.location,
                    join_behavior: buffer.join_behavior,
                    interface: AnyBuffer::interface_for::<T>(),
                }))
            })),
        );

        // Allow downcasting back to the original Buffer<T>
        buffer_downcasts.insert(
            TypeId::of::<Buffer<T>>(),
            Box::leak(Box::new(|buffer: AnyBuffer| -> AnyMessageResult {
                Ok(Box::new(Buffer::<T> {
                    location: buffer.location,
                    _ignore: Default::default(),
                }))
            })),
        );

        // Allow downcasting to the very general FetchFromBuffer type
        buffer_downcasts.insert(
            TypeId::of::<FetchFromBuffer<T>>(),
            Box::leak(Box::new(|buffer: AnyBuffer| -> AnyMessageResult {
                Ok(Box::new(FetchFromBuffer::<T>::try_from(buffer)?))
            })),
        );

        let mut key_downcasts: HashMap<_, KeyDowncastRef> = HashMap::new();

        // Automatically register a downcast to AnyBufferKey
        key_downcasts.insert(
            TypeId::of::<AnyBufferKey>(),
            Box::leak(Box::new(|body| -> AnyMessageBox {
                Box::new(AnyBufferKey {
                    body,
                    interface: AnyBuffer::interface_for::<T>(),
                })
            })),
        );

        Self {
            buffer_downcasts: Mutex::new(buffer_downcasts),
            key_downcasts: Mutex::new(key_downcasts),
            cloning: Default::default(),
            _ignore: Default::default(),
        }
    }
}

impl<T: 'static + Send + Sync + Any> AnyBufferAccessInterface for AnyBufferAccessImpl<T> {
    fn message_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn message_type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn buffered_count(&self, entity: &EntityRef, session: Entity) -> Result<usize, OperationError> {
        entity.buffered_count::<T>(session)
    }

    fn ensure_session(&self, id: BufferInstanceId, world: &mut World) -> OperationResult {
        world.ensure_buffer_session::<T>(id)
    }

    fn register_buffer_downcast(&self, buffer_type: TypeId, f: BufferDowncastBox) {
        let mut downcasts = self.buffer_downcasts.lock().unwrap();

        if let Entry::Vacant(entry) = downcasts.entry(buffer_type) {
            // SAFETY: We only leak this into the register once per type
            entry.insert(Box::leak(f));
        }
    }

    fn register_cloning(
        &self,
        clone_for_any_join: CloneForAnyFn,
        clone_for_join_fn: &'static (dyn Any + Send + Sync),
    ) {
        *self.cloning.lock().unwrap() = Some(CloneInterfaces {
            clone_for_any_join,
            clone_for_join_fn,
        });
    }

    fn buffer_downcast(&self, buffer_type: TypeId) -> Option<BufferDowncastRef> {
        self.buffer_downcasts
            .lock()
            .unwrap()
            .get(&buffer_type)
            .copied()
    }

    fn register_key_downcast(&self, key_type: TypeId, f: KeyDowncastBox) {
        let mut downcasts = self.key_downcasts.lock().unwrap();

        if let Entry::Vacant(entry) = downcasts.entry(key_type) {
            // SAFTY: We only leak this in to the register once per type
            entry.insert(Box::leak(f));
        }
    }

    fn key_downcast(&self, key_type: TypeId) -> Option<KeyDowncastRef> {
        self.key_downcasts.lock().unwrap().get(&key_type).copied()
    }

    fn pull(
        &self,
        req: RequestId,
        key: &BufferKeyTag,
        world: &mut World,
    ) -> Result<AnyMessageBox, OperationError> {
        world
            .unchecked_buffer_mut::<T, _>(req, key, |mut buffer| buffer.pull())
            .or_broken()?
            .map(to_any_message)
            .or_broken()
    }

    fn clone_from_buffer(
        &self,
        req: RequestId,
        key: &BufferKeyTag,
        world: &mut World,
    ) -> Result<AnyMessageBox, OperationError> {
        let f = self
            .cloning
            .lock()
            .unwrap()
            .as_ref()
            .or_broken()?
            .clone_for_any_join;
        f(req, key, world)
    }

    fn clone_for_join_fn(&self) -> Option<&'static (dyn Any + Send + Sync)> {
        self.cloning
            .lock()
            .unwrap()
            .as_ref()
            .map(|c| c.clone_for_join_fn)
    }

    fn create_any_buffer_view<'a>(
        &self,
        key: &AnyBufferKey,
        world: &'a World,
    ) -> Result<AnyBufferView<'a>, BufferError> {
        let buffer_ref = world.get_entity(key.tag().buffer)?;
        let storage = buffer_ref
            .get::<BufferStorage<T>>()
            .ok_or(BufferError::BufferStorageMissing)?;

        let gate = buffer_ref
            .get::<GateState>()
            .ok_or(BufferError::GateStorageMissing)?;

        Ok(AnyBufferView {
            viewing: Arc::new(BufferView::<T> {
                storage,
                session: key.tag().session,
            }),
            gate,
            session: key.tag().session,
        })
    }

    fn create_any_buffer_access_mut_state(
        &self,
        world: &mut World,
    ) -> Box<dyn AnyBufferAccessMutState> {
        Box::new(SystemState::<BufferAccessMut<T>>::new(world))
    }
}

pub struct IterAnyBuffer<'a> {
    interface: Box<dyn IterAnyBufferInterface<'a> + 'a>,
}

impl<'a> Iterator for IterAnyBuffer<'a> {
    type Item = AnyMessageRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.interface.any_next()
    }
}

trait IterAnyBufferInterface<'a> {
    fn any_next(&mut self) -> Option<AnyMessageRef<'a>>;
}

impl<'a, T: 'static + Send + Sync + Any> IterAnyBufferInterface<'a> for IterBufferView<'a, T> {
    fn any_next(&mut self) -> Option<AnyMessageRef<'a>> {
        self.next().map(to_any_ref)
    }
}

pub struct DrainAnyBuffer<'a> {
    interface: Box<dyn DrainAnyBufferInterface + 'a>,
}

impl<'a> Iterator for DrainAnyBuffer<'a> {
    type Item = AnyMessageBox;

    fn next(&mut self) -> Option<Self::Item> {
        self.interface.any_next()
    }
}

trait DrainAnyBufferInterface {
    fn any_next(&mut self) -> Option<AnyMessageBox>;
}

impl<'w, 's, T: 'static + Send + Sync + Any> DrainAnyBufferInterface
    for DrainBuffer<'w, 's, '_, T>
{
    fn any_next(&mut self) -> Option<AnyMessageBox> {
        self.next().map(to_any_message)
    }
}

impl Bufferable for AnyBuffer {
    type BufferType = Self;
    fn into_buffer(self, builder: &mut Builder) -> Self::BufferType {
        assert_eq!(self.scope(), builder.scope());
        self
    }
}

impl Buffering for AnyBuffer {
    fn verify_scope(&self, scope: Entity) {
        assert_eq!(scope, self.scope());
    }

    fn buffered_count(&self, session: Entity, world: &World) -> Result<usize, OperationError> {
        let entity_ref = world.get_entity(self.id()).or_broken()?;
        self.interface.buffered_count(&entity_ref, session)
    }

    fn buffered_count_for(
        &self,
        buffer: Entity,
        session: Entity,
        world: &World,
    ) -> Result<usize, OperationError> {
        if buffer != self.id() {
            return Ok(0);
        }

        self.buffered_count(session, world)
    }

    fn add_listener(&self, listener: Entity, world: &mut World) -> OperationResult {
        add_listener_to_source(self.id(), listener, world)
    }

    fn gate_action(
        &self,
        req: RequestId,
        session: Entity,
        action: Gate,
        world: &mut World,
        roster: &mut OperationRoster,
    ) -> OperationResult {
        GateState::apply(self.id(), req, session, action, world, roster)
    }

    fn as_input(&self) -> SmallVec<[Entity; 8]> {
        SmallVec::from_iter([self.id()])
    }

    fn ensure_active_session(&self, session: Entity, world: &mut World) -> OperationResult {
        self.interface.ensure_session(
            BufferInstanceId {
                buffer: self.id(),
                session,
            },
            world,
        )
    }
}

impl Joining for AnyBuffer {
    type Item = AnyMessageBox;
    fn fetch_for_join(
        &self,
        req: RequestId,
        session: Entity,
        world: &mut World,
    ) -> Result<Self::Item, OperationError> {
        let key = BufferKeyTag {
            buffer: self.id(),
            session,
            accessor: req.source,
        };
        match self.join_behavior {
            FetchBehavior::Pull => self.interface.pull(req, &key, world),
            FetchBehavior::Clone => self.interface.clone_from_buffer(req, &key, world),
        }
    }
}

impl Accessing for AnyBuffer {
    type Key = AnyBufferKey;
    fn add_accessor(&self, accessor: Entity, world: &mut World) -> OperationResult {
        world
            .get_mut::<BufferAccessors>(self.id())
            .or_broken()?
            .add_accessor(accessor);
        Ok(())
    }

    fn create_key(&self, builder: &mut super::BufferKeyBuilder) -> OperationResult<Self::Key> {
        Ok(AnyBufferKey {
            body: builder.make_body(self.id())?,
            interface: self.interface,
        })
    }

    fn deep_clone_key(key: &Self::Key) -> Self::Key {
        key.deep_clone()
    }

    fn is_key_in_use(key: &Self::Key) -> bool {
        key.is_in_use()
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, testing::*};
    use bevy_ecs::prelude::World;

    #[test]
    fn test_any_count() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffer = builder.create_buffer(BufferSettings::keep_all());
            let push_multiple_times = builder
                .commands()
                .spawn_service(push_multiple_times_into_buffer);
            let count = builder.commands().spawn_service(get_buffer_count);

            builder
                .chain(scope.start)
                .with_access(buffer)
                .then(push_multiple_times)
                .then(count)
                .connect(scope.terminate);
        });

        let count = context.resolve_request(1, workflow);
        assert_eq!(count, 5);
    }

    fn push_multiple_times_into_buffer(
        Blocking {
            request: (value, key),
            id,
            ..
        }: Blocking<(usize, BufferKey<usize>)>,
        mut access: BufferAccessMut<usize>,
    ) -> AnyBufferKey {
        let mut buffer = access.get_mut(id, &key).unwrap();
        for _ in 0..5 {
            buffer.push(value);
        }

        key.into()
    }

    fn get_buffer_count(
        Blocking {
            request: key, id, ..
        }: Blocking<AnyBufferKey>,
        world: &mut World,
    ) -> usize {
        world.any_buffer_view(id, &key).unwrap().len()
    }

    #[test]
    fn test_modify_any_message() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffer = builder.create_buffer(BufferSettings::keep_all());
            let push_multiple_times = builder
                .commands()
                .spawn_service(push_multiple_times_into_buffer);
            let modify_content = builder.commands().spawn_service(modify_buffer_content);
            let drain_content = builder.commands().spawn_service(pull_each_buffer_item);

            builder
                .chain(scope.start)
                .with_access(buffer)
                .then(push_multiple_times)
                .then(modify_content)
                .then(drain_content)
                .connect(scope.terminate);
        });

        let values = context.resolve_request(3, workflow);
        assert_eq!(values, vec![0, 3, 6, 9, 12]);
    }

    fn modify_buffer_content(
        Blocking {
            request: key, id, ..
        }: Blocking<AnyBufferKey>,
        world: &mut World,
    ) -> AnyBufferKey {
        world
            .any_buffer_mut(id, &key, |mut access| {
                for i in 0..access.len() {
                    access.get_mut(i).map(|mut value| {
                        *value.downcast_mut::<usize>().unwrap() *= i;
                    });
                }
            })
            .unwrap();

        key
    }

    fn pull_each_buffer_item(
        Blocking {
            request: key, id, ..
        }: Blocking<AnyBufferKey>,
        world: &mut World,
    ) -> Vec<usize> {
        world
            .any_buffer_mut(id, &key, |mut access| {
                let mut values = Vec::new();
                while let Some(value) = access.pull() {
                    values.push(*value.downcast::<usize>().unwrap());
                }
                values
            })
            .unwrap()
    }

    #[test]
    fn test_drain_any_message() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffer = builder.create_buffer(BufferSettings::keep_all());
            let push_multiple_times = builder
                .commands()
                .spawn_service(push_multiple_times_into_buffer);
            let modify_content = builder.commands().spawn_service(modify_buffer_content);
            let drain_content = builder.commands().spawn_service(drain_buffer_contents);

            builder
                .chain(scope.start)
                .with_access(buffer)
                .then(push_multiple_times)
                .then(modify_content)
                .then(drain_content)
                .connect(scope.terminate);
        });

        let values = context.resolve_request(3, workflow);
        assert_eq!(values, vec![0, 3, 6, 9, 12]);
    }

    fn drain_buffer_contents(
        Blocking {
            request: key, id, ..
        }: Blocking<AnyBufferKey>,
        world: &mut World,
    ) -> Vec<usize> {
        world
            .any_buffer_mut(id, &key, |mut access| {
                access
                    .drain(..)
                    .map(|value| *value.downcast::<usize>().unwrap())
                    .collect()
            })
            .unwrap()
    }

    #[test]
    fn double_any_messages() {
        let mut context = TestingContext::minimal_plugins();

        let workflow =
            context.spawn_io_workflow(|scope: Scope<(u32, i32, f32), (u32, i32, f32)>, builder| {
                let buffer_u32: AnyBuffer = builder
                    .create_buffer::<u32>(BufferSettings::default())
                    .into();
                let buffer_i32: AnyBuffer = builder
                    .create_buffer::<i32>(BufferSettings::default())
                    .into();
                let buffer_f32: AnyBuffer = builder
                    .create_buffer::<f32>(BufferSettings::default())
                    .into();

                let (input_u32, input_i32, input_f32) = builder.chain(scope.start).unzip();
                builder.chain(input_u32).map_block(|v| 2 * v).connect(
                    buffer_u32
                        .downcast_for_message::<u32>()
                        .unwrap()
                        .input_slot(),
                );

                builder.chain(input_i32).map_block(|v| 2 * v).connect(
                    buffer_i32
                        .downcast_for_message::<i32>()
                        .unwrap()
                        .input_slot(),
                );

                builder.chain(input_f32).map_block(|v| 2.0 * v).connect(
                    buffer_f32
                        .downcast_for_message::<f32>()
                        .unwrap()
                        .input_slot(),
                );

                (buffer_u32, buffer_i32, buffer_f32)
                    .join(builder)
                    .map_block(|(value_u32, value_i32, value_f32)| {
                        (
                            *value_u32.downcast::<u32>().unwrap(),
                            *value_i32.downcast::<i32>().unwrap(),
                            *value_f32.downcast::<f32>().unwrap(),
                        )
                    })
                    .connect(scope.terminate);
            });

        let r = context.resolve_request((1u32, 2i32, 3f32), workflow);
        let (v_u32, v_i32, v_f32) = r;
        assert_eq!(v_u32, 2);
        assert_eq!(v_i32, 4);
        assert_eq!(v_f32, 6.0);
    }
}
