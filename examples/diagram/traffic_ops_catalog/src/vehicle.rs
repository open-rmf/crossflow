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
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Enum variants represent hierarchical priority
#[repr(i32)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, PartialOrd)]
pub enum MoveVehicle {
    #[default]
    Stop = 0,
    ChangeSpeed(Acceleration) = 1,
    ChangeLane(Velocity) = 2,
    Forward(Velocity) = 3,
}

impl MoveVehicle {
    pub fn min(&self, other_move: &MoveVehicle) -> MoveVehicle {
        if self < other_move {
            return self.clone();
        }
        return other_move.clone();
    }
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

#[derive(Clone, Debug, Resource, Serialize, Deserialize, JsonSchema)]
pub struct VehicleState {
    engine_on: bool,
    distance_to_destination: f32,
    checklist: HashMap<String, ReadyState>,
    changing_to_lane: Option<Lane>,
    speed: i32,
}

impl Default for VehicleState {
    fn default() -> Self {
        let mut checklist = HashMap::<String, ReadyState>::new();
        let checklist_items = vec!["fuel", "mirrors"];
        for item in checklist_items.iter() {
            checklist.insert(item.to_string(), ReadyState::default());
        }
        Self {
            engine_on: false,
            distance_to_destination: 0.0,
            checklist,
            changing_to_lane: None,
            speed: Velocity::default_forward().y.round() as i32,
        }
    }
}

impl VehicleState {
    pub fn engine(&self) -> bool {
        self.engine_on
    }

    pub fn toggle_engine(&mut self, on: bool) -> &mut Self {
        self.engine_on = on;
        self
    }

    pub fn checklist(&self) -> &HashMap<String, ReadyState> {
        &self.checklist
    }

    pub fn checklist_mut(&mut self) -> &mut HashMap<String, ReadyState> {
        &mut self.checklist
    }

    pub fn distance_left(&self) -> f32 {
        self.distance_to_destination.clone()
    }

    pub fn try_move(&mut self, mut e_cmds: EntityCommands, move_vehicle: MoveVehicle) -> &mut Self {
        match move_vehicle {
            MoveVehicle::Forward(velocity) => {
                if velocity.y > self.speed as f32 {
                    // If the vehicle is starting to move from a stationary state,
                    // speed up quickly. Else, use the default acceleration.
                    if self.speed < 20 {
                        e_cmds.insert(Acceleration::quick_speed_up());
                    } else {
                        e_cmds.insert(Acceleration::default_speed_up());
                    }
                } else {
                    e_cmds.insert(Acceleration::default_slow_down());
                }
            }
            MoveVehicle::ChangeSpeed(acceleration) => {
                e_cmds.insert(acceleration);
            }
            MoveVehicle::ChangeLane(velocity) => {
                // TODO(@xiyuoh) incorporate lane change into the workflow instead
                // of handling it from the simulator
                let new_to_lane = if velocity.x > 0.0 {
                    Lane::Right
                } else {
                    Lane::Left
                };
                self.changing_to_lane = Some(new_to_lane);
                e_cmds.insert((velocity, Acceleration::zero()));
            }
            MoveVehicle::Stop => {
                e_cmds.insert((Velocity::zero(), Acceleration::zero()));
            }
        }
        self
    }

    pub fn set_distance_to_destination(&mut self, distance: f32) -> bool {
        if distance <= 0.0 {
            return false;
        }
        self.distance_to_destination = distance;
        true
    }

    pub fn update_remaining_distance(&mut self, distance: f32) -> &mut Self {
        self.distance_to_destination -= distance;
        if self.distance_to_destination < 0.0 {
            self.reset();
        }
        self
    }

    pub fn at_destination(&self) -> bool {
        self.distance_to_destination == 0.0
    }

    pub fn reset(&mut self) -> &mut Self {
        self.distance_to_destination = 0.0;
        for (_, state) in self.checklist.iter_mut() {
            *state = ReadyState::default();
        }
        self.changing_to_lane = None;
        self
    }

    pub fn changing_lane(&self) -> &Option<Lane> {
        &self.changing_to_lane
    }

    pub fn changed_lane(&mut self) -> &mut Self {
        self.changing_to_lane = None;
        self
    }

    pub fn speed(&self) -> i32 {
        self.speed
    }

    pub fn update_speed(&mut self, speed: i32) -> &mut Self {
        self.speed = speed;
        self
    }
}

#[derive(Clone, Debug, Default, Component)]
pub struct Vehicle;

#[derive(Clone, Debug, Component)]
#[require(Vehicle)]
pub struct MainVehicle;

#[derive(Clone, Debug, Component, Serialize, Deserialize, JsonSchema, PartialEq, PartialOrd)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Default for Velocity {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Velocity {
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn default_forward() -> Self {
        Self { x: 0.0, y: 60.0 }
    }

    pub fn default_change_lane(to_lane: Lane) -> Self {
        Self {
            x: match to_lane {
                Lane::Left => -10.0,
                Lane::Right => 10.0,
            },
            y: 40.0,
        }
    }

    pub fn default_pedestrian() -> Self {
        Self { x: 40.0, y: 0.0 }
    }
}

#[derive(Clone, Debug, Component, Serialize, Deserialize, JsonSchema, PartialEq, PartialOrd)]
pub struct Acceleration {
    pub x: f32,
    pub y: f32,
}

impl Default for Acceleration {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Acceleration {
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn default_slow_down() -> Self {
        Self { x: 0.0, y: -5.0 }
    }

    pub fn default_speed_up() -> Self {
        Self { x: 0.0, y: 5.0 }
    }

    pub fn quick_speed_up() -> Self {
        Self { x: 0.0, y: 30.0 }
    }
}

#[derive(Clone, Debug, Default, Bundle)]
pub struct VehicleBundle {
    pub vehicle: Vehicle,
    pub velocity: Velocity,
    pub acceleration: Acceleration,
}
