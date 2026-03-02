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

use bevy_ecs::{
    prelude::{ChildOf, Commands, Entity, World},
    system::Command,
};

pub use tokio::sync::mpsc::UnboundedReceiver as Receiver;
use tokio::sync::mpsc::unbounded_channel;

use std::{rc::Rc, sync::Arc};

use crate::{
    AddExecution, AddOperation, AnonymousStreamRedirect, Builder, DefaultStreamBufferContainer,
    DeferredRoster, InnerChannel, InputSlot, OperationError, OperationResult, OperationRoster,
    OrBroken, Output, Push, RedirectScopeStream, RedirectWorkflowStream, ReportUnhandled,
    SingleInputStorage, StreamAvailability, StreamBuffer, StreamEffect, StreamPack, StreamTarget,
    StreamRequest, StreamTargetMap, TakenStream, UnusedStreams, UnusedTarget, RequestId, output_port,
    dyn_node::{DynStreamInputPack, DynStreamOutputPack},
};

/// A wrapper to turn any [`StreamEffect`] into an anonymous (unnamed) stream.
/// This should be used if you want a stream with the same behavior as [`StreamOf`][1]
/// but with some additional side effect. The input and output data types of the
/// stream may be different.
///
/// [1]: crate::StreamOf
pub struct AnonymousStream<S: StreamEffect>(std::marker::PhantomData<fn(S)>);

impl<S: StreamEffect> StreamEffect for AnonymousStream<S> {
    type Input = S::Input;
    type Output = S::Output;
    fn side_effect(
        input: Self::Input,
        request: &mut StreamRequest,
    ) -> Result<Self::Output, OperationError> {
        S::side_effect(input, request)
    }
}

impl<S: StreamEffect> StreamPack for AnonymousStream<S> {
    type StreamInputPack = InputSlot<S::Input>;
    type StreamOutputPack = Output<S::Output>;
    type StreamReceivers = Receiver<S::Output>;
    type StreamChannels = AnonymousStreamChannel<S>;
    type StreamBuffers = StreamBuffer<S::Input>;
    type StreamTypes = (S,);

    fn spawn_scope_streams(
        in_scope: Entity,
        out_scope: Entity,
        commands: &mut Commands,
    ) -> (InputSlot<S::Input>, Output<S::Output>) {
        let source = commands.spawn(()).id();
        let target = commands.spawn(UnusedTarget).id();
        commands.queue(AddOperation::new(
            Some(in_scope),
            source,
            RedirectScopeStream::<Self>::new(target),
        ));

        (
            InputSlot::new(in_scope, source),
            Output::new(out_scope, target),
        )
    }

    fn spawn_workflow_streams(builder: &mut Builder) -> InputSlot<S::Input> {
        let source = builder.commands.spawn(()).id();
        builder.commands.queue(AddOperation::new(
            Some(builder.scope()),
            source,
            RedirectWorkflowStream::new(AnonymousStreamRedirect::<S>::new(None)),
        ));
        InputSlot::new(builder.scope(), source)
    }

    fn spawn_node_streams(
        source: Entity,
        map: &mut StreamTargetMap,
        builder: &mut Builder,
    ) -> Output<S::Output> {
        let target = builder
            .commands
            .spawn((SingleInputStorage::new(source), UnusedTarget))
            .id();

        map.add_anonymous::<S::Output>(target, builder.commands());
        Output::new(builder.scope(), target)
    }

    fn take_streams(
        source: Entity,
        map: &mut StreamTargetMap,
        commands: &mut Commands,
    ) -> Receiver<S::Output> {
        let (sender, receiver) = unbounded_channel::<S::Output>();
        let target = commands
            .spawn(())
            // Set the parent of this stream to be the series so it can be
            // recursively despawned together.
            .insert(ChildOf(source))
            .id();

        map.add_anonymous::<S::Output>(target, commands);
        commands.queue(AddExecution::new(target, TakenStream::new(sender)));

        receiver
    }

    fn collect_streams(
        source: Entity,
        target: Entity,
        map: &mut StreamTargetMap,
        commands: &mut Commands,
    ) {
        let redirect = commands.spawn(()).insert(ChildOf(source)).id();
        commands.queue(AddExecution::new(
            redirect,
            Push::<S::Output>::new(target, true),
        ));
        map.add_anonymous::<S::Output>(redirect, commands);
    }

    fn make_stream_channels(inner: &Arc<InnerChannel>, world: &World) -> Self::StreamChannels {
        let target = world
            .get::<StreamTargetMap>(inner.source())
            .and_then(|t| t.get_anonymous::<S::Output>());
        AnonymousStreamChannel::new(target, Arc::clone(inner))
    }

    fn make_stream_buffers(target_map: Option<&StreamTargetMap>) -> StreamBuffer<S::Input> {
        let target = target_map.and_then(|map| map.get_anonymous::<S::Output>());

        StreamBuffer {
            container: Default::default(),
            target,
        }
    }

