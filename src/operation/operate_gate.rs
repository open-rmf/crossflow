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

use bevy_ecs::prelude::{Component, Entity};

use crate::{
    Buffering, Disposal, Gate, GateRequest, Input, InputBundle, ManageInput, Operation,
    OperationCleanup, OperationReachability, OperationRequest, OperationResult, OperationSetup,
    OrBroken, ReachabilityResult, SingleInputStorage, SingleTargetStorage, ManageDisposal,
    output_port, RequestId,
};

#[derive(Component)]
pub(crate) struct BufferRelationStorage<B>(B);

#[derive(Component, Clone, Copy)]
pub(crate) struct GateActionStorage(pub(crate) Gate);

pub(crate) struct OperateDynamicGate<T, B> {
    buffers: B,
    target: Entity,
    _ignore: std::marker::PhantomData<fn(T)>,
}

impl<B, T> OperateDynamicGate<T, B> {
    pub(crate) fn new(buffers: B, target: Entity) -> Self {
        Self {
            buffers,
            target,
            _ignore: Default::default(),
        }
    }
}

impl<T, B> Operation for OperateDynamicGate<T, B>
where
    T: 'static + Send + Sync,
    B: Buffering + 'static + Send + Sync,
{
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        world
            .get_entity_mut(self.target)
            .or_broken()?
            .insert(SingleInputStorage::new(source));

        world.entity_mut(source).insert((
            InputBundle::<GateRequest<T>>::new(),
            SingleTargetStorage::new(self.target),
            BufferRelationStorage(self.buffers),
            // We store Gate::Open for this here because this component is
            // checked by buffers when examining their reachability, and dynamic
            // gates can't know if they will open or close a buffer until the
            // input arrives, so we need to treat it as opening to avoid any
            // false negatives on reachability.
            GateActionStorage(Gate::Open),
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
            data: GateRequest { action, data },
            seq,
        } = world.take_input::<GateRequest<T>>(source)?;
        let request_id = RequestId { session, source, seq };

        let source_ref = world.get_entity(source).or_broken()?;
        let target = source_ref.get::<SingleTargetStorage>().or_broken()?.get();
        let buffers = source_ref
            .get::<BufferRelationStorage<B>>()
            .or_broken()?
            .0
            .clone();

        buffers.gate_action(request_id, session, action, world, roster)?;

        let port = output_port::next();
        world.give_input(request_id.to_message_route(&port, target), data, roster)?;

        if action.is_closed() {
            // When doing a closing, we should emit a disposal because we are
            // cutting off part of the workflow, which may alter the
            // reachability of the terminal node.
            let disposal = Disposal::closed_gate(source, buffers.as_input());
            let port = output_port::dispose();
            let route = request_id.to_route_source(&port);
            world.emit_disposal(route, disposal, roster);
        }

        Ok(())
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<GateRequest<T>>()?;
        clean.cleanup_disposals()?;
        clean.notify_cleaned()
    }

    fn is_reachable(mut reachability: OperationReachability) -> ReachabilityResult {
        if reachability.has_input::<T>()? {
            return Ok(true);
        }

        SingleInputStorage::is_reachable(&mut reachability)
    }
}

pub(crate) struct OperateStaticGate<T, B> {
    buffers: B,
    target: Entity,
    action: Gate,
    _ignore: std::marker::PhantomData<fn(T)>,
}

impl<T, B> OperateStaticGate<T, B> {
    pub(crate) fn new(buffers: B, target: Entity, action: Gate) -> Self {
        Self {
            buffers,
            target,
            action,
            _ignore: Default::default(),
        }
    }
}

impl<T, B> Operation for OperateStaticGate<T, B>
where
    B: Buffering + 'static + Send + Sync,
    T: 'static + Send + Sync,
{
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        world
            .get_entity_mut(self.target)
            .or_broken()?
            .insert(SingleInputStorage::new(source));

        world.entity_mut(source).insert((
            InputBundle::<T>::new(),
            SingleTargetStorage::new(self.target),
            BufferRelationStorage(self.buffers),
            GateActionStorage(self.action),
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
        let Input { session, data, seq } = world.take_input::<T>(source)?;
        let request_id = RequestId { session, source, seq };

        let source_ref = world.get_entity(source).or_broken()?;
        let target = source_ref.get::<SingleTargetStorage>().or_broken()?.get();
        let action = source_ref.get::<GateActionStorage>().or_broken()?.0;
        let buffers = source_ref
            .get::<BufferRelationStorage<B>>()
            .or_broken()?
            .0
            .clone();

        buffers.gate_action(request_id, session, action, world, roster)?;

        let port = output_port::next();
        let route = request_id.to_message_route(&port, target);
        world.give_input(route, data, roster)?;

        if action.is_closed() {
            // When doing a closing, we should emit a disposal because we are
            // cutting off part of the workflow, which may alter the
            // reachability of the terminal node.
            let disposal = Disposal::closed_gate(source, buffers.as_input());
            let port = output_port::dispose();
            let route = request_id.to_route_source(&port);
            world.emit_disposal(route, disposal, roster);
        }

        Ok(())
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<T>()?;
        clean.cleanup_disposals()?;
        clean.notify_cleaned()
    }

    fn is_reachable(mut reachability: OperationReachability) -> ReachabilityResult {
        if reachability.has_input::<T>()? {
            return Ok(true);
        }

        SingleInputStorage::is_reachable(&mut reachability)
    }
}
