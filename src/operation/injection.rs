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
    ActiveTasksStorage, AddExecution, Cleanup, CleanupContents, DisposeForUnavailableService,
    Executable, FinalizeCleanup, FinalizeCleanupRequest, InScope, Input, InputBundle,
    ManageDisposal, ManageInput, OperateService, Operation, OperationCleanup,
    OperationReachability, OperationRequest, OperationResult, OperationSetup, OrBroken,
    ProviderStorage, ReachabilityResult, RequestId, RouteSource, RouteTarget, Routing,
    ServiceInstructions, ServiceRequest, SingleInputStorage, SingleTargetStorage, StreamPack,
    StreamTargetMap, dispatch_service, output_port,
};

use bevy_ecs::prelude::{ChildOf, Command, Component, Entity};

use smallvec::{SmallVec, smallvec};

use std::collections::HashMap;

pub(crate) struct Injection<Request, Response, Streams> {
    target: Entity,
    _ignore: std::marker::PhantomData<fn(Request, Response, Streams)>,
}

impl<Request, Response, Streams> Operation for Injection<Request, Response, Streams>
where
    Request: 'static + Send + Sync,
    Response: 'static + Send + Sync,
    Streams: StreamPack,
{
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        world
            .get_entity_mut(self.target)
            .or_broken()?
            .insert(SingleInputStorage::new(source));

        world.entity_mut(source).insert((
            InjectionStorage::default(),
            InputBundle::<(Request, ServiceInstructions<Request, Response, Streams>)>::new(),
            SingleTargetStorage::new(self.target),
            CleanupContents::new(),
            AwaitingCleanup::default(),
            FinalizeCleanup::new(Self::finalize_cleanup),
        ));

        Ok(())
    }

    fn execute(
        OperationRequest {
            source,
            world,
            roster,
        }: OperationRequest,
    ) -> OperationResult {
        let Input {
            session,
            data: (request, service),
            seq,
        } = world
            .take_input::<(Request, ServiceInstructions<Request, Response, Streams>)>(source)?;
        let request_id = RequestId {
            session,
            source,
            seq,
        };

        let source_ref = world.get_entity(source).or_broken()?;
        let scope = source_ref.get::<InScope>().or_broken()?.scope();
        let provider = service.provider();
        let instructions = service.instructions().cloned();

        let stream_targets = source_ref
            .get::<StreamTargetMap>()
            .cloned()
            .unwrap_or_else(|| StreamTargetMap::default());

        let finish = world
            .spawn((
                InputBundle::<Response>::new(),
                InjectionId(request_id),
                ChildOf(source),
            ))
            .id();
        AddExecution::new(finish, InjectionFinish::<Response>::new()).apply(world);

        let task = world
            .spawn((
                InputBundle::<Request>::new(),
                ProviderStorage(provider),
                SingleTargetStorage::new(finish),
                ActiveTasksStorage::default(),
                DisposeForUnavailableService::new::<Request>(),
                InScope::new(scope),
                stream_targets,
            ))
            .id();

        let port = output_port::inject();
        let route = request_id.to_message_route(&port, task);
        // SAFETY: We must do a sneak_input here because we do not want the
        // roster to register the task as an operation. In fact it does not
        // implement Operation at all. It is just a temporary container for the
        // input and the stream targets.
        let execute = unsafe { world.sneak_input(route, request, false, roster)? };

        if !execute {
            // If giving the input failed then this workflow will not be able to
            // proceed. Therefore we should report that this is broken.
            None.or_broken()?;
        }

        let mut storage = world.get_mut::<InjectionStorage>(source).or_broken()?;
        storage.list.push(Injected {
            session,
            task,
            finish,
        });
        dispatch_service(ServiceRequest {
            provider,
            target: finish,
            instructions,
            operation: OperationRequest {
                source: task,
                world,
                roster,
            },
        });

        Ok(())
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<(Request, ServiceInstructions<Request, Response, Streams>)>()?;
        clean.cleanup_disposals()?;

        let OperationCleanup {
            source,
            cleanup,
            world,
            roster,
        } = clean;
        let session = cleanup.session;
        let cleanup_id = cleanup.cleanup_id;
        let mut storage = world.get_mut::<InjectionStorage>(source).or_broken()?;
        let tasks: SmallVec<[Entity; 16]> = storage
            .list
            .iter()
            .filter_map(|injected| {
                if injected.session == session {
                    Some(injected.task)
                } else {
                    None
                }
            })
            .collect();
        storage.list.retain(|injected| injected.session != session);

        if tasks.is_empty() {
            // No cleanup needed, just notify right away
            cleanup.notify_cleaned(world, roster)?;
            return Ok(());
        }

        world
            .get_mut::<CleanupContents>(source)
            .or_broken()?
            .add_cleanup(cleanup_id, tasks.clone());
        world
            .get_mut::<AwaitingCleanup>(source)
            .or_broken()?
            .map
            .insert(cleanup_id, cleanup);

        for node in tasks.iter().copied() {
            let cleanup = Cleanup {
                cleaner: source,
                node,
                session,
                cleanup_id,
            };
            let clean = OperationCleanup {
                source: node,
                cleanup,
                world,
                roster,
            };
            OperateService::<Request>::cleanup(clean)?;
        }

        Ok(())
    }

    fn is_reachable(mut reachability: OperationReachability) -> ReachabilityResult {
        if reachability.has_input::<(Request, ServiceInstructions<Request, Response, Streams>)>()? {
            return Ok(true);
        }

        if InjectionStorage::contains_session(&reachability)? {
            return Ok(true);
        }

        SingleInputStorage::is_reachable(&mut reachability)
    }
}

