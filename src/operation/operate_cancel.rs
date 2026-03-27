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

use bevy_ecs::prelude::Component;

use crate::{
    Cancellation, InScope, Input, InputBundle, ManageCancellation, ManageInput, Operation,
    OperationCleanup, OperationReachability, OperationRequest, OperationResult, OperationSetup,
    OrBroken, ReachabilityResult, RouteSource, ScopeEndpoints, SingleInputStorage,
    SingleTargetStorage, output_port,
};

/// Create an operation that will cancel a scope. The incoming message will be
/// included in the cancellation data as a [`String`]. The incoming message type
/// must support the [`ToString`] trait.
///
/// To trigger a cancellation for types that do not support [`ToString`], convert
/// the message to a trigger and send it to [`OperateQuietCancel`].
pub struct OperateCancel<T: 'static + Send + Sync + ToString> {
    implicit: bool,
    _ignore: std::marker::PhantomData<fn(T)>,
}

impl<T> OperateCancel<T>
where
    T: 'static + Send + Sync + ToString,
{
    /// Make an explicit cancelling operation. When an explicit cancel is
    /// reachable, the scope will be kept alive.
    pub fn explicit() -> Self {
        Self {
            implicit: false,
            _ignore: Default::default(),
        }
    }

    /// Make an implicit cancelling operation. An implicit cancel has no effect
    /// on reachability.
    pub fn implicit() -> Self {
        Self {
            implicit: true,
            _ignore: Default::default(),
        }
    }
}

#[derive(Component)]
struct IsImplicit(bool);

impl<T> Operation for OperateCancel<T>
where
    T: 'static + Send + Sync + ToString,
{
    fn setup(self, setup: OperationSetup) -> OperationResult {
        setup_cancel_operation::<T>(setup, self.implicit)
    }

    fn execute(
        OperationRequest {
            source,
            world,
            roster,
        }: OperationRequest,
    ) -> OperationResult {
        let Input { session, data, seq } = world.take_input::<T>(source).or_broken()?;
        let cancellation = Cancellation::triggered(source, Some(data.to_string()));

        let route = RouteSource {
            session,
            source,
            seq,
            port: &output_port::cancel(),
        };
        world.emit_scope_cancel(route, session, cancellation, roster);
        Ok(())
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<T>()?;
        clean.notify_cleaned()
    }

    fn is_reachable(mut r: OperationReachability) -> ReachabilityResult {
        if r.has_input::<T>()? {
            return Ok(true);
        }

        let is_implicit = r.world.get::<IsImplicit>(r.source).or_broken()?.0;
        if is_implicit {
            // If this is an implicit cancellation then stop the reachability
            // check here. If it does not already have an input then we consider
            // it to be unreachable.
            return Ok(false);
        }

        SingleInputStorage::is_reachable(&mut r)
    }
}

/// Create an operation that will cancel a scope. This operation only accepts
/// trigger `()` inputs. There will be no information included in the
/// cancellation message except that the cancellation was triggered at this node.
pub struct OperateQuietCancel {
    implicit: bool,
}

impl OperateQuietCancel {
    pub fn explicit() -> Self {
        Self { implicit: false }
    }

    pub fn implicit() -> Self {
        Self { implicit: true }
    }
}

impl Operation for OperateQuietCancel {
    fn setup(self, setup: OperationSetup) -> OperationResult {
        setup_cancel_operation::<()>(setup, self.implicit)
    }

    fn execute(
        OperationRequest {
            source,
            world,
            roster,
        }: OperationRequest,
    ) -> OperationResult {
        let Input { session, seq, .. } = world.take_input::<()>(source).or_broken()?;

        let cancellation = Cancellation::triggered(source, None);
        let route = RouteSource {
            session,
            source,
            seq,
            port: &output_port::cancel(),
        };
        world.emit_scope_cancel(route, session, cancellation, roster);
        Ok(())
    }

    fn cleanup(mut clean: OperationCleanup) -> OperationResult {
        clean.cleanup_inputs::<()>()?;
        clean.notify_cleaned()
    }

    fn is_reachable(mut r: OperationReachability) -> ReachabilityResult {
        if r.has_input::<()>()? {
            return Ok(true);
        }

        let is_implicit = r.world.get::<IsImplicit>(r.source).or_broken()?.0;
        if is_implicit {
            // If this is an implicit cancellation then stop the reachability
            // check here. If it does not already have an input then we consider
            // it to be unreachable.
            return Ok(false);
        }

        SingleInputStorage::is_reachable(&mut r)
    }
}

fn setup_cancel_operation<T: 'static + Send + Sync>(
    OperationSetup { source, world }: OperationSetup,
    is_implicit: bool,
) -> OperationResult {
    let scope = **world.get::<InScope>(source).or_broken()?;

    let cancel_target = world.get::<ScopeEndpoints>(scope).or_broken()?.cancel_scope;
    world
        .get_mut::<SingleInputStorage>(cancel_target)
        .or_broken()?
        .add(source);

    world.entity_mut(source).insert((
        InputBundle::<T>::new(),
        SingleTargetStorage::new(cancel_target),
        IsImplicit(is_implicit),
    ));
    Ok(())
}
