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

use core::f32;

use crate::{
    spawn_world::WorldLimits,
    traffic::{TrafficLight, TrafficSignal},
    user_panel::UserPanel,
    vehicle::MainVehicle,
};
use bevy::prelude::*;
use bevy_color::palettes::css as Colors;
use rand::Rng;

#[derive(Event)]
pub struct TrafficSignalChange {
    pub target: Entity,
    pub next: TrafficSignal,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct NextTrafficLight(pub Option<Entity>);

#[derive(Event)]
pub struct UpcomingTrafficSignal(pub TrafficSignal);

#[derive(Clone, Debug, Resource)]
pub struct TrafficLightColors {
    pub green: Handle<ColorMaterial>,
    pub yellow: Handle<ColorMaterial>,
    pub red: Handle<ColorMaterial>,
}

impl FromWorld for TrafficLightColors {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        Self {
            green: materials.add(Color::Srgba(Colors::DARK_GREEN)),
            yellow: materials.add(Color::Srgba(Colors::ORANGE)),
            red: materials.add(Color::Srgba(Colors::DARK_RED)),
        }
    }
}

#[derive(Default)]
pub struct TrafficSignalPlugin {}

impl Plugin for TrafficSignalPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NextTrafficLight::default())
            .add_event::<TrafficSignalChange>()
            .add_event::<UpcomingTrafficSignal>()
            .add_systems(Update, change_traffic_signal)
            .add_systems(PostUpdate, monitor_upcoming_traffic_signal)
            .add_observer(on_traffic_signal_change);
    }
}

// This system generates the next traffic signal and triggers a TrafficSignalChange
fn change_traffic_signal(
    mut commands: Commands,
    time: Res<Time>,
    traffic_lights: Query<(Entity, &TrafficLight)>,
    user_panel: Res<UserPanel>,
) {
    if !user_panel.auto_signal_change {
        return;
    }

    let mut rng = rand::rng();
    for (e, traffic_light) in traffic_lights.iter() {
        let time_to_change: u64 = match traffic_light.signal {
            TrafficSignal::Red => rng.random_range(2..=5),
            TrafficSignal::Green => rng.random_range(4..=8),
            TrafficSignal::Yellow => rng.random_range(1..=3),
        };
        let now = time.elapsed_secs();
        if (now - traffic_light.last_update) as u64 > time_to_change {
            // Change traffic signal
            let next = match traffic_light.signal {
                TrafficSignal::Red => TrafficSignal::Green,
                TrafficSignal::Green => TrafficSignal::Yellow,
                TrafficSignal::Yellow => TrafficSignal::Red,
            };
            commands.trigger(TrafficSignalChange { target: e, next });
        }
    }
}

// This system checks for traffiic signal changes and updates accordingly.
// We separate this from change_traffic_signal to allow user input via the UI.
fn on_traffic_signal_change(
    trigger: Trigger<TrafficSignalChange>,
    mut commands: Commands,
    mut traffic_lights: Query<&mut TrafficLight>,
    time: Res<Time>,
    traffic_light_colors: Res<TrafficLightColors>,
) {
    let signal_change = trigger.event();
    if let Ok(mut signal) = traffic_lights.get_mut(signal_change.target) {
        signal.last_update = time.elapsed_secs();
        signal.signal = signal_change.next.clone();

        let new_material = match signal_change.next {
            TrafficSignal::Red => MeshMaterial2d(traffic_light_colors.red.clone()),
            TrafficSignal::Green => MeshMaterial2d(traffic_light_colors.green.clone()),
            TrafficSignal::Yellow => MeshMaterial2d(traffic_light_colors.yellow.clone()),
        };
        commands.entity(signal_change.target).insert(new_material);
    };
}

fn monitor_upcoming_traffic_signal(
    mut upcoming_signal: EventWriter<UpcomingTrafficSignal>,
    mut next_traffic_light: ResMut<NextTrafficLight>,
    main_vehicle: Query<&Transform, With<MainVehicle>>,
    traffic_lights: Query<(Entity, &Transform, &TrafficLight), Without<MainVehicle>>,
    world_limits: Res<WorldLimits>,
) {
    let Ok(vehicle_y) = main_vehicle.single().map(|tf| tf.translation.y) else {
        return;
    };

    let mut next_signal: Option<(Entity, TrafficSignal)> = None;
    let mut distance_to_next_signal = f32::INFINITY;
    for (e, transform, traffic_light) in traffic_lights.iter() {
        let offset_y = transform.translation.y - vehicle_y;
        // Ignore if traffic light is behind the main vehicle
        if offset_y < 0.0 {
            continue;
        }
        // Ignore if traffic light is off-screen
        if transform.translation.y > 0.5 * world_limits.window_height {
            continue;
        }
        // TODO(@xiyuoh) Consider ignoring traffic lights too far away, or
        // incorporating approaching intersection logic.

        if offset_y < distance_to_next_signal {
            let _ = next_signal.insert((e, traffic_light.signal.clone()));
            distance_to_next_signal = offset_y;
        }
    }

    if let Some((new_next_entity, new_next_signal)) = next_signal {
        if next_traffic_light.0.is_none()
            || next_traffic_light.0.is_some_and(|e| e != new_next_entity)
        {
            next_traffic_light.0 = Some(new_next_entity);
        }
        upcoming_signal.write(UpcomingTrafficSignal(new_next_signal));
        return;
    } else if next_traffic_light.0.is_some() {
        next_traffic_light.0 = None;
    }
    // If there is no upcoming traffic signal, it means that the next
    // traffic light is non-existent or off-screen. We write a Green
    // signal here to keep the vehicle moving.
    upcoming_signal.write(UpcomingTrafficSignal(TrafficSignal::Green));
}
