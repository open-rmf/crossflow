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
use serde_json::json;
use std::{collections::HashMap, thread::sleep, time::Duration};
use thiserror::Error;

pub mod movement;
pub use movement::*;

pub mod pedestrian;
pub use pedestrian::*;

pub mod spawn_world;
pub use spawn_world::*;

pub mod speed_limit;
pub use speed_limit::*;

pub mod traffic;
pub use traffic::*;

pub mod traffic_signal;
pub use traffic_signal::*;

pub mod user_panel;
pub use user_panel::*;

pub mod vehicle;
pub use vehicle::*;

#[derive(StreamPack)]
struct MidJourneyStreams {
    trigger: (),
}

#[derive(StreamPack)]
struct TrafficStateStreams {
    traffic_signal: TrafficSignal,
    obstacles: Obstacles,
    arriving: ApproachingIntersection,
    speed_limit: SpeedLimit,
}

#[derive(Clone, Accessor)]
struct TrafficStateAccessor {
    traffic_signal: BufferKey<TrafficSignal>,
    obstacles: BufferKey<Obstacles>,
    arriving: BufferKey<ApproachingIntersection>,
    speed_limit: BufferKey<SpeedLimit>,
}

#[derive(Clone, Accessor)]
struct TrafficSignalWithObstaclesAccessor {
    traffic_signal: BufferKey<TrafficSignal>,
    obstacles: BufferKey<Obstacles>,
}

#[derive(Clone, Accessor)]
struct TrafficSignalWithArrivingAccessor {
    traffic_signal: BufferKey<TrafficSignal>,
    arriving: BufferKey<ApproachingIntersection>,
}

#[derive(Clone, Debug, Default, Joined)]
pub struct TrafficSignalWithObstacles {
    traffic_signal: TrafficSignal,
    obstacles: Obstacles,
}

#[derive(Clone, Debug, Default, Joined)]
pub struct TrafficSignalWithArriving {
    traffic_signal: TrafficSignal,
    arriving: ApproachingIntersection,
}

