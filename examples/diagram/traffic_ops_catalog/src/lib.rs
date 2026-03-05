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

use bevy::prelude::*;
use crossflow::{ConfigExample, NodeBuilderOptions, prelude::*};
use crossflow_diagram_editor::basic_executor::BasicExecutorSetup;
use rand::Rng;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, thread::sleep, time::Duration};
use thiserror::Error;

pub mod movement;
pub use movement::*;

pub mod pedestrian;
pub use pedestrian::*;

pub mod spawn_world;
pub use spawn_world::*;

pub mod traffic;
pub use traffic::*;

pub mod traffic_signal;
pub use traffic_signal::*;

pub mod user_panel;
pub use user_panel::*;

pub mod vehicle;
pub use vehicle::*;

#[derive(StreamPack)]
struct TrafficStateStreams {
    traffic_signal: TrafficSignal,
    obstacles: Obstacles,
    speed_limit: SpeedLimit,
}

#[derive(Clone, Accessor)]
struct TrafficStateAccessor {
    traffic_signal: BufferKey<TrafficSignal>,
    obstacles: BufferKey<Obstacles>,
    speed_limit: BufferKey<SpeedLimit>,
}

#[derive(Clone, Accessor)]
struct TrafficSignalWithObstaclesAccessor {
    traffic_signal: BufferKey<TrafficSignal>,
    obstacles: BufferKey<Obstacles>,
}

#[derive(Clone, Debug, Default, Joined)]
pub struct TrafficState {
    traffic_signal: TrafficSignal,
    obstacles: Obstacles,
    speed_limit: SpeedLimit,
}

#[derive(Clone, Debug, Default, Joined)]
pub struct TrafficSignalWithObstacles {
    traffic_signal: TrafficSignal,
    obstacles: Obstacles,
}

#[derive(Clone, Debug, Error, Serialize, Deserialize, JsonSchema)]
pub enum TripRequestError {
    #[error("Engine start error")]
    EngineStartError,
    #[error("Vehicle check error")]
    VehicleCheckError,
    #[error("Buffer empty error")]
    BufferEmptyError,
    #[error("Next move error")]
    NextMoveError,
    #[error("Vehicle position error")]
    VehiclePositionError,
}

