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

use bevy_ecs::prelude::{Component, Entity, World};

use crate::{
    Disposal, ForkTargetStorage, Input, InputBundle, ManageDisposal, ManageInput, Operation,
    OperationCleanup, OperationReachability, OperationRequest, OperationResult, OperationRoster,
    OperationSetup, OrBroken, ReachabilityResult, SingleInputStorage, Seq, output_port,
    RequestId,
};

use smallvec::{smallvec, SmallVec};

use thiserror::Error as ThisError;

pub struct Branching<Input, Outputs, F> {
    activator: F,
    // TODO(@mxgrey): Consider expanding support for these port identifiers
    // beyond string literals. Maybe we should support entire OutputKeys. The
    // main question is how to do this such that there is no overhead from
    // heap allocations in the hot loop.
    ports: SmallVec<[&'static str; 8]>,
    targets: ForkTargetStorage,
    _ignore: std::marker::PhantomData<fn(Input, Outputs)>,
}

#[allow(clippy::type_complexity)]
pub(crate) fn make_result_branching<T, E>(
    targets: ForkTargetStorage,
) -> Branching<Result<T, E>, (T, E), fn(Result<T, E>) -> (BranchResult<T>, BranchResult<E>)> {
    Branching {
        activator: branch_result,
        ports: smallvec!["ok", "err"],
        targets,
        _ignore: Default::default(),
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn make_option_branching<T>(
    targets: ForkTargetStorage,
) -> Branching<Option<T>, (T, ()), fn(Option<T>) -> (BranchResult<T>, BranchResult<()>)> {
    Branching {
        activator: branch_option,
        ports: smallvec!["some", "none"],
        targets,
        _ignore: Default::default(),
    }
}

#[derive(Component, Clone)]
struct BranchingActivatorStorage<F: 'static + Send + Sync + Copy> {
    activator: F,
    ports: SmallVec<[&'static str; 8]>,
}

impl<InputT, Outputs, F> Operation for Branching<InputT, Outputs, F>
where
    InputT: 'static + Send + Sync,
    Outputs: Branchable,
    F: Fn(InputT) -> Outputs::Activation + Copy + 'static + Send + Sync,
{
    fn setup(self, OperationSetup { source, world }: OperationSetup) -> OperationResult {
        for target in &self.targets.0 {
            world
                .get_entity_mut(*target)
                .or_broken()?
                .insert(SingleInputStorage::new(source));
        }
        world.entity_mut(source).insert((
            self.targets,
            InputBundle::<InputT>::new(),
            BranchingActivatorStorage {
                activator: self.activator,
                ports: self.ports,
            },
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
            data: input,
            seq,
        } = world.take_input::<InputT>(source)?;
        let BranchingActivatorStorage::<F> { activator, ports } = world
            .get_entity_mut(source)
            .or_broken()?
            .get()
            .cloned()
            .or_broken()?;

        let activation = activator(input);
        Outputs::activate(session, activation, source, seq, &ports, world, roster)
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<InputT>()?;
        clean.cleanup_disposals()?;
        clean.notify_cleaned()
    }

    fn is_reachable(mut reachability: OperationReachability) -> ReachabilityResult {
        if reachability.has_input::<InputT>()? {
            return Ok(true);
        }

        SingleInputStorage::is_reachable(&mut reachability)
    }
}

pub trait Branchable {
    type Activation;

    fn activate<'a>(
        session: Entity,
        activation: Self::Activation,
        source: Entity,
        seq: Seq,
        ports: &[&'static str],
        world: &'a mut World,
        roster: &'a mut OperationRoster,
    ) -> OperationResult;
}

pub type BranchResult<T> = Result<T, Option<anyhow::Error>>;

impl<A, B> Branchable for (A, B)
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
{
    type Activation = (BranchResult<A>, BranchResult<B>);

    fn activate<'a>(
        session: Entity,
        (a, b): Self::Activation,
        source: Entity,
        seq: Seq,
        ports: &[&'static str],
        world: &'a mut World,
        roster: &'a mut OperationRoster,
    ) -> OperationResult {
        let targets = world.get::<ForkTargetStorage>(source).or_broken()?;
        #[allow(clippy::get_first)]
        let target_a = *targets.0.get(0).or_broken()?;
        let target_b = *targets.0.get(1).or_broken()?;
        let req = RequestId { session, source, seq };

        let port_a = output_port::name_str(ports[0]);
        match a {
            Ok(a) => {
                let route = req.to_message_route(&port_a, target_a);
                world.give_input(route, a, roster)?;
            },
            Err(reason) => {
                let route = req.to_route_source(&port_a);
                let disposal = Disposal::branching(source, target_a, reason);
                world.emit_disposal(route, disposal, roster);
            }
        }

        let port_b = output_port::name_str(ports[1]);
        match b {
            Ok(b) => {
                let route = req.to_message_route(&port_b, target_b);
                world.give_input(route, b, roster)?;
            },
            Err(reason) => {
                let route = req.to_route_source(&port_b);
                let disposal = Disposal::branching(source, target_b, reason);
                world.emit_disposal(route, disposal, roster);
            }
        }

        Ok(())
    }
}

#[derive(ThisError, Debug)]
#[error("An Ok value was received, so the Err branch is being disposed")]
pub struct OkInput;

#[derive(ThisError, Debug)]
#[error("An Err value was received, so the Ok branch is being disposed")]
pub struct ErrInput;

fn branch_result<T, E>(input: Result<T, E>) -> (BranchResult<T>, BranchResult<E>) {
    match input {
        Ok(value) => (Ok(value), Err(Some(OkInput.into()))),
        Err(err) => (Err(Some(ErrInput.into())), Ok(err)),
    }
}

#[derive(ThisError, Debug)]
#[error("A Some value was received, so the None branch is being disposed")]
pub struct SomeInput;

#[derive(ThisError, Debug)]
#[error("A None value was received, so the Some branch is being disposed")]
pub struct NoneInput;

fn branch_option<T>(input: Option<T>) -> (BranchResult<T>, BranchResult<()>) {
    match input {
        Some(value) => (Ok(value), Err(Some(SomeInput.into()))),
        None => (Err(Some(NoneInput.into())), Ok(())),
    }
}
