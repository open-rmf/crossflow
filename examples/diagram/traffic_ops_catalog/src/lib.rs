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
        Blocking {
            request: distance_to_destination,
            ..
        }: Blocking<f32>,
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
    let start_engine_service = app.spawn_service(start_engine);
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
        Blocking { .. }: Blocking<()>,
        vehicle_state: Res<VehicleState>,
    ) -> HashMap<String, ReadyState> {
        vehicle_state.checklist().clone()
    }
    let begin_vehicle_service = app.spawn_service(begin_vehicle_check);
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
        Blocking {
            request: checklist, ..
        }: Blocking<Vec<ReadyState>>,
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
    let validate_vehicle_check_service = app.spawn_service(validate_vehicle_check);
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
    let detect_traffic_signal_description = "Detects traffic signal updates via events";
    fn detect_traffic_signal(
        srv: ContinuousService<(), (), TrafficStateStreams>,
        mut orders: ContinuousQuery<(), (), TrafficStateStreams>,
        mut upcoming_signal: EventReader<UpcomingTrafficSignal>,
    ) {
        let Some(mut orders) = orders.get_mut(&srv.key) else {
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
        Blocking {
            request: (_, key),
            id,
            ..
        }: Blocking<((), BufferKey<TrafficSignal>)>,
        mut access: BufferAccessMut<TrafficSignal>,
    ) -> Result<MoveVehicle, ()> {
        let Some(signal) = access
            .get_mut(id, &key)
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
            |builder, _config: ()| builder.create_node(process_traffic_signal.into_callback()),
        )
        .with_buffer_access()
        .with_result()
        .with_common_response();

    // =========================================================================
    let detect_obstacles_description = "Detects obstacles in range via query";
    fn detect_obstacles(
        srv: ContinuousService<(), (), TrafficStateStreams>,
        mut orders: ContinuousQuery<(), (), TrafficStateStreams>,
        main_vehicle: Query<&Transform, (With<MainVehicle>, Without<Obstacle>)>,
        obstacles: Query<&Transform, (With<Obstacle>, Without<MainVehicle>)>,
        world_limits: Res<WorldLimits>,
    ) {
        let Some(mut orders) = orders.get_mut(&srv.key) else {
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
        if obstacles.0.is_empty() {
            return;
        }

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
        Blocking {
            request: (_, key),
            id,
            ..
        }: Blocking<((), BufferKey<Obstacles>)>,
        mut access: BufferAccessMut<Obstacles>,
        world_limits: Res<WorldLimits>,
    ) -> Result<MoveVehicle, ()> {
        access
            .get_mut(id, &key)
            .ok()
            .map(|mut res| res.pull_newest())
            .flatten()
            .ok_or(())
            .and_then(|obstacles| {
                determine_next_move_from_obstacles(&obstacles, &world_limits).map_err(|_| ())
            })
    }
    let process_obstacles_service = app.spawn_service(process_obstacles);
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
        Blocking {
            request: (next_move, key),
            id,
            ..
        }: Blocking<(MoveVehicle, BufferKey<Obstacles>)>,
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
            .get(id, &key)
            .ok()
            .map(|mut res| res.newest().cloned())
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
    let check_change_lane_service = app.spawn_service(check_change_lane);
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
        Blocking { request: input, .. }: Blocking<TrafficSignalWithObstacles>,
        world_limits: Res<WorldLimits>,
    ) -> Result<MoveVehicle, TripRequestError> {
        // Since this is a Join operation, we have input from both TrafficSignal
        // and Obstacles. We can determine what is the next best move from both.
        let next_move_for_signal = determine_next_move_from_traffic_signal(&input.traffic_signal);

        let Ok(next_move_for_obstacles) =
            determine_next_move_from_obstacles(&input.obstacles, &world_limits)
        else {
            return Err(TripRequestError::NextMoveError);
        };

        Ok(choose_best_move(
            &next_move_for_signal,
            &next_move_for_obstacles,
        ))
    }
    let join_traffic_signal_and_obstacles_service =
        app.spawn_service(join_traffic_signal_and_obstacles);
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
        Blocking {
            request: keys, id, ..
        }: Blocking<TrafficSignalWithObstaclesAccessor>,
        mut traffic_signal_access: BufferAccessMut<TrafficSignal>,
        mut obstacles_access: BufferAccessMut<Obstacles>,
        world_limits: Res<WorldLimits>,
    ) -> Result<MoveVehicle, TripRequestError> {
        let signal_next_move = traffic_signal_access
            .get(id, &keys.traffic_signal)
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
            .get(id, &keys.obstacles)
            .ok()
            .map(|res| res.newest().cloned())
            .flatten()
            .and_then(|obstacles| {
                determine_next_move_from_obstacles(&obstacles, &world_limits).ok()
            });

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
        app.spawn_service(listen_traffic_signal_and_obstacles);
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
    let move_forward_description = "Generates a MoveVehicle::Forward output";
    fn move_forward(Blocking { .. }: Blocking<()>) -> MoveVehicle {
        MoveVehicle::Forward(Velocity::default_forward())
    }
    registry.register_node_builder(
        NodeBuilderOptions::new("move_forward".to_owned())
            .with_description(move_forward_description),
        |builder, _config: ()| builder.create_node(move_forward.into_callback()),
    );

    // =========================================================================
    let move_vehicle_description = "Move vehicle";
    fn move_vehicle(
        Blocking {
            request: move_vehicle,
            ..
        }: Blocking<MoveVehicle>,
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
    let move_vehicle_service = app.spawn_service(move_vehicle);
    registry.register_node_builder(
        NodeBuilderOptions::new("move_vehicle".to_string())
            .with_description(move_vehicle_description),
        move |builder, _config: ()| builder.create_node(move_vehicle_service),
    );

    // =========================================================================
    let destination_reached_description = "Check if the main vehicle has reached the destination";
    fn destination_reached(
        Blocking { .. }: Blocking<()>,
        vehicle_state: Res<VehicleState>,
    ) -> Result<(), ()> {
        if vehicle_state.at_destination() {
            Ok(())
        } else {
            Err(())
        }
    }
    let destination_reached_service = app.spawn_service(destination_reached);
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
        Blocking { .. }: Blocking<()>,
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
    let stop_engine_service = app.spawn_service(stop_engine);
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
    world_limits: &WorldLimits,
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