pub fn register(setup: &mut BasicExecutorSetup) {
    let registry = &mut setup.registry;
    let app = &mut setup.app;

    // =========================================================================
    let start_engine_description = "Starts the engine";
    fn start_engine(
        In(distance_to_destination): In<f32>,
        mut vehicle_state: ResMut<VehicleState>,
    ) -> Result<(), TripRequestError> {
        vehicle_state.toggle_engine(true);
        if vehicle_state.set_distance_to_destination(distance_to_destination) {
            info!(
                "Vehicle beginning its trip, distance to destination: {}",
                vehicle_state.distance_left()
            );
            Ok(())
        } else {
            info!(
                "Unable to begin trip, please check the input distance: {}",
                distance_to_destination
            );
            Err(TripRequestError::EngineStartError)
        }
    }
    let start_engine_service = app.spawn_service(start_engine.into_blocking_service());
    registry
        .register_node_builder(
            NodeBuilderOptions::new("start_engine".to_string())
                .with_description(start_engine_description),
            move |builder, _config: ()| builder.create_node(start_engine_service),
        )
        .with_result();

    // =========================================================================
    let begin_vehicle_check_description = "Kicks off a vehicle check to ensure that \
        all components are in ready-state";
    fn begin_vehicle_check(
        In(_): In<()>,
        vehicle_state: Res<VehicleState>,
    ) -> HashMap<String, ReadyState> {
        vehicle_state.checklist().clone()
    }
    let begin_vehicle_service = app.spawn_service(begin_vehicle_check.into_blocking_service());
    registry.register_node_builder(
        NodeBuilderOptions::new("begin_vehicle_check".to_string())
            .with_description(begin_vehicle_check_description),
        move |builder, _config: ()| builder.create_node(begin_vehicle_service),
    );

    // =========================================================================
    let vehicle_check_ready_description = "Toggles individual item on the vehicle checklist \
        to ready-state.";
    registry.register_node_builder(
        NodeBuilderOptions::new("vehicle_check_ready")
            .with_description(vehicle_check_ready_description),
        |builder, _config: ()| {
            builder.create_map_async(|_current_state: ReadyState| {
                let mut rng = rand::rng();
                let sleep_time = rng.random_range(100..500);
                async move {
                    // Sleep for X seconds (randomized) to demonstrate checking different items,
                    // then updates the state to ready.
                    // We don't actually need to know the current state for this example.
                    sleep(Duration::from_millis(sleep_time));
                    ReadyState::Ready
                }
            })
        },
    );

    // =========================================================================
    let validate_vehicle_check_description = "Validates items in vehicle checklist are ready";
    fn validate_vehicle_check(
        // TODO(@xiyuoh) Use Collect operation when it's ready on the diagram editor
        In(checklist): In<Vec<ReadyState>>,
        mut vehicle_state: ResMut<VehicleState>,
    ) -> Result<(), TripRequestError> {
        for item in checklist.iter() {
            if matches!(item, ReadyState::NotReady) {
                // All or nothing check
                return Err(TripRequestError::VehicleCheckError);
            }
        }
        for (_item, state) in vehicle_state.checklist_mut().iter_mut() {
            *state = ReadyState::Ready;
        }

        Ok(())
    }
    let validate_vehicle_check_service =
        app.spawn_service(validate_vehicle_check.into_blocking_service());
    registry
        .register_node_builder(
            NodeBuilderOptions::new("validate_vehicle_check".to_string())
                .with_description(validate_vehicle_check_description),
            move |builder, _config: ()| builder.create_node(validate_vehicle_check_service),
        )
        .with_join()
        .with_result()
        .with_common_response();

    // =========================================================================
    let detect_traffic_signal_description = "Detects traffic signal updates via Query";
    fn detect_traffic_signal(
        In(ContinuousService { key }): ContinuousServiceInput<(), (), TrafficStateStreams>,
        mut orders: ContinuousQuery<(), (), TrafficStateStreams>,
        mut upcoming_signal: EventReader<UpcomingTrafficSignal>,
    ) {
        let Some(mut orders) = orders.get_mut(&key) else {
            return;
        };

        if orders.is_empty() {
            return;
        }

        for signal in upcoming_signal.read() {
            orders.for_each(|order| order.streams().traffic_signal.send(signal.0.clone()));
        }
    }

    let detect_traffic_signal_service =
        app.spawn_continuous_service(PostUpdate, detect_traffic_signal);
    registry.register_node_builder(
        NodeBuilderOptions::new("detect_traffic_signal".to_string())
            .with_description(detect_traffic_signal_description),
        move |builder, _config: ()| builder.create_node(detect_traffic_signal_service),
    );

    // =========================================================================
    let trigger_check_description = "Trigger a check every interval";
    registry.register_node_builder(
        NodeBuilderOptions::new("trigger_check").with_description(trigger_check_description),
        |builder, _config: ()| {
            builder.create_map_async(|_: ()| {
                async move {
                    // Sleep for X seconds to allow events to be detected
                    // TODO(@xiyuoh) Make this configurable from config
                    sleep(Duration::from_millis(500));
                }
            })
        },
    );

    // =========================================================================
    let process_traffic_signal_description = "Process the latest traffic signal \
        upon trigger and make a decision on what the vehicle should do next";
    fn process_traffic_signal(
        In(((), key)): In<((), BufferKey<TrafficSignal>)>,
        access: BufferAccess<TrafficSignal>,
    ) -> Result<MoveVehicle, ()> {
        let Some(signal) = access
            .get(&key)
            .ok()
            .map(|res| res.newest().cloned())
            .flatten()
        else {
            return Err(());
        };

        Ok(determine_next_move_from_traffic_signal(&signal))
    }
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("process_traffic_signal".to_owned())
                .with_description(process_traffic_signal_description),
            |builder, _config: ()| {
                builder.create_node(process_traffic_signal.into_blocking_callback())
            },
        )
        .with_buffer_access()
        .with_result()
        .with_common_response();

    // =========================================================================
    let detect_obstacles_description = "Detects obstacles in range via event reader";
    fn detect_obstacles(
        In(ContinuousService { key }): ContinuousServiceInput<(), (), TrafficStateStreams>,
        mut orders: ContinuousQuery<(), (), TrafficStateStreams>,
        main_vehicle: Query<&Transform, (With<MainVehicle>, Without<Obstacle>)>,
        obstacles: Query<&Transform, (With<Obstacle>, Without<MainVehicle>)>,
        world_limits: Res<WorldLimits>,
    ) {
        let Some(mut orders) = orders.get_mut(&key) else {
            return;
        };
        if orders.is_empty() {
            return;
        }

        let Ok(vehicle) = main_vehicle.single() else {
            return;
        };

        let obstacles = Obstacles(
            obstacles
                .iter()
                .filter(|ob| {
                    // Ignore obstacles behind the main vehicle
                    if ob.translation.y - vehicle.translation.y < world_limits.vehicle_size.1 {
                        return false;
                    }
                    // Ignore obstacles off screen
                    if ob.translation.y > 0.5 * world_limits.window_height {
                        return false;
                    }
                    true
                })
                .map(|t| {
                    ObstacleInRange::new(
                        (t.translation.x - vehicle.translation.x).round() as i32,
                        (t.translation.y - vehicle.translation.y).round() as i32,
                    )
                })
                .collect(),
        );
        orders.for_each(|order| order.streams().obstacles.send(obstacles.clone()));
    }
    let detect_obstacles_service = app.spawn_continuous_service(PostUpdate, detect_obstacles);
    registry.register_node_builder(
        NodeBuilderOptions::new("detect_obstacles".to_string())
            .with_description(detect_obstacles_description),
        move |builder, _config: ()| builder.create_node(detect_obstacles_service),
    );

    // =========================================================================
    let process_obstacles_description = "Process the current obstacles in range upon \
        trigger and make a decision on what the vehicle should do next";
    fn process_obstacles(
        In(((), key)): In<((), BufferKey<Obstacles>)>,
        mut access: BufferAccessMut<Obstacles>,
        user_panel: Res<UserPanel>,
        vehicle_state: Res<VehicleState>,
        world_limits: Res<WorldLimits>,
        main_vehicle: Query<&Transform, With<MainVehicle>>,
    ) -> Result<MoveVehicle, ()> {
        let Ok(vehicle_x) = main_vehicle.single().map(|t| t.translation.x) else {
            return Err(());
        };

        access
            .get_mut(&key)
            .ok()
            .map(|mut res| res.pull_newest())
            .flatten()
            .ok_or(())
            .and_then(|obstacles| {
                determine_next_move_from_obstacles(
                    &obstacles,
                    user_panel.allow_change_lane,
                    vehicle_x,
                    &world_limits,
                    &vehicle_state,
                )
                .map_err(|_| ())
            })
    }
    let process_obstacles_service = app.spawn_service(process_obstacles.into_blocking_service());
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("process_obstacles".to_string())
                .with_description(process_obstacles_description),
            move |builder, _config: ()| builder.create_node(process_obstacles_service),
        )
        .with_buffer_access()
        .with_result()
        .with_common_response();

    // =========================================================================
    let check_change_lane_description = "Check whether changing lane is an option \
        based on obstacles buffer and vehicle's current lane status";
    fn check_change_lane(
        In((next_move, key)): In<(MoveVehicle, BufferKey<Obstacles>)>,
        mut access: BufferAccessMut<Obstacles>,
        user_panel: Res<UserPanel>,
        vehicle_state: Res<VehicleState>,
        world_limits: Res<WorldLimits>,
        main_vehicle: Query<&Transform, With<MainVehicle>>,
    ) -> MoveVehicle {
        // Only check for change lane preference if the vehicle is slowing down
        // and lane change is allowed
        if !user_panel.allow_change_lane {
            return next_move;
        }
        match next_move {
            MoveVehicle::ChangeSpeed(ref acceleration) => {
                if acceleration.y > 0.0 {
                    return next_move;
                }
            }
            _ => return next_move,
        }

        let Ok(vehicle_x) = main_vehicle.single().map(|t| t.translation.x) else {
            return next_move;
        };
        // If we were to change lane, we would first prioritize any lanes the
        // main vehicle is moving towards, followed by the other lane (opposite
        // of where the main vehicle is on currently)
        let Some((to_lane, adjacent_lane_space)) = world_limits
            .other_lane(vehicle_x)
            .zip(world_limits.adjacent_lane_space(vehicle_x))
            .map(|(to_lane, space)| {
                (
                    vehicle_state.changing_lane().clone().unwrap_or(to_lane),
                    space,
                )
            })
        else {
            return next_move;
        };

        let mut adjacent_obstacles = false;
        let limits = &world_limits.obstacle_limits;

        let Some(obstacles) = access
            .get_mut(&key)
            .ok()
            .map(|mut res| res.pull_newest())
            .flatten()
        else {
            return next_move;
        };
        for obstacle in obstacles.0.iter() {
            let (x, y) = (obstacle.offset_x as f32, obstacle.offset_y as f32);
            adjacent_obstacles = adjacent_obstacles
                && limits
                    .obstacle_adjacent(x, y, adjacent_lane_space)
                    .is_some_and(|lane| lane == to_lane);
        }
        if !adjacent_obstacles {
            return MoveVehicle::ChangeLane(Velocity::default_change_lane(to_lane));
        }

        next_move
    }
    let check_change_lane_service = app.spawn_service(check_change_lane.into_blocking_service());
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("check_change_lane".to_string())
                .with_description(check_change_lane_description),
            move |builder, _config: ()| builder.create_node(check_change_lane_service),
        )
        .with_buffer_access()
        .with_common_response();

    // TODO(@xiyuoh) Adjust speed, input MoveVehicle, output MoveVehicle with
    // new/clamped speed

    // =========================================================================
    let join_traffic_signal_and_obstacles_description = "Join the latest traffic signal \
        and obstacles buffers and determine the best move from both input";
    fn join_traffic_signal_and_obstacles(
        In(input): In<TrafficSignalWithObstacles>,
        user_panel: Res<UserPanel>,
        world_limits: Res<WorldLimits>,
        vehicle_state: Res<VehicleState>,
        main_vehicle: Query<&Transform, With<MainVehicle>>,
    ) -> Result<MoveVehicle, TripRequestError> {
        let Ok(vehicle_x) = main_vehicle.single().map(|t| t.translation.x) else {
            return Err(TripRequestError::VehiclePositionError);
        };

        // Since this is a Join operation, we have input from both TrafficSignal
        // and Obstacles. We can determine what is the next best move from both.

        // TODO(@xiyuoh) Enable toggling to hide all pedestrians, to showcase how
        // without Obstacles (aka pedestrians) the car won't be able to proecss
        // anything because Join wouldn't be able to complete.
        // TODO(@xiyuoh) hmm this is not working, vehicle still moves and just
        // follows the traffic signal, figure out why.

        let next_move_for_signal = determine_next_move_from_traffic_signal(&input.traffic_signal);

        let Ok(next_move_for_obstacles) = determine_next_move_from_obstacles(
            &input.obstacles,
            user_panel.allow_change_lane,
            vehicle_x,
            &world_limits,
            &vehicle_state,
        ) else {
            return Err(TripRequestError::NextMoveError);
        };

        Ok(choose_best_move(
            &next_move_for_signal,
            &next_move_for_obstacles,
        ))
    }
    let join_traffic_signal_and_obstacles_service =
        app.spawn_service(join_traffic_signal_and_obstacles.into_blocking_service());
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("join_traffic_signal_and_obstacles".to_string())
                .with_description(join_traffic_signal_and_obstacles_description),
            move |builder, _config: ()| {
                builder.create_node(join_traffic_signal_and_obstacles_service)
            },
        )
        .with_result()
        .with_join()
        .with_common_response();

    // =========================================================================
    let listen_traffic_signal_and_obstacles_description = "Listen to both traffic \
        signal and obstacles buffers and determine the next move based on the \
        combination of buffer activated.";
    fn listen_traffic_signal_and_obstacles(
        In(keys): In<TrafficSignalWithObstaclesAccessor>,
        traffic_signal_access: BufferAccess<TrafficSignal>,
        mut obstacles_access: BufferAccessMut<Obstacles>,
        user_panel: Res<UserPanel>,
        vehicle_state: Res<VehicleState>,
        world_limits: Res<WorldLimits>,
        main_vehicle: Query<&Transform, With<MainVehicle>>,
    ) -> Result<MoveVehicle, TripRequestError> {
        let Ok(vehicle_x) = main_vehicle.single().map(|t| t.translation.x) else {
            return Err(TripRequestError::VehiclePositionError);
        };

        let signal_next_move = traffic_signal_access
            .get(&keys.traffic_signal)
            .ok()
            .map(|res| res.newest().cloned())
            .flatten()
            .map(|signal| determine_next_move_from_traffic_signal(&signal));

        // If the next move determined from the TrafficSignal is to stop, we will
        // just return that without having to access Obstacles
        if let Some(signal_move) = signal_next_move
            .as_ref()
            .filter(|mv| matches!(mv, MoveVehicle::Stop))
        {
            return Ok(signal_move.clone());
        }

        let obstacles_next_move = obstacles_access
            .get_mut(&keys.obstacles)
            .ok()
            .map(|mut res| res.pull_newest())
            .flatten()
            .ok_or(TripRequestError::BufferEmptyError)
            .and_then(|obstacles| {
                determine_next_move_from_obstacles(
                    &obstacles,
                    user_panel.allow_change_lane,
                    vehicle_x,
                    &world_limits,
                    &vehicle_state,
                )
            })
            .ok();

        if signal_next_move.is_none() && obstacles_next_move.is_none() {
            return Err(TripRequestError::NextMoveError);
        } else if let Some(next_move) = signal_next_move
            .as_ref()
            .xor(obstacles_next_move.as_ref())
            .cloned()
        {
            return Ok(next_move);
        }

        Ok(choose_best_move(
            &signal_next_move.unwrap(),
            &obstacles_next_move.unwrap(),
        ))
    }
    let listen_traffic_signal_and_obstacles_service =
        app.spawn_service(listen_traffic_signal_and_obstacles.into_blocking_service());
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("listen_traffic_signal_and_obstacles".to_string())
                .with_description(listen_traffic_signal_and_obstacles_description),
            move |builder, _config: ()| {
                builder.create_node(listen_traffic_signal_and_obstacles_service)
            },
        )
        .with_listen()
        .with_result()
        .with_common_response();

    // =========================================================================
    let move_vehicle_description = "Move vehicle";
    fn move_vehicle(
        In(move_vehicle): In<MoveVehicle>,
        mut commands: Commands,
        mut vehicle_state: ResMut<VehicleState>,
        vehicle_velocity: Query<Entity, (With<MainVehicle>, With<Velocity>)>,
    ) {
        let Ok(e) = vehicle_velocity.single() else {
            return;
        };
        let e_cmds = commands.entity(e);
        vehicle_state.try_move(e_cmds, move_vehicle);
    }
    let move_vehicle_service = app.spawn_service(move_vehicle.into_blocking_service());
    registry.register_node_builder(
        NodeBuilderOptions::new("move_vehicle".to_string())
            .with_description(move_vehicle_description),
        move |builder, _config: ()| builder.create_node(move_vehicle_service),
    );

    // =========================================================================
    let destination_reached_description = "Check if the main vehicle has reached the destination";
    fn destination_reached(In(_): In<()>, vehicle_state: Res<VehicleState>) -> Result<(), ()> {
        if vehicle_state.at_destination() {
            Ok(())
        } else {
            Err(())
        }
    }
    let destination_reached_service =
        app.spawn_service(destination_reached.into_blocking_service());
    registry
        .register_node_builder(
            NodeBuilderOptions::new("destination_reached".to_string())
                .with_description(destination_reached_description),
            move |builder, _config: ()| builder.create_node(destination_reached_service),
        )
        .with_result();

    // =========================================================================
    let stop_engine_description = "Stop engine and reset vehicle";
    fn stop_engine(
        In(_): In<()>,
        mut commands: Commands,
        mut vehicle_state: ResMut<VehicleState>,
        vehicle_velocity: Query<Entity, (With<MainVehicle>, With<Velocity>)>,
    ) {
        let Ok(e) = vehicle_velocity.single() else {
            return;
        };
        let e_cmds = commands.entity(e);
        vehicle_state
            .try_move(e_cmds, MoveVehicle::Stop)
            .toggle_engine(false)
            .reset();
        info!("Vehicle successfully completed its trip!");
    }
    let stop_engine_service = app.spawn_service(stop_engine.into_blocking_service());
    registry.register_node_builder(
        NodeBuilderOptions::new("stop_engine".to_string())
            .with_description(stop_engine_description),
        move |builder, _config: ()| builder.create_node(stop_engine_service),
    );

    // =========================================================================
    let trip_error_description = "Log trip errors";
    registry.register_node_builder(
        NodeBuilderOptions::new("trip_error").with_description(trip_error_description),
        |builder, _config: ()| {
            builder.create_map_block(|err: TripRequestError| {
                error!("{:?}", err);
            })
        },
    );
}

