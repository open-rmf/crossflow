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

// ANCHOR: example
use crossflow::bevy_app::{App, Update};
use crossflow::prelude::*;

use bevy_derive::*;
use bevy_ecs::prelude::*;
use bevy_time::{Time, TimePlugin};
use glam::Vec2;

use std::collections::HashMap;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
struct MoveBaseVehicle;

fn main() {
    let mut app = App::new();
    app.add_plugins((CrossflowExecutorApp::default(), TimePlugin::default()))
        .insert_resource(Position(Vec2::ZERO));

    let move_base = app.spawn_continuous_service(
        Update,
        move_base_vehicle_to_target
            .with(|mut srv: EntityWorldMut| {
                // Set the speed component for this service provider
                srv.insert(Speed(1.0));
            })
            .configure(|config| {
                // Put this service into a system set so that we can order other
                // services before or after it.
                config.in_set(MoveBaseVehicle)
            }),
    );
    let send_drone = app.spawn_continuous_service(
        Update,
        send_drone_to_target.configure(|config| {
            // This service depends on side-effects from move_base, so we should
            // always schedule it afterwards.
            config.after(MoveBaseVehicle)
        }),
    );

    let move_vehicle_to_random_position = move |app: &mut App| {
        app.world_mut()
            .command(|commands| commands.request(random_vec2(20.0), move_base).outcome())
    };

    let launch_drone_to_random_position = move |app: &mut App| {
        app.world_mut().command(|commands| {
            let request = DroneRequest {
                target: random_vec2(20.0),
                speed: 1.0,
            };
            commands.request(request, send_drone).detach();
        });
    };

    let mut base_moving = move_vehicle_to_random_position(&mut app);
    let mut last_launch = std::time::Instant::now();
    loop {
        app.update();

        if base_moving.is_available() {
            // Send the base to a new location
            base_moving = move_vehicle_to_random_position(&mut app);
        }

        if last_launch.elapsed() > std::time::Duration::from_secs(1) {
            launch_drone_to_random_position(&mut app);
            last_launch = std::time::Instant::now();
        }
    }
}

fn random_vec2(width: f32) -> Vec2 {
    width * Vec2::new(rand::random::<f32>(), rand::random::<f32>())
}

// ANCHOR: move_base_vehicle_to_target_example
#[derive(Resource, Deref, DerefMut)]
struct Position(Vec2);

#[derive(Component, Deref)]
struct Speed(f32);

fn move_base_vehicle_to_target(
    In(srv): ContinuousServiceInput<Vec2, Result<(), ()>>,
    mut query: ContinuousQuery<Vec2, Result<(), ()>>,
    speeds: Query<&Speed>,
    mut base_position: ResMut<Position>,
    time: Res<Time>,
) {
    let Some(mut orders) = query.get_mut(&srv.key) else {
        // The service provider has been despawned, so this continuous service
        // can no longer function.
        return;
    };

    // Get the oldest active request for this service. Orders will be indexed
    // from 0 to N-1 from oldest to newest. When an order is completed, all
    // orders after it will shift down by an index on the next update cycle.
    let Some(order) = orders.get_mut(0) else {
        // There are no active requests, so no need to do anything
        return;
    };

    let dt = time.delta_secs_f64() as f32;
    let Ok(speed) = speeds.get(srv.key.provider()) else {
        // The velocity setting has been taken from the service, so it can no
        // longer complete requests.
        order.respond(Err(()));
        return;
    };

    let target = *order.request();
    match move_to(**base_position, target, **speed, dt) {
        Ok(_) => {
            // The vehicle arrived
            **base_position = target;
            println!("Base vehicle arrived at {target}");
            order.respond(Ok(()));
        }
        Err(new_position) => {
            // The vehicle made progress but did not arrive
            **base_position = new_position;
        }
    }
}
// ANCHOR_END: move_base_vehicle_to_target_example

// ANCHOR: send_drone_to_target_example
#[derive(Clone, Copy)]
struct DroneRequest {
    target: Vec2,
    speed: f32,
}

fn send_drone_to_target(
    In(srv): ContinuousServiceInput<DroneRequest, ()>,
    mut query: ContinuousQuery<DroneRequest, ()>,
    mut drone_positions: Local<HashMap<Entity, Vec2>>,
    base_position: Res<Position>,
    time: Res<Time>,
) {
    let Some(mut orders) = query.get_mut(&srv.key) else {
        return;
    };

    orders.for_each(|order| {
        let DroneRequest { target, speed } = *order.request();
        let position = drone_positions.entry(order.id()).or_insert_with(|| {
            println!(
                "Drone {} taking off from {}, heading to {target}",
                order.id().index(),
                **base_position,
            );
            **base_position
        });
        let dt = time.delta_secs_f64() as f32;
        match move_to(*position, target, speed, dt) {
            Ok(_) => {
                println!("Drone {} arrived at {target}", order.id().index());
                order.respond(());
            }
            Err(new_position) => {
                *position = new_position;
            }
        }
    });

    // Remove any old task IDs that are no longer in use
    drone_positions.retain(|id, _| orders.iter().any(|order| order.id() == *id));
}
// ANCHOR_END: send_drone_to_target_example

fn move_to(current: Vec2, target: Vec2, speed: f32, dt: f32) -> Result<(), Vec2> {
    let dx = f32::max(0.0, speed * dt);
    let dp = target - current;
    let distance = dp.length();
    if distance <= dx {
        return Ok(());
    }

    let Some(u) = dp.try_normalize() else {
        // If dp can't be normalized then it's close to zero, so there's
        // nowhere to go.
        return Ok(());
    };

    return Err(current + u * dx);
}
// ANCHOR_END: example
