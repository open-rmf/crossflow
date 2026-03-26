/*
 * Copyright (C) 2023 Open Source Robotics Foundation
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

use bevy_ecs::{
    prelude::{Entity, Resource, World},
    system::Commands,
    world::CommandQueue,
};

use tokio::sync::{
    mpsc::{UnboundedReceiver as TokioReceiver, UnboundedSender as TokioSender, unbounded_channel},
    oneshot,
};

use std::sync::{Arc, Mutex, atomic::Ordering};

use crate::{
    AccessError, Accessor, BufferWorldAccess, MiscellaneousFailure, OperationError,
    OperationRoster, Outcome, Promise, ProvideOnce, Reply, RequestExt, RequestId, Seq, StreamPack,
    UnhandledErrors, async_execution::spawn_task,
};

use anyhow::anyhow;

/// Provides asynchronous access to the [`World`], allowing you to issue queries
/// or commands and then await the result.
#[derive(Clone)]
pub struct Channel {
    inner: Arc<InnerChannel>,
}

impl Channel {
    /// Get the outcome of sending a request to a provider.
    ///
    /// If you need to build a [`Series`] or do a full [`Capture`] then use
    /// [`Self::commands`] and use `.await` to dig into it. This is a convenience
    /// function for cases where all you need is the outcome.
    ///
    /// [`Series`]: crate::Series
    /// [`Capture`]: crate::Capture
    pub fn request_outcome<P>(&self, request: P::Request, provider: P) -> Outcome<P::Response>
    where
        P: ProvideOnce,
        P::Request: 'static + Send + Sync,
        P::Response: 'static + Send + Sync,
        P::Streams: 'static + StreamPack,
        P: 'static + Send + Sync,
    {
        let (outcome, capture) = Outcome::new();
        let _ = self
            .commands(move |commands| commands.request(request, provider).send_outcome(capture));
        outcome
    }

    /// Get mutable access to one or more buffers and then receive a reply.
    ///
    /// This must run asynchronously because access to the buffers requires
    /// exclusive world access.
    pub fn access<A: Accessor, U: 'static + Send>(
        &self,
        accessor: A,
        f: impl FnOnce(A::Access<'_, '_, '_>) -> U + 'static + Send,
    ) -> Reply<Result<U, AccessError>> {
        let req = self.inner.request_id;
        self.world(move |world| world.buffers_mut(req, &accessor, f))
    }

    /// This is a two-stage approach to accessing buffers:
    /// * `when` - The first stage tests whether some conditions for the buffers
    ///   and/or the world are being met. You get to view the buffers of this
    ///   accessor and also get to view the entire world. If the conditions that
    ///   you are waiting for are met, return Some(_). Otherwise return None to
    ///   keep waiting.
    /// * `then` - The first time that the first stage passes (returns Some(_)),
    ///   the value produced by the first stage will be passed to this stage,
    ///   along with mutable access to the buffers.
    ///
    /// `when` will be run each time there is a change in any of the relevant
    /// buffers, until its condition passes. It will not be run for any other
    /// world updates. Since it needs to be run multiple times, it needs `FnMut`.
    ///
    /// `then` will be run at most once, so it supports `FnOnce`.
    ///
    /// If you detach the [`Reply`] given by this function, then the callbacks
    /// will keep running until they finish. Otherwise the callbacks will stop
    /// trying after the [`Reply`] and its underlying receiver are dropped.
    #[must_use = "If the reply is dropped without being detached, the access callback might not be executed."]
    pub fn wait_for_access<A: Accessor, V: 'static, U: 'static + Send>(
        &self,
        accessor: A,
        mut when: impl FnMut(A::View<'_>, &World) -> Option<V> + 'static + Send,
        then: impl FnOnce(A::Access<'_, '_, '_>, V) -> U + 'static + Send,
    ) -> Reply<Result<U, AccessError>> {
        let req = self.inner.request_id;
        let mut then = Some(then);
        let f = {
            let accessor = accessor.clone();
            move |world: &mut World| -> Option<Result<U, AccessError>> {
                let view = match world.buffers_view_untraced(&accessor) {
                    Ok(view) => view,
                    Err(err) => return Some(Err(err.into())),
                };

                if let Some(v) = when(view, world) {
                    if let Some(then) = then.take() {
                        let f = move |access: A::Access<'_, '_, '_>| then(access, v);
                        return Some(world.buffers_mut(req, &accessor, f));
                    } else {
                        world
                            .get_resource_or_init::<UnhandledErrors>()
                            .miscellaneous
                            .push(MiscellaneousFailure {
                                error: Arc::new(anyhow!("Access callback gone before it was used")),
                                backtrace: Some(backtrace::Backtrace::new()),
                            });

                        // We don't want this to run forever, but there is no
                        // other fitting error to return. This shouldn't happen
                        // anyway.
                        return Some(Err(AccessError::Multiple(vec![])));
                    }
                }

                None
            }
        };

        self.wait_for(accessor, f)
    }

    /// Try to join values from the buffers of this accessor. If one or more of
    /// the buffers are not ready to join, then this will return `Ok(None)`.
    pub fn try_join<A: Accessor>(
        &self,
        accessor: A,
    ) -> Reply<Result<Option<A::Joined>, AccessError>> {
        let req = self.inner.request_id;
        self.world(move |world| accessor.join(req, world))
    }

    /// Keep trying to join until all the buffers are ready.
    #[must_use = "If the Reply is dropped without being detached, the join might not happen."]
    pub fn wait_for_join<A: Accessor>(&self, accessor: A) -> Reply<Result<A::Joined, AccessError>> {
        let f = {
            let req = self.inner.request_id;
            let accessor = accessor.clone();
            move |world: &mut World| -> Option<Result<A::Joined, AccessError>> {
                accessor.join(req, world).transpose()
            }
        };

        self.wait_for(accessor, f)
    }

    /// Distribute values to a set of buffers
    pub fn distribute<A: Accessor>(
        &self,
        accessor: A,
        values: A::Joined,
    ) -> Reply<Result<(), AccessError>> {
        let req = self.inner.request_id;
        self.world(move |world| accessor.distribute(values, req, world))
    }

    /// Run a query in the world and receive the promise of the query's output.
    #[deprecated(since = "0.0.6", note = "Use .request_outcome() instead")]
    pub fn query<P>(&self, request: P::Request, provider: P) -> Promise<P::Response>
    where
        P: ProvideOnce,
        P::Request: 'static + Send + Sync,
        P::Response: 'static + Send + Sync,
        P::Streams: 'static + StreamPack,
        P: 'static + Send + Sync,
    {
        #[allow(deprecated)]
        self.command(move |commands| commands.request(request, provider).take().response)
            .flatten()
    }

    /// Get access to a [`Commands`] for the [`World`].
    ///
    /// The commands will be carried out asynchronously. You can .await the
    /// receiver that this returns to know when the commands have been finished.
    pub fn commands<F, U>(&self, f: F) -> Reply<U>
    where
        F: FnOnce(&mut Commands) -> U + 'static + Send,
        U: 'static + Send,
    {
        let (sender, receiver) = oneshot::channel();
        self.inner
            .sender
            .send(Box::new(
                move |world: &mut World, _: &mut OperationRoster| {
                    let mut command_queue = CommandQueue::default();
                    let mut commands = Commands::new(&mut command_queue, world);
                    let u = f(&mut commands);
                    command_queue.apply(world);
                    let _ = sender.send(u);
                },
            ))
            .ok();

        Reply::new(receiver)
    }

    /// Get access to a [`Commands`] for the [`World`]
    #[deprecated(since = "0.0.6", note = "Use .commands() instead")]
    pub fn command<F, U>(&self, f: F) -> Promise<U>
    where
        F: FnOnce(&mut Commands) -> U + 'static + Send,
        U: 'static + Send,
    {
        let (sender, promise) = Promise::new();
        self.inner
            .sender
            .send(Box::new(
                move |world: &mut World, _: &mut OperationRoster| {
                    let mut command_queue = CommandQueue::default();
                    let mut commands = Commands::new(&mut command_queue, world);
                    let u = f(&mut commands);
                    command_queue.apply(world);
                    let _ = sender.send(u);
                },
            ))
            .ok();

        promise
    }

    /// Apply a closure onto the [`World`].
    ///
    /// The closure will be executed asynchronously. You can .await the receiver
    /// that this returns to know when the closure has been applied.
    pub fn world<F, U>(&self, f: F) -> Reply<U>
    where
        F: FnOnce(&mut World) -> U + 'static + Send,
        U: 'static + Send,
    {
        let (sender, receiver) = oneshot::channel();
        self.inner
            .sender
            .send(Box::new(
                move |world: &mut World, _: &mut OperationRoster| {
                    let u = f(world);
                    let _ = sender.send(u);
                },
            ))
            .ok();

        Reply::new(receiver)
    }

    /// Trigger a callback until it returns `Some(u)` then reply with the `u`.
    /// The callback will be triggered each time a change happens in any of the
    /// buffers included in the `dependencies` [`Accessor`].
    #[must_use = "If the Reply is dropped without being detached, the callback might not be triggered."]
    pub fn wait_for<A: Accessor, U: 'static + Send>(
        &self,
        dependencies: A,
        f: impl FnMut(&mut World) -> Option<U> + 'static + Send,
    ) -> Reply<U> {
        wait_for(self.inner.clone(), dependencies, f)
    }

    pub(crate) fn for_streams<Streams: StreamPack>(
        &self,
        world: &World,
    ) -> Result<Streams::StreamChannels, OperationError> {
        Ok(Streams::make_stream_channels(&self.inner, world))
    }

    pub(crate) fn new(request_id: RequestId, sender: TokioSender<ChannelItem>) -> Self {
        Self {
            inner: Arc::new(InnerChannel { request_id, sender }),
        }
    }
}

#[derive(Clone)]
pub struct InnerChannel {
    pub(crate) request_id: RequestId,
    pub(crate) sender: TokioSender<ChannelItem>,
}

impl InnerChannel {
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }

    pub fn source(&self) -> Entity {
        self.request_id.source
    }

    pub fn seq(&self) -> Seq {
        self.request_id.seq
    }

    pub fn sender(&self) -> &TokioSender<ChannelItem> {
        &self.sender
    }
}

pub(crate) type ChannelItem = Box<dyn FnOnce(&mut World, &mut OperationRoster) + Send>;
pub(crate) type ChannelSender = TokioSender<ChannelItem>;
pub(crate) type ChannelReceiver = TokioReceiver<ChannelItem>;

#[derive(Resource)]
pub(crate) struct ChannelQueue {
    pub(crate) sender: ChannelSender,
    pub(crate) receiver: ChannelReceiver,
}

impl ChannelQueue {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = unbounded_channel();
        Self { sender, receiver }
    }
}

impl Default for ChannelQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn wait_for<A: Accessor, U: 'static + Send>(
    channel: Arc<InnerChannel>,
    mut accessor: A,
    mut callback: impl FnMut(&mut World) -> Option<U> + 'static + Send,
) -> Reply<U> {
    let (sender, receiver) = oneshot::channel();
    let reply = Reply::new(receiver);
    let detached = reply.detached();
    let channel_clone = Arc::clone(&channel);
    let _ = channel_clone.sender.send(Box::new(
        move |world: &mut World, _: &mut OperationRoster| {
            if let Some(u) = callback(world) {
                let _ = sender.send(u);
                return;
            }

            let seen = accessor.make_seen(world);
            accessor.seen(seen);

            let shared_callback = Arc::new(Mutex::new(callback));
            let shared_sender = Arc::new(Mutex::new(Some(sender)));
            let _ = spawn_task(
                async move {
                    loop {
                        if detached.load(Ordering::Acquire) {
                            // The Reply is detached, so we can ignore responding to
                            // whether or not it's dropped.
                            let _ = accessor.wait_for_change();
                        } else {
                            let Ok(Some(mut sender)) = shared_sender.lock().map(|mut l| l.take())
                            else {
                                // For some reason the sender is no longer available.
                                // This suggests that the join was already performed
                                // somehow, even though that shouldn't be the case.
                                return;
                            };

                            tokio::select! {
                                _ = accessor.wait_for_change() => { },
                                _ = sender.closed() => {
                                    if !detached.load(Ordering::Acquire) {
                                        // The receiver was dropped without being
                                        // detached, so just return at this point.
                                        return;
                                    }
                                }
                            }

                            // Restore the sender to its shared mutex
                            match shared_sender.lock() {
                                Ok(mut shared_sender) => {
                                    *shared_sender = Some(sender);
                                }
                                Err(poison) => {
                                    *poison.into_inner() = Some(sender);
                                }
                            }
                            shared_sender.clear_poison();
                        }

                        let sender = shared_sender.clone();
                        let callback = shared_callback.clone();
                        let (seen_sender, seen_receiver) = oneshot::channel();
                        let a = accessor.clone();
                        let _ = channel.sender.send(Box::new(
                            move |world: &mut World, _: &mut OperationRoster| {
                                let Ok(mut callback) = callback.lock() else {
                                    return;
                                };

                                if let Some(u) = (*callback)(world) {
                                    // We have the value to return
                                    if let Ok(Some(sender)) = sender.lock().map(|mut l| l.take()) {
                                        let _ = sender.send(u);
                                    }
                                    return;
                                }

                                // The world was not ready, so update the waiting
                                // task with the new seen value.
                                let seen = a.make_seen(world);
                                let _ = seen_sender.send(seen);
                            },
                        ));

                        let Ok(seen) = seen_receiver.await else {
                            // We aren't receiving a new seen update, so that
                            // means the task is finished.
                            return;
                        };
                        accessor.seen(seen);
                    }
                },
                world,
            )
            .detach();
        },
    ));

    reply
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, testing::*};
    use bevy_ecs::system::EntityCommands;
    use std::time::Duration;

    #[test]
    fn test_channel_request() {
        let mut context = TestingContext::minimal_plugins();

        let (hello, repeat) = context.command(|commands| {
            let hello =
                commands.spawn_service(say_hello.with(|entity_cmds: &mut EntityCommands| {
                    entity_cmds.insert((
                        Salutation("Guten tag, ".into()),
                        Name("tester".into()),
                        RunCount(0),
                    ));
                }));
            let repeat =
                commands.spawn_service(repeat_service.with(|entity_cmds: &mut EntityCommands| {
                    entity_cmds.insert(RunCount(0));
                }));
            (hello, repeat)
        });

        for _ in 0..5 {
            context
                .try_resolve_request(
                    RepeatRequest {
                        service: hello,
                        count: 5,
                    },
                    repeat,
                    Duration::from_secs(5),
                )
                .unwrap();
        }

        let count = context
            .app
            .world()
            .get::<RunCount>(hello.provider())
            .unwrap()
            .0;
        assert_eq!(count, 25);

        let count = context
            .app
            .world()
            .get::<RunCount>(repeat.provider())
            .unwrap()
            .0;
        assert_eq!(count, 5);
    }

    #[test]
    fn test_channel_join() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = TestKeys::select_buffers(
                builder.create_buffer(Default::default()),
                builder.create_buffer(Default::default()),
                builder.create_buffer(Default::default()),
            );

            builder
                .chain(scope.start)
                .with_access(buffers.clone())
                .then(async_distribute_values.into_callback())
                .with_access(buffers.clone())
                .then(async_join_values.into_callback())
                .connect(scope.terminate);
        });

        let values = TestJoined {
            integer: 9,
            float: 2.71828,
            string: String::from("hi"),
        };

        let result = context.resolve_request(values.clone(), workflow);
        assert_eq!(result.integer, values.integer);
        assert_eq!(result.float, values.float);
        assert_eq!(result.string, values.string);

        let delay = context.spawn_delay(Duration::from_secs_f32(0.05));

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = TestKeys::select_buffers(
                builder.create_buffer(Default::default()),
                builder.create_buffer(Default::default()),
                builder.create_buffer(Default::default()),
            );

            let (values, trigger) = builder
                .chain(scope.start)
                .map_block(|values| (values, ()))
                .unzip();

            builder
                .chain(values)
                .then(delay)
                .with_access(buffers.clone())
                .then(async_distribute_values.into_callback())
                .unused();

            builder
                .chain(trigger)
                .with_access(buffers.clone())
                .then(async_wait_for_join_values.into_callback())
                .connect(scope.terminate);
        });

        let result = context.resolve_request(values.clone(), workflow);
        assert_eq!(result.integer, values.integer);
        assert_eq!(result.float, values.float);
        assert_eq!(result.string, values.string);
    }

    #[test]
    fn test_wait_for_access() {
        let mut context = TestingContext::minimal_plugins();

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = TestKeys::select_buffers(
                builder.create_buffer(Default::default()),
                builder.create_buffer(Default::default()),
                builder.create_buffer(Default::default()),
            );

            let (integers, floats, string) = builder.chain(scope.start).unzip();
            let integers_node = builder
                .chain(integers)
                .then_node(slowly_stream_values.into_callback());
            builder.connect(integers_node.streams, buffers.integer.input_slot());

            let floats_node = builder
                .chain(floats)
                .then_node(slowly_stream_values.into_callback());
            builder.connect(floats_node.streams, buffers.float.input_slot());

            builder
                .chain(string)
                .with_access(buffers)
                .map(
                    |Async {
                         request: (string, keys),
                         channel,
                         ..
                     }: Async<_>| {
                        async move {
                            channel
                                .wait_for_access(
                                    keys,
                                    |view: TestView<'_>, _| {
                                        if let (Some(i), Some(f)) =
                                            (view.integer.newest(), view.float.newest())
                                        {
                                            if *i > 2 && *f > 2.0 {
                                                let product = *i * *f as i64;
                                                return Some(product);
                                            }
                                        }

                                        None
                                    },
                                    move |mut access: TestAccess<'_, '_, '_>, product| {
                                        access.string.push(string);
                                        product
                                    },
                                )
                                .await
                                .unwrap()
                        }
                    },
                )
                .connect(scope.terminate);
        });

        let integers: Vec<i64> = vec![0, 1, 2, 3, 4, 5];
        let floats: Vec<f32> = vec![1.1, 0.3, 2.2, 3.4, -10.0, 5.0];
        let string = String::from("hello");

        let result = context.resolve_request((integers, floats, string), workflow);
        assert!(result > 4);
    }

    async fn async_distribute_values(
        Async {
            request, channel, ..
        }: Async<(TestJoined, TestKeys)>,
    ) {
        let (values, keys) = request;
        channel.distribute(keys, values).await.unwrap();
    }

    async fn async_join_values(
        Async {
            request, channel, ..
        }: Async<((), TestKeys)>,
    ) -> TestJoined {
        let (_, keys) = request;
        channel.try_join(keys).await.unwrap().unwrap()
    }

    async fn async_wait_for_join_values(
        Async {
            request, channel, ..
        }: Async<((), TestKeys)>,
    ) -> TestJoined {
        let (_, keys) = request;
        channel.wait_for_join(keys).await.unwrap()
    }

    async fn slowly_stream_values<T: 'static + Send + Sync>(
        Async {
            request, streams, ..
        }: Async<Vec<T>, StreamOf<T>>,
    ) {
        for value in request {
            let start = Instant::now();
            let duration = Duration::from_secs_f32(0.01);
            let mut elapsed = start.elapsed();
            while elapsed < duration {
                let never = async_std::future::pending::<()>();
                let timeout = duration - elapsed;
                let _ = async_std::future::timeout(timeout, never).await;
                elapsed = start.elapsed();
            }

            streams.send(value);
        }
    }

    #[derive(Accessor, Clone)]
    #[accessor(
        buffers_struct_name = TestBuffers,
        use_as_joined = TestJoined,
        view_struct_name = TestView,
        access_struct_name = TestAccess,
    )]
    struct TestKeys {
        integer: BufferKey<i64>,
        float: BufferKey<f32>,
        string: BufferKey<String>,
    }

    #[derive(Clone)]
    struct TestJoined {
        integer: i64,
        float: f32,
        string: String,
    }
}
