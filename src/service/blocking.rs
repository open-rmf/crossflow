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

use bevy_ecs::{
    prelude::Component,
    system::{BoxedSystem, EntityCommands, IntoSystem},
    world::EntityWorldMut,
};

use crate::{
    Blocking, BlockingService, Input, IntoService, ManageDisposal, ManageInput,
    OperationError, OperationRequest, OrBroken, ServiceBundle, ServiceRequest, ServiceTrait,
    StreamPack, UnusedStreams, dispose_for_despawned_service, make_stream_buffers_from_world,
    service::service_builder::BlockingChosen, MessageRoute, output_port, RequestId,
};

pub struct BlockingServiceMarker<M>(std::marker::PhantomData<fn(M)>);

#[derive(Component)]
struct BlockingServiceStorage<Request, Response, Streams: StreamPack>(
    Option<BoxedSystem<BlockingService<Request, Streams>, Response>>,
);

#[derive(Component)]
struct UninitBlockingServiceStorage<Request, Response, Streams: StreamPack>(
    BoxedSystem<BlockingService<Request, Streams>, Response>,
);

impl<Request, Response, Streams, M, Sys> IntoService<BlockingServiceMarker<(Request, Response, Streams, M)>>
    for Sys
where
    Sys: IntoSystem<BlockingService<Request, Streams>, Response, M>,
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Response;
    type Streams = Streams;
    type DefaultDeliver = BlockingChosen;

    fn insert_service_commands(self, entity_commands: &mut EntityCommands) {
        entity_commands.insert((
            UninitBlockingServiceStorage(Box::new(IntoSystem::into_system(self))),
            ServiceBundle::<BlockingServiceStorage<Request, Response, Streams>>::new(),
        ));
    }

    fn insert_service_mut(self, entity_mut: &mut EntityWorldMut) {
        entity_mut.insert((
            UninitBlockingServiceStorage(Box::new(IntoSystem::into_system(self))),
            ServiceBundle::<BlockingServiceStorage<Request, Response, Streams>>::new(),
        ));
    }
}

pub struct BlockingMarker<M>(std::marker::PhantomData<fn(M)>);

impl<Request, Response, Streams, M, Sys> IntoService<BlockingMarker<(Request, Response, Streams, M)>>
 for Sys
where
    Sys: IntoSystem<Blocking<Request, Streams>, Response, M>,
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Response;
    type Streams = Streams;
    type DefaultDeliver = BlockingChosen;

    fn insert_service_commands(self, entity_commands: &mut EntityCommands) {
        peel_service_provider.pipe(self).insert_service_commands(entity_commands)
    }

    fn insert_service_mut(self, entity_mut: &mut EntityWorldMut) {
        peel_service_provider.pipe(self).insert_service_mut(entity_mut)
    }
}

fn peel_service_provider<Request, Streams: StreamPack>(
    input: BlockingService<Request, Streams>,
) -> Blocking<Request, Streams> {
    input.into()
}


impl<Request, Response, Streams> ServiceTrait for BlockingServiceStorage<Request, Response, Streams>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Response;
    fn serve(
        ServiceRequest {
            provider,
            target,
            instructions: _,
            operation:
                OperationRequest {
                    source,
                    world,
                    roster,
                },
        }: ServiceRequest,
    ) -> Result<(), OperationError> {
        let Input {
            session,
            data: request,
            seq,
        } = world.take_input::<Request>(source)?;

        let mut service = if let Ok(mut provider_mut) = world.get_entity_mut(provider) {
            if let Some(mut storage) =
                provider_mut.get_mut::<BlockingServiceStorage<Request, Response, Streams>>()
            {
                storage
                    .0
                    .take()
                    .or_broken()?
            } else {
                // Check if the system still needs to be initialized
                if let Some(uninit) =
                    provider_mut.take::<UninitBlockingServiceStorage<Request, Response, Streams>>()
                {
                    // We need to initialize the service
                    let mut service = uninit.0;
                    service.initialize(world);

                    // Re-obtain the provider since we needed to mutably borrow the world a moment ago
                    let mut provider_mut = world.entity_mut(provider);
                    provider_mut.insert(BlockingServiceStorage::<Request, Response, Streams>(None));
                    service
                } else {
                    // The provider has had its service removed, so we treat this request as cancelled.
                    dispose_for_despawned_service(provider, world, roster);
                    return Ok(());
                }
            }
        } else {
            // If the provider has been despawned then we treat this request as cancelled.
            dispose_for_despawned_service(provider, world, roster);
            return Ok(());
        };

        let streams = make_stream_buffers_from_world::<Streams>(source, world)?;
        let response = service.run(
            BlockingService {
                request,
                streams: streams.clone(),
                provider,
                id: RequestId { source, seq, session },
            },
            world,
        );
        service.apply_deferred(world);

        let request_id = RequestId { session, source, seq };
        let mut unused_streams = UnusedStreams::new(request_id);
        Streams::process_stream_buffers(
            streams,
            request_id,
            &mut unused_streams,
            world,
            roster,
        )?;

        if let Ok(mut provider_mut) = world.get_entity_mut(provider) {
            if let Some(mut storage) =
                provider_mut.get_mut::<BlockingServiceStorage<Request, Response, Streams>>()
            {
                storage.0 = Some(service);
            } else {
                // The service storage has been removed for some reason. We
                // will treat this as the service itself being removed. But we can
                // still complete this service request.
            }
        } else {
            // Apparently the service was despawned by the service itself.
            // But we can still deliver the response to the target, so we will
            // not consider this to be cancelled.
        }

        if !unused_streams.streams.is_empty() {
            let port = output_port::name_str("stream_out");
            let route = request_id.to_route_source(&port);
            world.emit_disposal(route, unused_streams.into(), roster);
        }

        let route = MessageRoute {
            session,
            source,
            seq,
            port: &output_port::next(),
            target,
        };
        world.give_input(route, response, roster)?;
        Ok(())
    }
}
