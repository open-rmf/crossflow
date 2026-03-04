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
    Async, AsyncService, Blocker, Channel, ChannelQueue, ChooseAsyncServiceDelivery,
    Deliver, Delivery, DeliveryOrder, DeliveryUpdate, Disposal, Input, IntoService, ManageInput,
    OperateTask, OperationError, OperationRequest, OperationResult, OperationRoster, OrBroken,
    Sendish, ServiceBuilder, ServiceBundle, ServiceRequest, ServiceTrait, SingleTargetStorage,
    StopTask, StopTaskFailure, StreamPack, UnhandledErrors, RequestId, Seq, output_port, ManageDisposal,
    async_execution::{spawn_task, task_cancel_sender},
    dispose_for_despawned_service, insert_new_order, pop_next_delivery,
    service::service_builder::{ParallelChosen, SerialChosen},
};

use bevy_ecs::{
    prelude::{Component, Entity, World},
    system::{BoxedSystem, EntityCommands, IntoSystem},
    world::EntityWorldMut,
};

use std::future::Future;

pub trait IsAsyncService<M> {}

#[derive(Component)]
struct AsyncServiceStorage<Request, Streams: StreamPack, Task>(
    Option<BoxedSystem<AsyncService<Request, Streams>, Task>>,
);

#[derive(Component)]
struct UninitAsyncServiceStorage<Request, Streams: StreamPack, Task>(
    BoxedSystem<AsyncService<Request, Streams>, Task>,
);

pub struct AsyncServiceMarker<M>(std::marker::PhantomData<fn(M)>);

impl<Request, Streams, Task, M, Sys> IntoService<AsyncServiceMarker<(Request, Streams, Task, M)>> for Sys
where
    Sys: IntoSystem<AsyncService<Request, Streams>, Task, M>,
    Task: Future + 'static + Sendish,
    Request: 'static + Send + Sync,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Task::Output;
    type Streams = Streams;
    type DefaultDeliver = ();

    fn insert_service_commands(self, entity_commands: &mut EntityCommands) {
        entity_commands.insert((
            UninitAsyncServiceStorage(Box::new(IntoSystem::into_system(self))),
            ServiceBundle::<AsyncServiceStorage<Request, Streams, Task>>::new(),
        ));
    }

    fn insert_service_mut(self, entity_mut: &mut EntityWorldMut) {
        entity_mut.insert((
            UninitAsyncServiceStorage(Box::new(IntoSystem::into_system(self))),
            ServiceBundle::<AsyncServiceStorage<Request, Streams, Task>>::new(),
        ));
    }
}

impl<Request, Streams, Task, M, Sys> IsAsyncService<AsyncServiceMarker<(Request, Streams, Task, M)>> for Sys
where
    Sys: IntoSystem<AsyncService<Request, Streams>, Task, M>,
    Task: Future + 'static + Sendish,
    Request: 'static + Send + Sync,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
}

