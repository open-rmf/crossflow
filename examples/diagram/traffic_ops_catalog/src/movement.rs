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

use crate::{
    spawn_world::{LaneDash, WorldLimits},
    vehicle::{Acceleration, MainVehicle, Velocity},
};
use bevy::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Component)]
pub struct ScrollingWorld;

#[derive(Resource)]
struct GlobalSpeed(f32);

impl FromWorld for GlobalSpeed {
    fn from_world(_world: &mut World) -> Self {
        Self(0.0)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Kinematics {
    pub velocity: Velocity,
    pub acceleration: Acceleration,
    pub dt: f32,
}

#[derive(Default)]
pub struct MovementPlugin {}

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobalSpeed>().add_systems(
            PostUpdate,
            (move_vehicles, update_global_speed, scroll_world_system),
        );
    }
}

// Our vehicle doesn't actually have to move vertically, as the world is set to
// move around the vehicle (treadmill concept). This system is mainly set up to
// facilitate lane changing, so that the vehicle can move horizontally.
fn move_vehicles(
    mut transforms: Query<(
        &mut Transform,
        &mut Velocity,
        &Acceleration,
        Option<&MainVehicle>,
    )>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut transform, mut velocity, acceleration, main_vehicle) in transforms.iter_mut() {
        velocity.x += acceleration.x * dt.clone();
        velocity.y += acceleration.y * dt.clone();

        // Clamp velocity to 0.0, do not allow reverse motion
        if velocity.y < 0.0 {
            velocity.y = 0.0;
        }

        transform.translation.x += velocity.x * dt.clone();
        if main_vehicle.is_none() {
            // Only update y-transform for non-main vehicles
            transform.translation.y += velocity.y * dt;
        }
    }
}

fn update_global_speed(
    mut global_speed: ResMut<GlobalSpeed>,
    velocity: Query<&Velocity, (With<MainVehicle>, Changed<Velocity>)>,
) {
    let Ok(vehicle_velocity) = velocity.single() else {
        return;
    };
    global_speed.0 = vehicle_velocity.y;
}

fn scroll_world_system(
    mut scrolling_world: Query<(&mut Transform, Option<&LaneDash>), With<ScrollingWorld>>,
    time: Res<Time>,
    global_speed: Res<GlobalSpeed>,
    world_limits: Res<WorldLimits>,
) {
    let window_height = world_limits.window.1;
    let dt = time.delta_secs();
    let scroll_distance = global_speed.0 * dt;

    for (mut transform, lane) in scrolling_world.iter_mut() {
        // Move the world backward to make the vehicle look like it's moving forward
        transform.translation.y -= scroll_distance;
        // If the world element has gone out of frame, teleport it back up
        // Let full runway be 4x window height
        if transform.translation.y < -window_height {
            if lane.is_some() {
                transform.translation.y += 2.0 * window_height;
            } else {
                transform.translation.y += world_limits.full_runway;
            }
        }
    }
}
