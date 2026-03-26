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
    OperationError, OperationRoster, Outcome, Promise, ProvideOnce, Reply, RequestExt, RequestId,
    Seq, StreamPack, Accessor, BufferWorldAccess, AccessError,
    async_execution::spawn_task,
};

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
        self.world(move |world| {
            world.buffers_mut(req, &accessor, f)
        })
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
    #[must_use = "If the reply is dropped without being detached, the access callback might not be executed"]
    pub fn access_when<A: Accessor, V: 'static, U: 'static + Send>(
        &self,
        accessor: A,
        when: impl FnMut(A::View<'_>, &World) -> Option<V> + 'static + Send,
        then: impl FnOnce(A::Access<'_, '_, '_>, V) -> U + 'static + Send,
    ) -> Reply<Result<U, AccessError>> {
        access_when(self.inner.clone(), accessor, when, then)
    }

    /// Try to join values from the buffers of this accessor. If one or more of
    /// the buffers are not ready to join, then this will return `Ok(None)`.
    pub fn try_join<A: Accessor>(&self, accessor: A) -> Reply<Result<Option<A::Joined>, AccessError>> {
        let req = self.inner.request_id;
        self.world(move |world| {
            accessor.join(req, world)
        })
    }

    /// Keep trying to join until all the buffers are ready.
    pub fn wait_for_join<A: Accessor>(&self, accessor: A) -> Reply<Result<A::Joined, AccessError>> {
        wait_for_join(self.inner.clone(), accessor)
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

fn wait_for_join<A: Accessor>(
    channel: Arc<InnerChannel>,
    mut accessor: A,
) -> Reply<Result<A::Joined, AccessError>> {
    let req = channel.request_id;
    let (sender, receiver) = oneshot::channel();
    let reply = Reply::new(receiver);
    let detached = reply.detached();
    let channel_clone = Arc::clone(&channel);
    let _ = channel_clone.sender.send(Box::new(move |world: &mut World, _: &mut OperationRoster| {
        match accessor.join(req, world) {
            Ok(joined) => {
                if let Some(joined) = joined {
                    // Joining succeeded, return the result
                    let _ = sender.send(Ok(joined));
                    return;
                }
            }
            Err(err) => {
                // Access failed, return the error
                let _ = sender.send(Err(err));
                return;
            }
        }

        let seen = accessor.make_seen(world);
        accessor.seen(seen);

        let shared_sender = Arc::new(Mutex::new(Some(sender)));

        // The buffers were not ready to join, so begin an async task to join them
        spawn_task(
            async move {
                loop {
                    if detached.load(Ordering::Acquire) {
                        // The Reply is detached, so we can ignore responding to
                        // whether or not it's dropped.
                        let _ = accessor.wait_for_change();
                    } else {
                        let mut locked_sender = shared_sender.lock();
                        if let Ok(Some(sender)) = locked_sender.as_mut().map(|l| l.as_mut()) {
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
                        } else {
                            // For some reason the sender is no longer available.
                            // This suggests that the join was already performed
                            // somehow, even though that shouldn't be the case.
                            return;
                        }
                    }

                    let sender = shared_sender.clone();
                    let a = accessor.clone();
                    let (seen_sender, seen_receiver) = oneshot::channel();
                    let _ = channel.sender.send(Box::new(
                        move |world: &mut World, _: &mut OperationRoster| {
                            match a.join(req, world) {
                                Ok(Some(joined)) => {
                                    if let Ok(Some(sender)) = sender.lock().map(|mut l| l.take()) {
                                        let _ = sender.send(Ok(joined));
                                    }
                                }
                                Err(err) => {
                                    if let Ok(Some(sender)) = sender.lock().map(|mut l| l.take()) {
                                        let _ = sender.send(Err(err));
                                    }
                                }
                                Ok(None) => {
                                    // Not ready yet, need to try again
                                }
                            }

                            let seen = a.make_seen(world);
                            let _ = seen_sender.send(seen);
                        }
                    ));

                    let Ok(seen) = seen_receiver.await else {
                        // We aren't receiveing a new seen update, so that means
                        // the task is finished.
                        return;
                    };
                    accessor.seen(seen);
                }
            },
            world
        )
        .detach();
    }));

    reply
}

fn access_when<A: Accessor, V: 'static, U: 'static + Send>(
    channel: Arc<InnerChannel>,
    mut accessor: A,
    mut when: impl FnMut(A::View<'_>, &World) -> Option<V> + 'static + Send,
    then: impl FnOnce(A::Access<'_, '_, '_>, V) -> U + 'static + Send,
) -> Reply<Result<U, AccessError>> {
    let req = channel.request_id;
    let (sender, receiver) = oneshot::channel();
    let reply = Reply::new(receiver);
    let detached = reply.detached();
    let channel_clone = Arc::clone(&channel);
    let _ = channel_clone.sender.send(Box::new(move |world: &mut World, _: &mut OperationRoster| {
        let view = match world.buffers_view_untraced(&accessor) {
            Ok(view) => view,
            Err(err) => {
                let _ = sender.send(Err(err.into()));
                return;
            }
        };

        if let Some(value) = when(view, world) {
            // The buffers are ready right away, so execute
            let then = move |access: A::Access<'_, '_, '_>| {
                then(access, value)
            };
            let _ = sender.send(world.buffers_mut(req, &accessor, then));
            return;
        }

        // The buffers were not ready, so loop around, awaiting the condition
        let seen = accessor.make_seen(world);
        accessor.seen(seen);

        // The callbacks now need to be wrapped in Arc<Mutex<Option>> so
        // they can be shared between the async task and the world callbacks.
        let shared_when = Arc::new(Mutex::new(when));
        let shared_then = Arc::new(Mutex::new(Some((then, sender))));

        // We can't spawn the task until we have world access, or else this
        // method won't work with the single_threaded_async feature. The first
        // pass above avoids various overhead if the buffers happen to already
        // be in the right states.
        spawn_task(
            async move {
                loop {
                    if detached.load(Ordering::Acquire) {
                        // The Reply is detached, so we can ignore responding to
                        // whether or not it's dropped.
                        let _ = accessor.wait_for_change();
                    } else {
                        let mut locked_then = shared_then.lock();
                        if let Ok(Some((_, sender))) = locked_then.as_mut().map(|l| l.as_mut()) {
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
                        } else {
                            // For some reason the sender is no longer available.
                            // This shouldn't happen, but anyhow there's no way
                            // to execute the access anymore.
                            return;
                        }
                    }

                    // One of the buffers has changed, so send a task to test
                    // if the conditions are met and then execute if they are.
                    let when = shared_when.clone();
                    let then = shared_then.clone();
                    let (seen_sender, seen_receiver) = oneshot::channel();
                    let a = accessor.clone();
                    let detached = detached.clone();
                    let _ = channel.sender.send(Box::new(
                        move |world: &mut World, _: &mut OperationRoster| {
                            let view = match world.buffers_view_untraced(&a) {
                                Ok(view) => view,
                                Err(err) => {
                                    // SAFETY: There is no risk of the mutex getting poisoned because
                                    // there are no operations that can panic while the mutex is locked.
                                    if let Ok(Some((_, sender))) = then.lock().map(|mut l| l.take()) {
                                        let _ = sender.send(Err(err.into()));
                                    }
                                    return;
                                }
                            };

                            let mut when = match when.lock() {
                                Ok(when) => when,
                                Err(_) => {
                                    if let Ok(Some((_, sender))) = then.lock().map(|mut l| l.take()) {
                                        let _ = sender.send(Err(AccessError::PoisonedMutex));
                                    }
                                    return;
                                }
                            };

                            if let Some(value) = (*when)(view, world) {
                                if let Ok(Some((then, sender))) = then.lock().map(|mut l| l.take()) {
                                    if !detached.load(std::sync::atomic::Ordering::Acquire) {
                                        // The task is not detached, so check if the receiver
                                        // is still awaiting this. If not, we shouldn't apply
                                        // the access function.
                                        if sender.is_closed() {
                                            // The receiver has dropped and the task is not
                                            // detached, so do not run the callback.
                                            return;
                                        }
                                    }

                                    let then = move |access: A::Access<'_, '_, '_>| {
                                        then(access, value)
                                    };
                                    let _ = sender.send(world.buffers_mut(req, &a, then));
                                }
                            } else {
                                // It's not time yet, so update the async task about which
                                // buffer states we saw.
                                let seen = a.make_seen(world);
                                let _ = seen_sender.send(seen);
                            }
                        }
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
        ).detach();
    }));

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
}