#[derive(Clone, Debug, Error, Serialize, Deserialize, JsonSchema)]
pub enum TripRequestError {
    #[error("Engine start error")]
    EngineStartError,
    #[error("Vehicle check error")]
    VehicleCheckError,
    #[error("Buffer access error")]
    BufferAccessError,
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
        mut commands: Commands,
        mut vehicle_state: ResMut<VehicleState>,
        vehicle_velocity: Query<Entity, (With<MainVehicle>, With<Velocity>)>,
    ) -> Result<(), TripRequestError> {
        let Ok(e) = vehicle_velocity.single() else {
            return Err(TripRequestError::EngineStartError);
        };
        let e_cmds = commands.entity(e);

        vehicle_state.toggle_engine(true);
        if vehicle_state.set_distance_to_destination(distance_to_destination) {
            info!(
                "Vehicle beginning its trip, distance to destination: {}",
                vehicle_state.distance_left()
            );
            // Move vehicle forward upon starting engine
            vehicle_state.try_move(e_cmds, MoveVehicle::Forward(Velocity::default_forward()));

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
    let configure_obstacles_thresholds_description = "Update ObstacleLimits based on
        the configured thresholds";
    let configure_obstacles_thresholds_examples = [ConfigExample::new(
        "Set thresholds for obstacle detection.",
        json!({
            "x_threshold": 100.0,
            "y_stop": 300.0,
            "y_slow_down": 400.0,
            "y_back": 100.0
        }),
    )];
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("configure_obstacles_thresholds".to_string())
                .with_description(configure_obstacles_thresholds_description)
                .with_config_examples(configure_obstacles_thresholds_examples),
            move |builder, config: Option<ObstacleLimits>| {
                let configure_obstacles_thresholds_service = builder.commands().spawn_service(
                    move |Blocking { .. }: Blocking<()>, mut world_limits: ResMut<WorldLimits>| {
                        if let Some(limits) = config.as_ref() {
                            *world_limits.obstacle_limits_mut() = limits.clone();
                            info!(
                                "Updated obstacle limits: {:?}",
                                world_limits.obstacle_limits
                            );
                        }
                    },
                );
                builder.create_node(configure_obstacles_thresholds_service)
            },
        );

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
                    if ob.translation.y > 0.5 * world_limits.window.1 {
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
    let approaching_intersection_description = "Detects how far the vehicle is from
        the upcoming traffic intersection";
    fn approaching_intersection(
        srv: ContinuousService<(), (), TrafficStateStreams>,
        mut orders: ContinuousQuery<(), (), TrafficStateStreams>,
        next_traffic_light: Res<NextTrafficLight>,
        main_vehicle: Query<&Transform, With<MainVehicle>>,
        traffic_lights: Query<&Transform, (With<TrafficLight>, Without<MainVehicle>)>,
        world_limits: Res<WorldLimits>,
    ) {
        let Some(mut orders) = orders.get_mut(&srv.key) else {
            return;
        };

        if orders.is_empty() {
            return;
        }

        let Ok(y_vehicle) = main_vehicle.single().map(|tf| tf.translation.y) else {
            return;
        };

        if let Some(y_next_light) = next_traffic_light
            .0
            .and_then(|e| traffic_lights.get(e).ok())
            .map(|tf| tf.translation.y)
        {
            let distance_to_intersection = y_next_light - y_vehicle;
            if distance_to_intersection <= 0.5 * world_limits.vehicle_size.1 {
                // Ignore if vehicle's front has already passed the intersection
                return;
            }
            orders.for_each(|order| {
                order.streams().arriving.send(ApproachingIntersection {
                    distance: distance_to_intersection,
                })
            });
        }
    }

    let approaching_intersection_service =
        app.spawn_continuous_service(PostUpdate, approaching_intersection);
    registry.register_node_builder(
        NodeBuilderOptions::new("approaching_intersection".to_string())
            .with_description(approaching_intersection_description),
        move |builder, _config: ()| builder.create_node(approaching_intersection_service),
    );

    // =========================================================================
    let filter_arriving_description = "Filter arriving messages based on distance to intersection";
    let filter_arriving_examples = [ConfigExample::new(
        "Filter ApproachingIntersection messages such that they are considered to
         be arriving if they are within 200.0 px from the intersection.",
        json!(200.0),
    )];
    registry
        .register_node_builder(
            NodeBuilderOptions::new("filter_arriving")
                .with_description(filter_arriving_description)
                .with_config_examples(filter_arriving_examples),
            |builder, config: Option<f32>| {
                builder.create_map_block(move |arriving: ApproachingIntersection| {
                    if arriving.distance <= config.unwrap_or(100.0) {
                        Ok(arriving)
                    } else {
                        Err(())
                    }
                })
            },
        )
        .with_result();

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
            .map(|res| res.newest().cloned())
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

    // =========================================================================
    let check_speed_limit_description = "Checks the current speed limit on the road";
    fn check_speed_limit(
        srv: ContinuousService<(), (), TrafficStateStreams>,
        mut orders: ContinuousQuery<(), (), TrafficStateStreams>,
        current_speed_limit: Res<CurrentSpeedLimit>,
    ) {
        let Some(mut orders) = orders.get_mut(&srv.key) else {
            return;
        };

        if orders.is_empty() {
            return;
        }

        orders.for_each(|order| {
            order
                .streams()
                .speed_limit
                .send(current_speed_limit.0.clone())
        });
    }

    let check_speed_limit_service = app.spawn_continuous_service(PostUpdate, check_speed_limit);
    registry.register_node_builder(
        NodeBuilderOptions::new("check_speed_limit".to_string())
            .with_description(check_speed_limit_description),
        move |builder, _config: ()| builder.create_node(check_speed_limit_service),
    );

    // =========================================================================
    let follow_speed_limit_description = "Check the current speed limit and clamp
        vehicle speed if it exceeds limit";
    fn follow_speed_limit(
        Blocking {
            request: (_, key),
            id,
            ..
        }: Blocking<((), BufferKey<SpeedLimit>)>,
        mut access: BufferAccessMut<SpeedLimit>,
        vehicle_state: Res<VehicleState>,
    ) -> Result<MoveVehicle, ()> {
        let Some(speed_limit) = access
            .get_mut(id, &key)
            .ok()
            .map(|res| res.newest().cloned())
            .flatten()
        else {
            return Err(());
        };

        let vehicle_speed = vehicle_state.speed();
        if vehicle_speed >= speed_limit.0 {
            return Ok(MoveVehicle::ChangeSpeed(Acceleration::default_slow_down()));
        }

        Ok(MoveVehicle::Forward(Velocity::default_forward()))
    }
    let follow_speed_limit_service = app.spawn_service(follow_speed_limit);
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("follow_speed_limit".to_string())
                .with_description(follow_speed_limit_description),
            move |builder, _config: ()| builder.create_node(follow_speed_limit_service),
        )
        .with_buffer_access()
        .with_result()
        .with_common_response();

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
    let join_traffic_signal_and_arriving_description = "Join the latest traffic signal, \
        obstacles and approaching intersection buffers, and determine the best move
        from these inputs";
    fn join_traffic_signal_and_arriving(
        Blocking { request: input, .. }: Blocking<TrafficSignalWithArriving>,
    ) -> Result<MoveVehicle, TripRequestError> {
        // Since this is a Join operation combining TrafficSignal and
        // ApproachingIntersection buffers, the vehicle is definitely approaching
        // an intersection since the buffer is non-empty.
        Ok(determine_next_move_from_traffic_signal(
            &input.traffic_signal,
        ))
    }
    let join_traffic_signal_and_arriving_service =
        app.spawn_service(join_traffic_signal_and_arriving);
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("join_traffic_signal_and_arriving".to_string())
                .with_description(join_traffic_signal_and_arriving_description),
            move |builder, _config: ()| {
                builder.create_node(join_traffic_signal_and_arriving_service)
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
    let listen_traffic_signal_and_arriving_description = "Listen both traffic signal and \
        approaching intersection buffers and determine the next move based on \
        the combination of buffers activated.";
    fn listen_traffic_signal_and_arriving(
        Blocking {
            request: keys, id, ..
        }: Blocking<TrafficSignalWithArrivingAccessor>,
        mut traffic_signal_access: BufferAccessMut<TrafficSignal>,
        mut arriving_access: BufferAccessMut<ApproachingIntersection>,
    ) -> Result<MoveVehicle, TripRequestError> {
        let Ok(traffic_signal_buffer) = traffic_signal_access.get(id, &keys.traffic_signal) else {
            error!("Unable to access traffic signal buffer");
            return Err(TripRequestError::BufferAccessError);
        };
        let Ok(mut arriving_buffer) = arriving_access.get_mut(id, &keys.arriving) else {
            error!("Unable to access approaching intersection buffer");
            return Err(TripRequestError::BufferAccessError);
        };

        let signal_next_move = if arriving_buffer.is_empty() {
            // Ignore traffic signal as vehicle is not approaching the intersection
            MoveVehicle::Forward(Velocity::default_forward())
        } else if let Some(signal) = traffic_signal_buffer.newest() {
            // If traffic signal is Green, drain the arriving buffer.
            // Else, leave it alone so that we can continue to listen for traffic
            // signal changes while approaching the intersection.
            if matches!(signal, TrafficSignal::Green) {
                arriving_buffer.drain(..);
            }
            determine_next_move_from_traffic_signal(&signal)
        } else {
            // The vehicle is approaching the intersection but no traffic signal
            // is detected. To be safe, treat this as a red light and stop the
            // vehicle regardless of obstacles.
            MoveVehicle::Stop
        };

        Ok(signal_next_move)
    }
    let listen_traffic_signal_and_arriving_service =
        app.spawn_service(listen_traffic_signal_and_arriving);
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("listen_traffic_signal_and_arriving".to_string())
                .with_description(listen_traffic_signal_and_arriving_description),
            move |builder, _config: ()| {
                builder.create_node(listen_traffic_signal_and_arriving_service)
            },
        )
        .with_listen()
        .with_result()
        .with_common_response();

    // =========================================================================
    let listen_traffic_state_description = "Listen all traffic state buffers \
        and determine the next move based on the combination of buffers activated.";
    fn listen_traffic_state(
        Blocking {
            request: keys, id, ..
        }: Blocking<TrafficStateAccessor>,
        mut traffic_signal_access: BufferAccessMut<TrafficSignal>,
        mut obstacles_access: BufferAccessMut<Obstacles>,
        mut arriving_access: BufferAccessMut<ApproachingIntersection>,
        mut speed_limit_access: BufferAccessMut<SpeedLimit>,
        vehicle_state: Res<VehicleState>,
        world_limits: Res<WorldLimits>,
    ) -> Result<MoveVehicle, TripRequestError> {
        let Ok(traffic_signal_buffer) = traffic_signal_access.get(id, &keys.traffic_signal) else {
            error!("Unable to access traffic signal buffer");
            return Err(TripRequestError::BufferAccessError);
        };
        let Ok(mut arriving_buffer) = arriving_access.get_mut(id, &keys.arriving) else {
            error!("Unable to access approaching intersection buffer");
            return Err(TripRequestError::BufferAccessError);
        };
        let Ok(obstacles_buffer) = obstacles_access.get(id, &keys.obstacles) else {
            error!("Unable to access obstacles buffer");
            return Err(TripRequestError::BufferAccessError);
        };
        let Ok(speed_limit_buffer) = speed_limit_access.get(id, &keys.speed_limit) else {
            error!("Unable to access speed limit buffer");
            return Err(TripRequestError::BufferAccessError);
        };

        let signal_next_move = if arriving_buffer.is_empty() {
            // Ignore traffic signal as vehicle is not approaching the intersection
            MoveVehicle::Forward(Velocity::default_forward())
        } else if let Some(signal) = traffic_signal_buffer.newest() {
            // If traffic signal is Green, drain the arriving buffer.
            // Else, leave it alone so that we can continue to listen for traffic
            // signal changes while approaching the intersection.
            if matches!(signal, TrafficSignal::Green) {
                arriving_buffer.drain(..);
            }
            determine_next_move_from_traffic_signal(&signal)
        } else {
            // The vehicle is approaching the intersection but no traffic signal
            // is detected. To be safe, treat this as a red light and stop the
            // vehicle regardless of obstacles.
            MoveVehicle::Stop
        };

        let mut best_move = signal_next_move.clone();
        // If obstacles buffer is non-empty, determine the best move from all
        // buffers
        if let Some(obstacles_next_move) = obstacles_buffer.newest().and_then(|obstacles| {
            determine_next_move_from_obstacles(&obstacles, &world_limits).ok()
        }) {
            best_move = choose_best_move(&signal_next_move, &obstacles_next_move);
        }
        // If speed limit buffer is non-empty, clamp speed and choose the best move
        if let Some(speed_limit_move) = speed_limit_buffer.newest().cloned().map(|limit| {
            if vehicle_state.speed() >= limit.0 {
                MoveVehicle::ChangeSpeed(Acceleration::default_slow_down())
            } else {
                MoveVehicle::Forward(Velocity::default_forward())
            }
        }) {
            best_move = choose_best_move(&best_move, &speed_limit_move);
        }

        Ok(best_move)
    }
    let listen_traffic_state_service = app.spawn_service(listen_traffic_state);
    registry
        .opt_out()
        .no_serializing()
        .no_deserializing()
        .register_node_builder(
            NodeBuilderOptions::new("listen_traffic_state".to_string())
                .with_description(listen_traffic_state_description),
            move |builder, _config: ()| builder.create_node(listen_traffic_state_service),
        )
        .with_listen()
        .with_result()
        .with_common_response();

    // =========================================================================
    let accelerate_vehicle_description = "Accelerate vehicle based on the requested MoveVehicle";
    fn accelerate_vehicle(
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

        match move_vehicle {
            MoveVehicle::Forward(velocity) => {
                if velocity.y > vehicle_state.speed() as f32 {
                    // If the vehicle is starting to move from a stationary state,
                    // speed up quickly. Else, use the default acceleration.
                    if vehicle_state.speed() < 20 {
                        vehicle_state.try_move(
                            e_cmds,
                            MoveVehicle::ChangeSpeed(Acceleration::quick_speed_up()),
                        );
                    } else {
                        vehicle_state.try_move(
                            e_cmds,
                            MoveVehicle::ChangeSpeed(Acceleration::default_speed_up()),
                        );
                    }
                } else {
                    vehicle_state.try_move(
                        e_cmds,
                        MoveVehicle::ChangeSpeed(Acceleration::default_slow_down()),
                    );
                }
            }
            _ => {
                vehicle_state.try_move(e_cmds, move_vehicle);
            }
        }
    }
    let accelerate_vehicle_service = app.spawn_service(accelerate_vehicle);
    registry.register_node_builder(
        NodeBuilderOptions::new("accelerate_vehicle".to_string())
            .with_description(accelerate_vehicle_description),
        move |builder, _config: ()| builder.create_node(accelerate_vehicle_service),
    );

    // =========================================================================
    let wait_for_destination_reached_description =
        "Wait until the main vehicle has reached the destination";
    fn wait_for_destination_reached(
        srv: ContinuousService<(), (), MidJourneyStreams>,
        mut orders: ContinuousQuery<(), (), MidJourneyStreams>,
        vehicle_state: Res<VehicleState>,
    ) {
        let Some(mut orders) = orders.get_mut(&srv.key) else {
            return;
        };
        if orders.is_empty() {
            return;
        }
        let Some(order) = orders.get_mut(0) else {
            return;
        };

        if vehicle_state.at_destination() {
            order.respond(());
        } else {
            order.streams().trigger.send(());
        }
    }
    let wait_for_destination_reached_service =
        app.spawn_continuous_service(PostUpdate, wait_for_destination_reached);
    registry.register_node_builder(
        NodeBuilderOptions::new("wait_for_destination_reached".to_string())
            .with_description(wait_for_destination_reached_description),
        move |builder, _config: ()| builder.create_node(wait_for_destination_reached_service),
    );

    // =========================================================================
    let stop_engine_description = "Stop engine and reset vehicle";
    fn stop_engine(
        Blocking { .. }: Blocking<()>,
        mut commands: Commands,
        mut vehicle_state: ResMut<VehicleState>,
        mut world_limits: ResMut<WorldLimits>,
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

        // Reset obstacle limits in case they were modified
        world_limits.reset_obstacle_limits();

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
    return match signal {
        TrafficSignal::Green => MoveVehicle::Forward(Velocity::default_forward()),
        TrafficSignal::Yellow => MoveVehicle::ChangeSpeed(Acceleration::default_slow_down()), // slow down for yellow light
        TrafficSignal::Red => MoveVehicle::Stop,
        TrafficSignal::Empty => MoveVehicle::Stop,
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