impl<Request, Streams, Task> ServiceTrait for AsyncServiceStorage<Request, Streams, Task>
where
    Request: 'static + Send + Sync,
    Task: Future + 'static + Sendish,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
    type Request = Request;
    type Response = Task::Output;
    fn serve(
        ServiceRequest {
            provider,
            target,
            instructions,
            operation:
                OperationRequest {
                    source,
                    world,
                    roster,
                },
        }: ServiceRequest,
    ) -> OperationResult {
        let Input {
            session,
            data: request,
            seq,
        } = world.take_input::<Request>(source)?;
        let task_id = world.spawn(()).id();

        let Some(mut delivery) = world.get_mut::<Delivery<Request>>(provider) else {
            // The async service's Delivery component has been removed so we should treat the request as cancelled.
            dispose_for_despawned_service(provider, world, roster);
            return Err(OperationError::NotReady);
        };

        let update = insert_new_order::<Request>(
            delivery.as_mut(),
            DeliveryOrder {
                request_id: RequestId { session, source, seq },
                task_id,
                request,
                instructions: instructions.clone(),
            },
        );

        let (request, seq, blocker) = match update {
            DeliveryUpdate::Immediate { blocking, request, seq } => {
                let serve_next = serve_next_async_request::<Request, Streams, Task>;
                let blocker = blocking.map(|label| Blocker {
                    provider,
                    request_id: RequestId { session, source, seq },
                    label,
                    serve_next,
                });
                (request, seq, blocker)
            }
            DeliveryUpdate::Queued {
                cancelled,
                stop,
                label,
            } => {
                for cancelled in cancelled {
                    let disposal = Disposal::supplanted(RequestId { session, source, seq });
                    let port = output_port::next();
                    let route = cancelled.request_id.to_route_source(&port);
                    world.emit_disposal(route, disposal, roster);
                    if let Ok(task_mut) = world.get_entity_mut(cancelled.task_id) {
                        task_mut.despawn();
                    }
                }
                if let Some(stop) = stop {
                    // This task is already running and we need to stop it at the
                    // task source level
                    let result = world
                        .get_entity(stop.task_id)
                        .or_broken()
                        .and_then(|task_ref| task_ref.get::<StopTask>().or_broken().copied())
                        .and_then(|stop_task| {
                            let disposal = Disposal::supplanted(RequestId { session, source, seq });
                            (stop_task.0)(
                                OperationRequest {
                                    source: stop.task_id,
                                    world,
                                    roster,
                                },
                                disposal,
                            )
                        });

                    if let Err(OperationError::Broken(backtrace)) = result {
                        world
                            .get_resource_or_insert_with(UnhandledErrors::default)
                            .stop_tasks
                            .push(StopTaskFailure {
                                task: stop.task_id,
                                backtrace,
                            });

                        // Immediately queue up an unblocking, otherwise the next
                        // task will never be able to run.
                        let serve_next = serve_next_async_request::<Request, Streams, Task>;
                        roster.unblock(Blocker {
                            provider,
                            request_id: stop.request_id,
                            label,
                            serve_next,
                        });
                    }
                }

                // The request has been queued up and should be delivered later
                return Ok(());
            }
        };

        serve_async_request::<Request, Streams, Task>(
            request,
            blocker,
            session,
            task_id,
            seq,
            ServiceRequest {
                provider,
                target,
                instructions,
                operation: OperationRequest {
                    source,
                    world,
                    roster,
                },
            },
        )
    }
}

fn serve_async_request<Request, Streams, Task>(
    request: Request,
    blocker: Option<Blocker>,
    session: Entity,
    task_id: Entity,
    seq: Seq,
    cmd: ServiceRequest,
) -> OperationResult
where
    Request: 'static + Send + Sync,
    Task: Future + 'static + Sendish,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
    let ServiceRequest {
        provider,
        target,
        instructions: _,
        operation:
            OperationRequest {
                source,
                world,
                roster,
            },
    } = cmd;
    let mut service = if let Ok(mut provider_mut) = world.get_entity_mut(provider) {
        if let Some(mut storage) =
            provider_mut.get_mut::<AsyncServiceStorage<Request, Streams, Task>>()
        {
            storage
                .0
                .take()
                .expect("Async service is missing while attempting to serve")
        } else if let Some(uninit) =
            provider_mut.take::<UninitAsyncServiceStorage<Request, Streams, Task>>()
        {
            // We need to initialize the service
            let mut service = uninit.0;
            service.initialize(world);

            // Re-obtain the provider since we needed to mutably borrow the world a moment ago
            let mut provider_mut = world.entity_mut(provider);
            provider_mut.insert(AsyncServiceStorage::<Request, Streams, Task>(None));
            service
        } else {
            // The provider has had its service removed, so we treat this request as cancelled.
            dispose_for_despawned_service(provider, world, roster);
            // We've already issued the disposal, but we need to return an
            // error so that serve_next_async_request continues iterating.
            return Err(OperationError::NotReady);
        }
    } else {
        // If the provider has been despawned then we treat this request as cancelled.
        dispose_for_despawned_service(provider, world, roster);
        // We've already issued the disposal, but we need to return an
        // error so that serve_next_async_request continues iterating.
        return Err(OperationError::NotReady);
    };

    let sender = world
        .get_resource_or_insert_with(ChannelQueue::new)
        .sender
        .clone();
    let request_id = RequestId { source, seq, session };
    let channel = Channel::new(request_id, sender.clone());
    let streams = channel.for_streams::<Streams>(world)?;
    let job = service.run(
        AsyncService {
            request,
            streams,
            channel,
            provider,
            id: request_id,
        },
        world,
    );
    service.apply_deferred(world);

    if let Some(mut service_storage) =
        world.get_mut::<AsyncServiceStorage<Request, Streams, Task>>(provider)
    {
        service_storage.0 = Some(service);
    } else {
        // We've already done everything we need to do with the service, but
        // apparently the service erased itself. We will allow the task to keep
        // running since there is nothing to prevent it from doing so.
        //
        // TODO(@mxgrey): Consider whether the removal of the service should
        // imply that all the service's active tasks should be dropped?
    }

    let task = spawn_task(job, world);
    let cancel_sender = task_cancel_sender(world);

    OperateTask::<_, Streams>::new(
        task_id,
        request_id,
        target,
        task,
        cancel_sender,
        blocker,
        sender,
    )
    .add(world, roster);
    Ok(())
}

