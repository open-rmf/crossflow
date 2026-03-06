/*
 * Copyright (C) 2026 Open Source Robotics Foundation
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

use crate::{RequestId, OperationRoster, ManageSession, DeferredRoster, SessionEvent};

use bevy_ecs::{
    prelude::{Entity, Resource, World, Commands},
    system::Command,
};

use std::collections::{HashSet, VecDeque};


/// This resource lets you manage the debugging behavior of workflow execution.
/// You can freely change any fields inside this resource and the debugger will
/// respond by pausing or releasing inputs accordingly.
///
/// If you remove this resource from the world entirely while some inputs are
/// paused, they will
#[derive(Debug, Default, Clone, Resource)]
pub struct Debug {
    /// When a message gets sent to an operation in this set, the session of that
    /// message will be added to the paused_sessions list.
    pub breakpoints: HashSet<Entity>,

    /// Sessions that have debugging active. If a breakpoint is hit for any of
    /// these sessions then that session will be added to paused_sessions.
    ///
    /// This is private to reduce API confusion between debugging sessions and
    /// paused sessions.
    debug_sessions: HashSet<Entity>,

    /// When a message is sent in a session in this set, it will remain in
    /// the input storage of its target operation until the user steps it forward.
    ///
    /// This is private to reduce API confusion between debugging sessions and
    /// paused sessions.
    paused_sessions: HashSet<Entity>,

    session_changes: VecDeque<SessionPauseChange>,
}

#[derive(Debug, Clone)]
enum SessionPauseChange {
    Paused(Entity),
    Unpaused(Entity),
    Breakpoint {
        session: Entity,
        breakpoint: Entity,
    },
}

impl Debug {
    /// Use this to deactivate debugging, which will unpause all sessions and
    /// prevent breakpoints from triggering any pauses.
    ///
    /// Any breakpoints that are set will remain set and become effective again
    /// if debugging gets turned on for any of their sessions.
    pub fn deactivate(&mut self) {
        self.debug_sessions.clear();
        self.session_changes.extend(
            self.paused_sessions.drain()
            .map(|session| SessionPauseChange::Unpaused(session))
        );
    }

    /// Start debugging for a session. Optionally pause the session immediately.
    pub fn start_debugging_for(&mut self, session: Entity, pause_immediately: bool) {
        self.debug_sessions.insert(session);
        if pause_immediately {
            self.pause(session);
        }
    }

    /// Turn off debugging for a session and unpause it.
    pub fn stop_debugging_for(&mut self, session: Entity) {
        self.debug_sessions.remove(&session);
        self.paused_sessions.remove(&session);
    }

    /// Pause a session immediately. Note that this does not activate debugging
    /// for the session, so the session will not respond to breakpoints. It will
    /// simply remain paused until it gets unpaused.
    pub fn pause(&mut self, session: Entity) {
        self.session_changes.push_back(SessionPauseChange::Paused(session));
        self.paused_sessions.insert(session);
    }

    /// Unpause a session immediately. Note that this does not deactivate debugging,
    /// so if debugging is enabled for the session then it will pause again when
    /// it reaches a breakpoint.
    pub fn unpause(&mut self, session: Entity) {
        self.paused_sessions.remove(&session);
        self.session_changes.push_back(SessionPauseChange::Unpaused(session));
    }

    /// Check if any debugging is active.
    pub fn is_active(&self) -> bool {
        let is_paused = !self.paused_sessions.is_empty();
        let can_pause = !self.debug_sessions.is_empty() && !self.breakpoints.is_empty();
        is_paused || can_pause
    }

    /// If the target is a breakpoint then the session will be added to paused
    /// sessions.
    pub(crate) fn evaluate_break(&mut self, session: Entity, target: Entity, world: &World) {
        let mut is_debug_session = None;
        for debug_session in &self.debug_sessions {
            if world.is_descendent_session(*debug_session, session) {
                is_debug_session = Some(*debug_session);
                break;
            }
        }

        if let Some(debug_session) = is_debug_session {
            if self.breakpoints.contains(&target) {
                if self.paused_sessions.insert(debug_session) {
                    self.session_changes.push_back(SessionPauseChange::Breakpoint {
                        session,
                        breakpoint: target,
                    });
                }
            }
        }
    }

    /// Return true if the request belongs to a paused session.
    pub fn is_paused(&self, session: Entity, world: &World) -> bool {
        for paused_session in &self.paused_sessions {
            if world.is_descendent_session(*paused_session, session) {
                return true;
            }
        }

        false
    }

    pub(crate) fn notify_session_changes(&mut self, world: &mut World) {
        while let Some(change) = self.session_changes.pop_front() {
            match change {
                SessionPauseChange::Paused(e) => {
                    SessionEvent::paused_by_user(e, world);
                }
                SessionPauseChange::Unpaused(e) => {
                    SessionEvent::unpaused(e, world);
                }
                SessionPauseChange::Breakpoint { session, breakpoint } => {
                    SessionEvent::paused_by_breakpoint(session, breakpoint, world);
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, Resource)]
pub(crate) struct DebugRoster {
    deferrals: Vec<RequestId>,
    allow: HashSet<RequestId>,
}

impl DebugRoster {
    /// Check whether an operation is allowed to take this request. This is
    /// assumes the request belongs to a paused session. Do not use this function
    /// for requests whose sessions are not paused or else the input will be
    /// forced to pause.
    pub(crate) fn is_allowed(&mut self, id: RequestId) -> bool {
        if self.allow.remove(&id) {
            return true;
        }

        if self.deferrals.contains(&id) {
            return false;
        }

        self.deferrals.push(id);
        false
    }

    pub(crate) fn pop_next_in_session(
        &mut self,
        session: Entity,
        operation: Option<Entity>,
        world: &mut World,
    ) {
        world.get_resource_or_init::<DeferredRoster>();
        world.resource_scope::<DeferredRoster, _>(|world, mut roster| {
            if let Some(index) = self.deferrals.iter().position(
                |req| {
                    let is_in_session = world.is_descendent_session(session, req.session);
                    if let Some(op) = operation {
                        req.source == op
                    } else {
                        is_in_session
                    }
                }) {
                let next = self.deferrals.remove(index);
                self.allow.insert(next);
                roster.queue(next.source);
            }
        });
    }

    /// For any sessions that have been unpaused, release their inputs.
    pub(crate) fn release_unpaused(
        &mut self,
        world: &mut World,
        roster: &mut OperationRoster,
    ) {
        world.get_resource_or_init::<Debug>();
        world.resource_scope::<Debug, _>(|world, debug| {
            self.deferrals.retain(|req| {
                let paused = debug.is_paused(req.session, world);
                if !paused {
                    roster.queue(req.source);
                }

                paused
            });
        });
    }
}

pub struct DebugStep {
    pub session: Entity,
    pub operation: Option<Entity>,
}

impl Command for DebugStep {
    fn apply(self, world: &mut World) -> () {
        world.get_resource_or_init::<DebugRoster>();
        world.resource_scope::<DebugRoster, _>(|world, mut debug_roster| {
            debug_roster.pop_next_in_session(self.session, self.operation, world);
        });
    }
}

pub trait DebugStepExt {
    /// Instruct a paused session to step forward, i.e. ingest the oldest paused
    /// message.
    fn debug_step(&mut self, session: Entity);

    /// Instruct a specific operation of a paused session to step forward, i.e.
    /// ingest its oldest paused message.
    fn debug_step_for_operation(&mut self, session: Entity, operation: Entity);
}

impl<'w, 's> DebugStepExt for Commands<'w, 's> {
    fn debug_step(&mut self, session: Entity) {
        self.queue(DebugStep {
            session,
            operation: None,
        });
    }

    fn debug_step_for_operation(&mut self, session: Entity, operation: Entity) {
        self.queue(DebugStep {
            session,
            operation: Some(operation),
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, testing::*};
    use std::collections::HashSet;

    #[test]
    fn test_debug_step() {
        let mut context = TestingContext::minimal_plugins();

        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        for _ in 0..3 {
            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            senders.push(sender);
            receivers.push(receiver);
        }

        let mut breakpoints = HashSet::new();
        let workflow = context.spawn_io_workflow(|scope, builder| {
            let first = builder.create_map_block({
                let senders = senders.clone();
                move |_: ()| {
                    let _ = senders[0].send(());
                }
            });
            breakpoints.insert(first.input.id());

            builder.connect(scope.start, first.input);
            builder
                .chain(first.output)
                .map_block({
                    let senders = senders.clone();
                    move |_: ()| {
                        let _ = senders[1].send(());
                    }
                })
                .map_block({
                    let senders = senders.clone();
                    move |_: ()| {
                        let _ = senders[2].send(());
                    }
                })
                .connect(scope.terminate);
        });

        // ----- Test: Reach a breakpoint and step through one operation at a time
        let Capture { mut outcome, session, .. } =
            context.command(|commands| commands.request((), workflow).capture());

        let mut debug = context.app.world_mut().get_resource_or_init::<Debug>();
        debug.start_debugging_for(session, false);
        debug.breakpoints = breakpoints;

        context.run_with_conditions(&mut outcome, 10);
        assert!(receivers[0].try_recv().is_err());
        assert!(receivers[1].try_recv().is_err());
        assert!(receivers[2].try_recv().is_err());

        context.command(|commands| commands.debug_step(session));
        context.run_with_conditions(&mut outcome, 10);
        assert!(receivers[0].try_recv().is_ok());
        assert!(receivers[1].try_recv().is_err());
        assert!(receivers[2].try_recv().is_err());

        context.command(|commands| commands.debug_step(session));
        context.run_with_conditions(&mut outcome, 10);
        assert!(receivers[0].try_recv().is_err());
        assert!(receivers[1].try_recv().is_ok());
        assert!(receivers[2].try_recv().is_err());

        context.command(|commands| commands.debug_step(session));
        context.run_with_conditions(&mut outcome, 10);
        assert!(receivers[0].try_recv().is_err());
        assert!(receivers[1].try_recv().is_err());
        assert!(receivers[2].try_recv().is_ok());

        context.app.world_mut().resource_mut::<Debug>().stop_debugging_for(session);
        context.run_with_conditions(&mut outcome, 1);
        outcome.try_recv().unwrap().unwrap();

        // ----- Test: Reach a breakpoint, take one step, and then unpause
        let Capture { mut outcome, session, .. } =
            context.command(|commands| commands.request((), workflow).capture());

        let mut debug = context.app.world_mut().get_resource_or_init::<Debug>();
        debug.start_debugging_for(session, false);
        // Note: The same breakpoint as before still applies

        // Make sure all receivers are currently empty.
        assert!(receivers[0].try_recv().is_err());
        assert!(receivers[1].try_recv().is_err());
        assert!(receivers[2].try_recv().is_err());

        context.run_with_conditions(&mut outcome, 10);
        assert!(receivers[0].try_recv().is_err());
        assert!(receivers[1].try_recv().is_err());
        assert!(receivers[2].try_recv().is_err());

        context.command(|commands| commands.debug_step(session));
        context.run_with_conditions(&mut outcome, 10);
        assert!(receivers[0].try_recv().is_ok());
        assert!(receivers[1].try_recv().is_err());
        assert!(receivers[2].try_recv().is_err());

        context.app.world_mut().resource_mut::<Debug>().stop_debugging_for(session);
        context.run_with_conditions(&mut outcome, 1);
        assert!(receivers[0].try_recv().is_err());
        assert!(receivers[1].try_recv().is_ok());
        assert!(receivers[2].try_recv().is_ok());
        outcome.try_recv().unwrap().unwrap();
    }

    #[test]
    fn test_debug_step_for_buffers_and_cycles() {
        let mut context = TestingContext::minimal_plugins();

        let (sum_sender, mut sum_receiver) = tokio::sync::mpsc::unbounded_channel();

        let mut breakpoints = HashSet::new();
        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffer_a = builder.create_buffer(Default::default());
            let buffer_b = builder.create_buffer(Default::default());
            breakpoints.insert(buffer_a.id());

            let (fork_input, fork_outputs) = builder.create_fork_clone();
            builder.connect(scope.start, fork_input);

            let clone_a = fork_outputs.clone_output(builder);
            builder
                .chain(clone_a)
                .map_block(|x| 2.0*x)
                .connect(buffer_a.input_slot());

            let clone_b = fork_outputs.clone_output(builder);
            builder
                .chain(clone_b)
                .map_block(|x| 3.0*x)
                .connect(buffer_b.input_slot());

            builder
                .join((buffer_a, buffer_b))
                .map_block(move |(a, b)| {
                    let sum = a + b;
                    let _ = sum_sender.send(sum);
                    if sum > 100.0 {
                        Ok(sum)
                    } else {
                        Err(sum)
                    }
                })
                .fork_result(
                    |ok: Chain<_>| ok.connect(scope.terminate),
                    |err: Chain<_>| err.connect(fork_input),
                );
        });

        let Capture { mut outcome, session, .. } =
            context.command(|commands| commands.request(1.0, workflow).capture());

        let mut debug = context.app.world_mut().get_resource_or_init::<Debug>();
        debug.start_debugging_for(session, false);
        debug.breakpoints = breakpoints;

        let mut step_up_to_sum = move |
            outcome: &mut Outcome<f64>,
            context: &mut TestingContext,
            expected_sum: f64
        | {
            // Run until the breakpoint
            context.run_with_conditions(outcome, 3);

            // Step buffer_a forward
            context.command(|commands| commands.debug_step(session));
            context.run_with_conditions(outcome, 3);
            assert!(sum_receiver.try_recv().is_err());

            // Step buffer_b forward
            context.command(|commands| commands.debug_step(session));
            context.run_with_conditions(outcome, 3);
            assert!(sum_receiver.try_recv().is_err());

            // Step join forward
            context.command(|commands| commands.debug_step(session));
            context.run_with_conditions(outcome, 3);
            assert!(sum_receiver.try_recv().is_err());

            // Step join forward again because it gets a notification from each
            // buffer. Since the buffers are empty, the join won't do anything.
            context.command(|commands| commands.debug_step(session));
            context.run_with_conditions(outcome, 3);
            assert!(sum_receiver.try_recv().is_err());

            // Step sum map forward
            context.command(|commands| commands.debug_step(session));
            context.run_with_conditions(outcome, 3);
            assert_eq!(sum_receiver.try_recv().unwrap(), expected_sum);
        };

        // Step up to the first sum operation
        step_up_to_sum(&mut outcome, &mut context, 5.0);

        // Unpause until the breakpoint is reached again, then step up to the sum
        context.app.world_mut().resource_mut::<Debug>().unpause(session);
        step_up_to_sum(&mut outcome, &mut context, 25.0);

        // Unpause until the breakpoint is reached again, then step up to the sum
        context.app.world_mut().resource_mut::<Debug>().unpause(session);
        step_up_to_sum(&mut outcome, &mut context, 125.0);

        // Unpause and let the workflow finish
        context.app.world_mut().resource_mut::<Debug>().unpause(session);
        context.run_with_conditions(&mut outcome, 1);
        assert_eq!(outcome.try_recv().unwrap().unwrap(), 125.0);
    }
}
