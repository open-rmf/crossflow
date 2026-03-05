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

use crate::vehicle::Velocity;
use bevy::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TrafficSignal {
    Red,
    #[default]
    Green,
    Yellow,
}

#[derive(Clone, Debug, Default, Component, Serialize, Deserialize, JsonSchema)]
pub struct TrafficLight {
    pub id: i32,
    pub last_update: f32,
    pub signal: TrafficSignal,
}

impl TrafficLight {
    pub fn new(id: i32, last_update: f32, signal: TrafficSignal) -> Self {
        Self {
            id,
            last_update,
            signal,
        }
    }
}

#[derive(Clone, Debug, Default, Event, Serialize, Deserialize, JsonSchema, Hash, Eq, PartialEq)]
pub struct ObstacleInRange {
    pub offset_x: i32,
    pub offset_y: i32,
}

impl ObstacleInRange {
    pub fn new(offset_x: i32, offset_y: i32) -> Self {
        Self { offset_x, offset_y }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Obstacles(pub HashSet<ObstacleInRange>);

#[derive(Clone, Debug, Component, Serialize, Deserialize, JsonSchema)]
pub struct SpeedLimit(pub f32);

impl Default for SpeedLimit {
    fn default() -> Self {
        Self(Velocity::default_forward().y)
    }
}

#[derive(Clone, Debug, Default, Component)]
#[require(Transform)]
pub struct Obstacle;

#[derive(Clone, Debug, Component)]
#[require(Velocity)]
pub struct Pedestrian {
    pub alive: bool,
}
impl Default for Pedestrian {
    fn default() -> Self {
        Self { alive: true }
    }
}

impl Pedestrian {
    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn died(&mut self) {
        self.alive = false;
    }

    pub fn revived(&mut self) {
        self.alive = true;
    }
}
