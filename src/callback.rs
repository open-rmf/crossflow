/*
 * Copyright (C) 2024 Open Source Robotics Foundation
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
    AddOperation, Async, Blocking, Channel, ChannelQueue, Input, ManageDisposal, Seq, RequestId,
    ManageInput, OperateCallback, OperateTask, OperationError, OperationRoster,
    ProvideOnce, Provider, Sendish, StreamPack, UnusedStreams, MessageRoute, output_port,
    async_execution::{spawn_task, task_cancel_sender},
    make_stream_buffers_from_world,
};

use bevy_ecs::{
    prelude::{Commands, Entity, World},
    system::{BoxedSystem, IntoSystem},
};

use std::{
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
};

/// A Callback is an object that implements [`Provider`], similar to [`Service`](crate::Service),
/// except it is not associated with an [`Entity`]. Instead it can be passed around and
/// shared as its own object. Cloning a Callback will produce a new reference to the
/// same underlying instance. If the Callback has any internal state (e.g. [`Local`](bevy_ecs::prelude::Local)
/// parameters, change trackers, or mutable captured variables), that internal state will
/// be shared among all its clones.
///
/// There are three ways to instantiate a callback:
///
/// ## [`.as_callback()`](AsCallback)
///
/// If you have a Bevy system with an input parameter of `In<`[`BlockingCallback`]`>`
/// or `In<`[`AsyncCallback`]`>` then you can convert it into a [`Callback`]
/// object by applying `.as_callback()`.
///
/// ```rust
/// use crossflow::{prelude::*, testing::Integer};
/// use bevy_ecs::prelude::*;
///
/// fn add_integer(
///     In(input): In<BlockingCallback<i32>>,
///     integer: Res<Integer>,
/// ) -> i32 {
///     input.request + integer.value
/// }
///
/// let callback = add_integer.as_callback();
/// ```
///
/// ```rust
/// use crossflow::{prelude::*, testing::Integer};
/// use bevy_ecs::prelude::*;
/// use std::future::Future;
///
/// fn add_integer_async(
///     In(input): In<AsyncCallback<i32>>,
///     integer: Res<Integer>,
/// ) -> impl Future<Output = i32> + use<> {
///     let value = integer.value;
///     async move { input.request + value }
/// }
///
/// let async_callback = add_integer_async.as_callback();
/// ```
///
/// ## [`.into_blocking_callback()`](IntoBlockingCallback)
///
/// Any Bevy system can be converted into a blocking [`Callback`] by applying
/// `.into_blocking_callback()` to it. The `Request` type of the callback will
/// be whatever the input type of the system is (the `T` inside of `In<T>`). The
/// `Response` type of the callback will be whatever the return value of the
/// callback is.
///
/// A blocking callback is always run as an exclusive system (even if it does not
/// use exclusive system parameters), so it will block  all other systems from
/// running until it is finished.
///
/// ```rust
/// use crossflow::{prelude::*, testing::Integer};
/// use bevy_ecs::prelude::*;
///
/// fn add_integer(
///     In(input): In<i32>,
///     integer: Res<Integer>,
/// ) -> i32 {
///     input + integer.value
/// }
///
/// let callback = add_integer.into_blocking_callback();
/// ```
///
/// ## [`.into_async_callback()`](IntoAsyncCallback)
///
/// If you have a Bevy system whose return type implements the [`Future`] trait,
/// it can be converted into an async [`Callback`] object by applying
/// `.into_async_callback()` to it. The `Response` type of the callback will be
/// `<T as Future>::Output` where `T` is the return type of the system. The `Future`
/// returned by the system will be polled in the async compute task pool (unless
/// you activate the `single_threaded_async` feature).
///
/// ```rust
/// use crossflow::{prelude::*, testing::Integer};
/// use bevy_ecs::prelude::*;
/// use std::future::Future;
///
/// fn add_integer(
///     In(input): In<i32>,
///     integer: Res<Integer>,
/// ) -> impl Future<Output = i32> + use<> {
///     let value = integer.value;
///     async move { input + value }
/// }
///
/// let callback = add_integer.into_async_callback();
/// ```
pub struct Callback<Request, Response, Streams = ()> {
    pub(crate) inner: Arc<Mutex<InnerCallback<Request, Response, Streams>>>,
}

impl<Request, Response, Streams> Clone for Callback<Request, Response, Streams> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Request, Response, Streams> Callback<Request, Response, Streams> {
    pub fn new(callback: impl CallbackTrait<Request, Response, Streams> + 'static + Send) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerCallback {
                queue: VecDeque::new(),
                callback: Some(Box::new(callback)),
            })),
        }
    }
}

pub(crate) struct InnerCallback<Request, Response, Streams> {
    pub(crate) queue: VecDeque<PendingCallbackRequest>,
    pub(crate) callback:
        Option<Box<dyn CallbackTrait<Request, Response, Streams> + 'static + Send>>,
}

pub struct CallbackRequest<'a> {
    pub(crate) source: Entity,
    pub(crate) target: Entity,
    pub(crate) world: &'a mut World,
    pub(crate) roster: &'a mut OperationRoster,
}

impl<'a> CallbackRequest<'a> {
    fn get_request<Request: 'static + Send + Sync>(
        &mut self,
    ) -> Result<Input<Request>, OperationError> {
        self.world.take_input(self.source)
    }

    fn give_response<Response: 'static + Send + Sync>(
        &mut self,
        session: Entity,
        response: Response,
        seq: Seq,
        unused_streams: UnusedStreams,
    ) -> Result<(), OperationError> {
        let request_id = RequestId { session, source: self.source, seq };
        if !unused_streams.streams.is_empty() {
            let port = output_port::name_str("stream_out");
            let route = request_id.to_route_source(&port);
            self.world.emit_disposal(route, unused_streams.into(), self.roster);
        }

        let route = MessageRoute {
            session,
            source: self.source,
            seq,
            port: &output_port::next(),
            target: self.target,
        };
        self.world.give_input(route, response, self.roster)?;

        Ok(())
    }

    fn give_task<Task: Future + 'static + Sendish, Streams: StreamPack>(
        &mut self,
        session: Entity,
        seq: Seq,
        task: Task,
    ) -> Result<(), OperationError>
    where
        Task::Output: Send + Sync,
    {
        let sender = self
            .world
            .get_resource_or_insert_with(ChannelQueue::new)
            .sender
            .clone();
        let task = spawn_task(task, self.world);
        let task_id = self.world.spawn(()).id();

        let cancel_sender = task_cancel_sender(self.world);
        OperateTask::<_, Streams>::new(
            task_id,
            RequestId { session, source: self.source, seq },
            self.target,
            task,
            cancel_sender,
            None,
            sender,
        )
        .add(self.world, self.roster);
        Ok(())
    }

    fn get_channel<Streams: StreamPack>(
        &mut self,
        seq: Seq,
        session: Entity,
    ) -> Result<(Channel, Streams::StreamChannels), OperationError> {
        let sender = self
            .world
            .get_resource_or_insert_with(ChannelQueue::new)
            .sender
            .clone();
        let channel = Channel::new(RequestId { source: self.source, seq, session }, sender);
        let streams = channel.for_streams::<Streams>(self.world)?;
        Ok((channel, streams))
    }
}

pub struct PendingCallbackRequest {
    pub(crate) source: Entity,
    pub(crate) target: Entity,
}

impl PendingCallbackRequest {
    pub(crate) fn activate<'a>(
        self,
        world: &'a mut World,
        roster: &'a mut OperationRoster,
    ) -> CallbackRequest<'a> {
        CallbackRequest {
            source: self.source,
            target: self.target,
            world,
            roster,
        }
    }
}

pub trait CallbackTrait<Request, Response, Streams> {
    fn call(&mut self, request: CallbackRequest) -> Result<(), OperationError>;
}

pub struct BlockingCallbackMarker<M>(std::marker::PhantomData<fn(M)>);

struct BlockingCallbackSystem<Request, Response, Streams: StreamPack> {
    system: BoxedSystem<Blocking<Request, Streams>, Response>,
    initialized: bool,
}

impl<Request, Response, Streams> CallbackTrait<Request, Response, Streams>
    for BlockingCallbackSystem<Request, Response, Streams>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    fn call(&mut self, mut input: CallbackRequest) -> Result<(), OperationError> {
        let Input {
            session,
            data: request,
            seq,
        } = input.get_request()?;
        let source = input.source;
        let request_id = RequestId { source, seq, session };

        if !self.initialized {
            self.system.initialize(input.world);
            self.initialized = true;
        }

        let streams = make_stream_buffers_from_world::<Streams>(input.source, input.world)?;

        let response = self.system.run(
            Blocking {
                request,
                streams: streams.clone(),
                id: request_id,
            },
            input.world,
        );
        self.system.apply_deferred(input.world);

        let mut unused_streams = UnusedStreams::new(request_id);
        Streams::process_stream_buffers(
            streams,
            RequestId { session, source: input.source, seq },
            &mut unused_streams,
            input.world,
            input.roster,
        )?;

        input.give_response(session, response, seq, unused_streams)
    }
}

pub struct AsyncCallbackMarker<M>(std::marker::PhantomData<fn(M)>);

struct AsyncCallbackSystem<Request, Task, Streams: StreamPack> {
    system: BoxedSystem<Async<Request, Streams>, Task>,
    initialized: bool,
}

impl<Request, Task, Streams> CallbackTrait<Request, Task::Output, Streams>
    for AsyncCallbackSystem<Request, Task, Streams>
where
    Task: Future + 'static + Sendish,
    Request: 'static + Send + Sync,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
    fn call(&mut self, mut input: CallbackRequest) -> Result<(), OperationError> {
        let Input {
            session,
            data: request,
            seq,
        } = input.get_request()?;

        let (channel, streams) = input.get_channel::<Streams>(seq, session)?;

        if !self.initialized {
            self.system.initialize(input.world);
        }

        let task = self.system.run(
            Async {
                request,
                streams,
                channel,
                id: RequestId { source: input.source, seq, session },
            },
            input.world,
        );
        self.system.apply_deferred(input.world);

        input.give_task::<_, Streams>(session, seq, task)
    }
}

struct MapCallback<F: 'static + Send> {
    callback: F,
}

impl<Request, Response, Streams, F> CallbackTrait<Request, Response, Streams> for MapCallback<F>
where
    F: FnMut(CallbackRequest) -> Result<(), OperationError> + 'static + Send,
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: 'static + Send + Sync,
{
    fn call(&mut self, request: CallbackRequest) -> Result<(), OperationError> {
        (self.callback)(request)
    }
}

pub struct BlockingMapCallbackMarker<M>(std::marker::PhantomData<fn(M)>);
pub struct AsyncMapCallbackMarker<M>(std::marker::PhantomData<fn(M)>);

#[allow(clippy::wrong_self_convention)]
pub trait IntoCallback<M> {
    type Request;
    type Response;
    type Streams;
    fn into_callback(self) -> Callback<Self::Request, Self::Response, Self::Streams>;
}

impl<Request, Response, Streams, M, Sys>
    IntoCallback<BlockingCallbackMarker<(Request, Response, Streams, M)>> for Sys
where
    Sys: IntoSystem<Blocking<Request, Streams>, Response, M>,
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Response;
    type Streams = Streams;

    fn into_callback(self) -> Callback<Self::Request, Self::Response, Self::Streams> {
        Callback::new(BlockingCallbackSystem {
            system: Box::new(IntoSystem::into_system(self)),
            initialized: false,
        })
    }
}

impl<Request, Task, Streams, M, Sys> IntoCallback<AsyncCallbackMarker<(Request, Task, Streams, M)>>
    for Sys
where
    Sys: IntoSystem<Async<Request, Streams>, Task, M>,
    Task: Future + 'static + Sendish,
    Request: 'static + Send + Sync,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Task::Output;
    type Streams = Streams;

    fn into_callback(self) -> Callback<Self::Request, Self::Response, Self::Streams> {
        Callback::new(AsyncCallbackSystem {
            system: Box::new(IntoSystem::into_system(self)),
            initialized: false,
        })
    }
}

impl<Request, Response, Streams> ProvideOnce for Callback<Request, Response, Streams>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Response;
    type Streams = Streams;

    fn connect(
        self,
        scope: Option<Entity>,
        source: Entity,
        target: Entity,
        commands: &mut Commands,
    ) {
        commands.queue(AddOperation::new(
            scope,
            source,
            OperateCallback::new(self, target),
        ));
    }
}

impl<Request, Response, Streams> Provider for Callback<Request, Response, Streams>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
}
