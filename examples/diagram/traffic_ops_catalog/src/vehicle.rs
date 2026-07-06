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
use core::f32;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use glam::Vec2;

use crate::spawn_world::METERS_PER_SECOND_TO_KMH;

pub const VEHICLE_LAYER_Z: f32 = 10.0;

///
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, PartialOrd, Component)]
pub struct ThrottleCommand {
    pub target_speed: f32,
    #[serde(default)]
    pub max_acceleration: Option<f32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, PartialOrd, Component)]
pub struct SteeringCommand {
    pub target_turn_angle: f32,
    #[serde(default)]
    pub max_steer_speed: Option<f32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub enum Lane {
    #[default]
    Left,
    Right,
}

impl Lane {
    pub fn inverse(&self) -> Lane {
        match self {
            Lane::Left => Lane::Right,
            Lane::Right => Lane::Left,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum ReadyState {
    Ready,
    #[default]
    NotReady,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Component)]
pub struct VehicleDynamics {
    pub speed: f32,
    pub wheel_rotation: f32,
}

impl Default for VehicleDynamics {
    fn default() -> Self {
        Self {
            speed: 0.0,
            wheel_rotation: 0.0,
        }
    }
}

impl VehicleDynamics {
    pub fn command(
        &mut self,
        throttle: &ThrottleCommand,
        steering: &SteeringCommand,
        dt: f32,
    ) {
        if dt <= 0.0 {
            return;
        }

        let max_accel = throttle.max_acceleration.unwrap_or(2.0 * METERS_PER_SECOND_TO_KMH);
        let dv = throttle.target_speed - self.speed;
        let a = cap(dv/dt, max_accel);
        self.speed += a * dt;
        self.speed;

        let max_rot_speed = steering.max_steer_speed.unwrap_or(90.0 / 16.0);
        let dr = steering.target_turn_angle - self.wheel_rotation;
        let v_rot = cap(dr, max_rot_speed);
        self.wheel_rotation += cap(v_rot * dt, 30.0);
    }
}

pub fn cap(value: f32, limit: f32) -> f32 {
    if f32::abs(value) > limit {
        return f32::signum(value) * limit;
    }

    value
}

#[derive(Clone, Debug, Default, Component)]
pub struct Vehicle;

#[derive(Clone, Debug, Component)]
#[require(Vehicle)]
pub struct MainVehicle;

#[derive(Clone, Debug, Component, Default)]
pub struct Position {
    pub translation: Vec2,
    pub yaw: f32,
}

#[derive(Clone, Debug, Default, Bundle)]
pub struct VehicleBundle {
    pub position: Position,
    pub dynamics: VehicleDynamics,
    pub engine: ThrottleCommand,
    pub steering: SteeringCommand,
    pub vehicle: Vehicle,
    pub transform: Transform,
}

impl VehicleBundle {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            position: Position {
                translation: Vec2::new(x, y),
                yaw: 0.0,
            },
            transform: Transform::from_xyz(x, y, VEHICLE_LAYER_Z),
            ..Default::default()
        }
    }
}