impl<Request, Response, Streams> Injection<Request, Response, Streams> {
    fn finalize_cleanup(
        FinalizeCleanupRequest {
            cleanup,
            world,
            roster,
        }: FinalizeCleanupRequest,
    ) -> OperationResult {
        let source = cleanup.cleaner;
        let parent_cleanup = world
            .get_mut::<AwaitingCleanup>(source)
            .or_broken()?
            .map
            .remove(&cleanup.cleanup_id)
            .or_broken()?;
        parent_cleanup.notify_cleaned(world, roster)
    }

    pub(crate) fn new(target: Entity) -> Self {
        Self {
            target,
            _ignore: Default::default(),
        }
    }
}

#[derive(Component, Default)]
struct InjectionStorage {
    list: SmallVec<[Injected; 16]>,
}

#[derive(Component, Default)]
struct AwaitingCleanup {
    // Map from cleanup_id to the upstream cleaner
    map: HashMap<RequestId, Cleanup>,
}

impl InjectionStorage {
    fn contains_session(r: &OperationReachability) -> ReachabilityResult {
        Ok(r.world()
            .get::<Self>(r.source())
            .or_broken()?
            .list
            .iter()
            .any(|injected| injected.session == r.session))
    }
}

#[derive(Clone, Copy)]
struct Injected {
    session: Entity,
    task: Entity,
    finish: Entity,
}

#[derive(Component)]
struct InjectionId(RequestId);

struct InjectionFinish<Response> {
    _ignore: std::marker::PhantomData<Response>,
}

impl<Response> InjectionFinish<Response> {
    fn new() -> Self {
        Self {
            _ignore: Default::default(),
        }
    }
}

impl<Response> Executable for InjectionFinish<Response>
where
    Response: 'static + Send + Sync,
{
    fn setup(self, _: OperationSetup) -> OperationResult {
        Ok(())
    }

    fn execute(
        OperationRequest {
            source,
            world,
            roster,
        }: OperationRequest,
    ) -> OperationResult {
        let Input {
            session: injection_session,
            data,
            seq,
        } = world.take_input::<Response>(source)?;
        let req = world.get::<InjectionId>(source).or_broken()?.0;
        world.despawn(source);

        let mut injector_mut = world.get_entity_mut(req.source).or_broken()?;
        let target = injector_mut.get::<SingleTargetStorage>().or_broken()?.get();
        let mut storage = injector_mut.get_mut::<InjectionStorage>().or_broken()?;
        let injected = *storage
            .list
            .iter()
            .find(|injected| injected.finish == source)
            .or_broken()?;
        storage.list.retain(|injected| injected.finish != source);
        world.transfer_disposals(injected.task, req.source)?;
        world.despawn(injected.task);

        let port = output_port::next();
        let finish = output_port::finish();
        let injector_output = req.to_route_source(&port);
        let finish_output = RouteSource {
            session: injection_session,
            source,
            seq,
            port: &finish,
        };
        let route = Routing {
            outputs: smallvec![injector_output, finish_output],
            input: RouteTarget {
                session: req.session,
                target,
            },
        };
        world.give_input(route, data, roster)?;

        Ok(())
    }
}