pub(crate) fn serve_next_async_request<Request, Streams, Task>(
    unblock: Blocker,
    world: &mut World,
    roster: &mut OperationRoster,
) where
    Request: 'static + Send + Sync,
    Task: Future + 'static + Sendish,
    Task::Output: 'static + Send + Sync,
    Streams: StreamPack,
{
    let Blocker {
        provider, label, ..
    } = unblock;
    loop {
        let Some(Deliver {
            request,
            task_id,
            blocker,
        }) = pop_next_delivery::<Request>(
            provider,
            label.clone(),
            serve_next_async_request::<Request, Streams, Task>,
            world,
        )
        else {
            // No more deliveries to pop, so we should return
            return;
        };

        let session = blocker.request_id.session;
        let source = blocker.request_id.source;
        let seq = blocker.request_id.seq;

        let Some(target) = world.get::<SingleTargetStorage>(source) else {
            // This will not be able to run, so we should move onto the next
            // item in the queue.
            continue;
        };
        let target = target.get();

        if serve_async_request::<Request, Streams, Task>(
            request,
            Some(blocker),
            session,
            task_id,
            seq,
            ServiceRequest {
                provider,
                target,
                // Instructions are already being handled by the delivery queue
                instructions: None,
                operation: OperationRequest {
                    source,
                    world,
                    roster,
                },
            },
        )
        .is_err()
        {
            // The service did not launch so we should move onto the next item
            // in the queue.
            continue;
        }

        // The next delivery has begun so we can return
        return;
    }
}

pub struct AsyncMarker<M>(std::marker::PhantomData<fn(M)>);

impl<Request, Streams, Task, M, Sys> IntoService<AsyncMarker<(Request, Streams, Task, M)>> for Sys
where
    Sys: IntoSystem<Async<Request, Streams>, Task, M>,
    Task: Future + 'static + Sendish,
    Request: 'static + Send + Sync,
    Streams: StreamPack,
    Task::Output: 'static + Send + Sync,
{
    type Request = Request;
    type Response = Task::Output;
    type Streams = Streams;
    type DefaultDeliver = ();

    fn insert_service_commands(self, entity_commands: &mut EntityCommands) {
        peel_service_provider
            .pipe(self)
            .insert_service_commands(entity_commands)
    }

    fn insert_service_mut(self, entity_mut: &mut EntityWorldMut) {
        peel_service_provider.pipe(self).insert_service_mut(entity_mut)
    }
}

impl<Request, Streams, Task, M, Sys> IsAsyncService<AsyncMarker<(Request, Streams, Task, M)>> for Sys
where
    Sys: IntoSystem<Async<Request, Streams>, Task, M>,
    Task: Future + 'static + Sendish,
    Request: 'static + Send + Sync,
    Streams: StreamPack,
    Task::Output: 'static + Send + Sync,
{
}

impl<M, Srv> ChooseAsyncServiceDelivery<M> for Srv
where
    Srv: IntoService<M> + IsAsyncService<M>,
{
    type Service = Srv;
    fn serial(self) -> ServiceBuilder<Srv, SerialChosen, (), (), ()> {
        ServiceBuilder::new(self)
    }
    fn parallel(self) -> ServiceBuilder<Srv, ParallelChosen, (), (), ()> {
        ServiceBuilder::new(self)
    }
}

fn peel_service_provider<Request, Streams: StreamPack>(
    input: AsyncService<Request, Streams>
) ->  Async<Request, Streams> {
    input.into()
}
