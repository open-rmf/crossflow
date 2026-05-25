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
    vehicle::{MainVehicle, Position, VehicleDynamics, ThrottleCommand, SteeringCommand, cap},
};
use bevy::prelude::*;

#[derive(Clone, Debug, Component)]
pub struct ScrollingWorld;

#[derive(Default)]
pub struct MovementPlugin {}

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            move_vehicles,
        );
    }
}

// Our vehicle doesn't actually have to move vertically, as the world is set to
// move around the vehicle (treadmill concept). This system is mainly set up to
// facilitate lane changing, so that the vehicle can move horizontally.
fn move_vehicles(
    mut transforms: Query<(
        &mut Transform,
        &mut Position,
        &mut VehicleDynamics,
        &ThrottleCommand,
        &SteeringCommand,
        Option<&MainVehicle>,
    )>,
    mut scrolling_world: Query<(&mut Transform, Option<&LaneDash>), (With<ScrollingWorld>, Without<ThrottleCommand>)>,
    mut camera: Query<&mut Transform, (With<Camera>, Without<ThrottleCommand>, Without<ScrollingWorld>)>,
    world_limits: Res<WorldLimits>,
    time: Res<Time>,
) {
    let scale = world_limits.size_conversion;
    let dt = time.delta_secs();
    for (mut transform, mut position, mut dynamics, engine, steering, main) in transforms.iter_mut() {
        dynamics.command(&*engine, &*steering, dt);
        let speed = dynamics.speed;

        let w = dynamics.wheel_rotation.to_radians();
        let yaw = position.yaw + w + f32::to_radians(90.0);

        let v = speed * Vec2::new(f32::cos(yaw), f32::sin(yaw));
        position.translation += v * dt;
        position.yaw += speed * w * dt;

        position.translation.x = cap(position.translation.x, 10.0);
        if position.translation.x < -12.0 {
            position.translation.x = -12.0
        } else if position.translation.x > -2.0 {
            position.translation.x = -2.0;
        }

        let p = position.translation;
        transform.translation = scale * Vec3::new(p.x, p.y, 0.0);
        transform.rotation = Quat::from_axis_angle(Vec3::Z, position.yaw);

        if main.is_some() {
            let reference_y = transform.translation.y;
            let window_height = world_limits.window.1;

            for (mut transform, lane) in scrolling_world.iter_mut() {
                // If the world element has gone out of frame, teleport it back up
                // Let full runway be 4x window height
                let dy = reference_y - transform.translation.y;
                if dy.abs() > window_height {
                    if lane.is_some() {
                        transform.translation.y += dy.signum() * 2.0 * window_height;
                    } else {
                        transform.translation.y += dy.signum() * world_limits.full_runway;
                    }
                }
            }

            if let Ok(mut camera_tf) = camera.single_mut() {
                camera_tf.translation.y = reference_y;
            }
        }
    }
}