fn choose_best_move(move_a: &MoveVehicle, move_b: &MoveVehicle) -> MoveVehicle {
    move_a.min(move_b)
}

fn determine_next_move_from_traffic_signal(signal: &TrafficSignal) -> MoveVehicle {
    // TODO(@xiyuoh) only process changes if vehicle is within X distance of traffic light
    return match signal {
        TrafficSignal::Green => MoveVehicle::Forward(Velocity::default_forward()),
        TrafficSignal::Yellow => MoveVehicle::ChangeSpeed(Acceleration::default_slow_down()), // slow down for yellow light
        TrafficSignal::Red => MoveVehicle::Stop,
    };
}

fn determine_next_move_from_obstacles(
    obstacles: &Obstacles,
    allow_change_lane: bool,
    vehicle_x: f32,
    world_limits: &WorldLimits,
    vehicle_state: &VehicleState,
) -> Result<MoveVehicle, TripRequestError> {
    let limits = &world_limits.obstacle_limits;
    let mut next_move = MoveVehicle::Forward(Velocity::default_forward());
    for obstacle in obstacles.0.iter() {
        let (x, y) = (obstacle.offset_x as f32, obstacle.offset_y as f32);
        // Ignore obstacles that are behind the vehicle
        if limits.obstacle_behind(y) {
            continue;
        }
        let mut possible_move = next_move.clone();

        if limits.immediate_obstacle(x, y) {
            possible_move = MoveVehicle::Stop;
        } else if limits.obstacle_ahead(x, y) {
            possible_move = MoveVehicle::ChangeSpeed(Acceleration::default_slow_down());
        }

        next_move = choose_best_move(&possible_move, &next_move);
        if matches!(next_move, MoveVehicle::Stop) {
            break;
        }
    }

    Ok(next_move)
}