    fn process_stream_buffers(
        buffer: Self::StreamBuffers,
        request_id: RequestId,
        unused: &mut UnusedStreams,
        world: &mut World,
        roster: &mut OperationRoster,
    ) -> OperationResult {
        let target = buffer.target;
        let mut was_unused = true;
        let RequestId { session, source, .. } = request_id;
        for data in Rc::into_inner(buffer.container)
            .or_broken()?
            .into_inner()
            .into_iter()
        {
            was_unused = false;
            let port = output_port::anonymous_stream(std::any::type_name::<S>());
            let mut request = StreamRequest {
                request_id,
                port: &port,
                target: target.map(|id| StreamTarget { id, session }),
                world,
                roster,
            };

            Self::side_effect(data, &mut request)
                .and_then(|output| request.send_output(output))
                .report_unhandled(source, world);
        }

        if was_unused {
            unused.streams.push(std::any::type_name::<Self>());
        }

        Ok(())
    }

    fn defer_buffers(
        buffer: Self::StreamBuffers,
        request_id: RequestId,
        commands: &mut Commands,
    ) {
        commands.queue(SendAnonymousStreams::<
            S,
            DefaultStreamBufferContainer<S::Input>,
        >::new(
            buffer.container.take(), request_id, buffer.target
        ));
    }

    fn set_stream_availability(availability: &mut StreamAvailability) {
        availability.add_anonymous::<S::Output>();
    }

    fn are_streams_available(availability: &StreamAvailability) -> bool {
        availability.has_anonymous::<S::Output>()
    }

    fn into_dyn_stream_input_pack(pack: &mut DynStreamInputPack, inputs: Self::StreamInputPack) {
        pack.add_anonymous(inputs);
    }

    fn into_dyn_stream_output_pack(
        pack: &mut DynStreamOutputPack,
        outputs: Self::StreamOutputPack,
    ) {
        pack.add_anonymous(outputs);
    }

    fn has_streams() -> bool {
        true
    }
}

pub struct SendAnonymousStreams<S, Container> {
    container: Container,
    request_id: RequestId,
    target: Option<Entity>,
    _ignore: std::marker::PhantomData<fn(S)>,
}

impl<S, Container> SendAnonymousStreams<S, Container> {
    pub fn new(
        container: Container,
        request_id: RequestId,
        target: Option<Entity>,
    ) -> Self {
        Self {
            container,
            request_id,
            target,
            _ignore: Default::default(),
        }
    }
}

impl<S, Container> Command for SendAnonymousStreams<S, Container>
where
    S: StreamEffect,
    Container: 'static + Send + Sync + IntoIterator<Item = S::Input>,
{
    fn apply(self, world: &mut World) {
        let RequestId { session, source, seq } = self.request_id;
        world.get_resource_or_init::<DeferredRoster>();
        world.resource_scope::<DeferredRoster, _>(|world, mut deferred| {
            let port = output_port::anonymous_stream(std::any::type_name::<S>());
            for data in self.container {
                let mut request = StreamRequest {
                    request_id: RequestId { source, seq, session },
                    port: &port,
                    target: self.target.map(|id| StreamTarget { id, session }),
                    world,
                    roster: &mut deferred,
                };

                S::side_effect(data, &mut request)
                    .and_then(move |output| request.send_output(output))
                    .report_unhandled(source, world);
            }
        });
    }
}

/// A channel to output messages from an anonymous (unnamed) stream
pub struct AnonymousStreamChannel<S> {
    target: Option<Entity>,
    inner: Arc<InnerChannel>,
    _ignore: std::marker::PhantomData<fn(S)>,
}

impl<S: StreamEffect> AnonymousStreamChannel<S> {
    /// Send an instance of data out over a stream.
    pub fn send(&self, data: S::Input) {
        let request_id = self.inner.request_id;
        let session = request_id.session;
        let source = request_id.source;
        let target = self.target;

        self.inner
            .sender
            .send(Box::new(
                move |world: &mut World, roster: &mut OperationRoster| {
                    let port = output_port::anonymous_stream(std::any::type_name::<S>());
                    let mut request = StreamRequest {
                        request_id,
                        port: &port,
                        target: target.map(|id| StreamTarget { id, session }),
                        world,
                        roster,
                    };

                    S::side_effect(data, &mut request)
                        .and_then(|output| request.send_output(output))
                        .report_unhandled(source, world);
                },
            ))
            .ok();
    }

    pub(crate) fn new(target: Option<Entity>, inner: Arc<InnerChannel>) -> Self {
        Self {
            target,
            inner,
            _ignore: Default::default(),
        }
    }
}

impl<S> Clone for AnonymousStreamChannel<S> {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            inner: Arc::clone(&self.inner),
            _ignore: Default::default(),
        }
    }
}
