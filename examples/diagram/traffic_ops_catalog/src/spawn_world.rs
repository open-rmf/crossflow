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

use crate::{Lane, MainVehicle, ScrollingWorld, TrafficLightColors, VehicleBundle, VehicleState};
use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
    window::{PrimaryWindow, WindowPlugin, WindowRef},
};
use bevy_color::palettes::css as Colors;
use bevy_egui::EguiPlugin;
use rand::Rng;

// Use Z-values in Transform to handle layering/depth
pub const LANE_LAYER_Z: f32 = 0.0;
pub const ENV_LAYER_Z: f32 = 1.0;
pub const PEDESTRIAN_LAYER_Z: f32 = 8.0;
pub const TRAFFIC_LIGHT_LAYER_Z: f32 = 9.0;
pub const VEHICLE_LAYER_Z: f32 = 10.0;

#[derive(Clone, Debug)]
// TODO(@xiyuoh) allow users to modify these values at runtime, either via the
// user panel or via workflow
pub struct ObstacleLimits {
    pub x_threshold: f32,
    pub y_stop: f32,
    pub y_slow_down: f32,
    pub y_back: f32,
}

impl ObstacleLimits {
    pub fn same_lane(&self, x: f32) -> bool {
        x.abs() < self.x_threshold
    }

    pub fn immediate_obstacle(&self, x: f32, y: f32) -> bool {
        if self.obstacle_behind(y) || !self.same_lane(x) {
            return false;
        }
        y < self.y_stop
    }

    pub fn obstacle_ahead(&self, x: f32, y: f32) -> bool {
        if self.obstacle_behind(y) || !self.same_lane(x) {
            return false;
        }
        y < self.y_slow_down
    }

    pub fn obstacle_behind(&self, y: f32) -> bool {
        y < -self.y_back
    }

    pub fn obstacle_adjacent(&self, x: f32, y: f32, adjacent_space: (f32, f32)) -> Option<Lane> {
        if y.abs() > self.y_stop || self.obstacle_behind(y) {
            return None;
        }
        if x < 0.0 && x.abs() < adjacent_space.0 {
            return Some(Lane::Left);
        } else if x > 0.0 && x.abs() < adjacent_space.1 {
            return Some(Lane::Right);
        }
        None
    }
}

#[derive(Clone, Debug, Resource)]
pub struct WorldLimits {
    pub window_height: f32,
    pub full_runway: f32,
    pub road_center: (f32, f32),
    pub user_panel_width: f32,
    pub lane_limits: (f32, f32),
    pub lane_centers: (f32, f32), // left lane center, right lane center
    pub pavement_limits: (f32, f32),
    pub vehicle_size: (f32, f32),
    pub obstacle_limits: ObstacleLimits,
}

impl FromWorld for WorldLimits {
    fn from_world(world: &mut World) -> Self {
        let mut q_window = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let window_height = q_window.single(world).map(|w| w.height()).unwrap_or(720.0);
        let full_runway = window_height * 4.0;
        let user_panel_width = 320.0;
        let road_center = (-0.5 * user_panel_width, 0.0);
        let (lane_limits, pavement_limits) = {
            let mut window = world.query_filtered::<&Window, With<PrimaryWindow>>();
            if let Ok(window) = window.single(world) {
                let env_width = window.width() - user_panel_width;
                let segment_x = env_width / 6.0;
                let lane_limits = (road_center.0 - segment_x, road_center.0 + segment_x);
                let pavement_limits = (
                    road_center.0 - (2.0 * segment_x),
                    road_center.0 + (2.0 * segment_x),
                );
                (lane_limits, pavement_limits)
            } else {
                (
                    (road_center.0 - 160.0, road_center.0 + 160.0),
                    (road_center.0 - 320.0, road_center.0 + 320.0),
                )
            }
        };
        let half_lane_width = 0.5 * (road_center.0 - lane_limits.0);
        let lane_centers = (
            road_center.0 - half_lane_width,
            road_center.0 + half_lane_width,
        );
        let vehicle_size = (60.0, 100.0);
        let obstacle_limits = ObstacleLimits {
            x_threshold: 0.25 * (lane_limits.1 - lane_limits.0),
            y_stop: vehicle_size.1 * 1.5,
            y_slow_down: vehicle_size.1 * 3.0,
            y_back: vehicle_size.1 * 0.5,
        };

        Self {
            window_height,
            full_runway,
            road_center,
            user_panel_width,
            lane_limits,
            lane_centers,
            pavement_limits,
            vehicle_size,
            obstacle_limits,
        }
    }
}

impl WorldLimits {
    pub fn on_lane(&self, x: f32) -> Option<Lane> {
        if self.lane_limits.0 < x && x < self.road_center.0 {
            return Some(Lane::Left);
        } else if self.road_center.0 < x && x < self.lane_limits.1 {
            return Some(Lane::Right);
        }
        None
    }

    pub fn other_lane(&self, x: f32) -> Option<Lane> {
        self.on_lane(x).map(|lane| lane.inverse())
    }

    pub fn adjacent_lane_space(&self, x: f32) -> Option<(f32, f32)> {
        if x < self.lane_limits.0 || x > self.lane_limits.1 {
            return None;
        }
        Some((x - self.lane_limits.0, self.lane_limits.1 - x))
    }
}

#[derive(Clone, Debug, Resource)]
pub struct WorldMeshes {
    pub vehicle: (Handle<Mesh>, Handle<ColorMaterial>),
    pub lane_dash: (Handle<Mesh>, Handle<ColorMaterial>),
    pub random_box: (Handle<Mesh>, Handle<ColorMaterial>),
}

impl FromWorld for WorldMeshes {
    fn from_world(world: &mut World) -> Self {
        let world_limits = world.resource::<WorldLimits>();
        let window_height = world_limits.window_height.clone();
        let vehicle_size = world_limits.vehicle_size.clone();
        let item_dimension = 50.0;

        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let vehicle_mesh = meshes.add(Rectangle::new(vehicle_size.0, vehicle_size.1));
        let lane_dash_mesh = meshes.add(create_dotted_line_mesh(window_height));
        let random_box_mesh = meshes.add(Rectangle::new(item_dimension, item_dimension));

        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        let vehicle_mat = materials.add(Color::Srgba(Colors::DARK_CYAN));
        let lane_dash_mat = materials.add(Color::Srgba(Colors::ANTIQUE_WHITE));
        let random_box_mat = materials.add(Color::Srgba(Colors::DARK_TURQUOISE));

        Self {
            vehicle: (vehicle_mesh, vehicle_mat),
            lane_dash: (lane_dash_mesh, lane_dash_mat),
            random_box: (random_box_mesh, random_box_mat),
        }
    }
}

#[derive(Clone, Debug, Event)]
pub struct AbandonTrip;

#[derive(Clone, Debug, Component)]
pub struct LaneDash;

#[derive(Default)]
pub struct SpawnWorldPlugin {}

impl Plugin for SpawnWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Crossflow Traffic App".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        });

        app.init_resource::<WorldLimits>()
            .init_resource::<WorldMeshes>()
            .init_resource::<VehicleState>()
            .init_resource::<TrafficLightColors>()
            .add_event::<AbandonTrip>()
            .add_systems(Startup, (spawn_vehicle_and_camera, spawn_environment))
            .add_observer(on_abandon_trip);
    }
}

fn spawn_vehicle_and_camera(
    mut commands: Commands,
    world_limits: Res<WorldLimits>,
    q_window: Query<Entity, With<PrimaryWindow>>,
) {
    let Ok(window_entity) = q_window.single() else {
        return;
    };
    commands.spawn((
        Camera2d::default(),
        Camera {
            target: WindowRef::Entity(window_entity).into(),
            ..default()
        },
    ));

    // Spawn main vehicle
    commands.spawn((
        Sprite {
            color: Color::srgb(0.3, 0.3, 0.9),
            custom_size: Some(Vec2::new(
                world_limits.vehicle_size.0,
                world_limits.vehicle_size.1,
            )),
            ..default()
        },
        Transform::from_xyz(
            world_limits.lane_centers.0, // left_lane
            world_limits.road_center.1 - 100.0,
            VEHICLE_LAYER_Z,
        ),
        VehicleBundle::default(),
        MainVehicle,
    ));
}

fn spawn_environment(
    mut commands: Commands,
    world_limits: Res<WorldLimits>,
    world_meshes: Res<WorldMeshes>,
) {
    let window_height = world_limits.window_height;
    // Spawn lane lines to govern left and right lanes
    let lane_segment_x = [
        world_limits.lane_limits.0,
        world_limits.road_center.0,
        world_limits.lane_limits.1,
    ];
    let lane_segment_y = [
        world_limits.road_center.1,
        world_limits.road_center.1 + window_height,
    ];
    for y in lane_segment_y.iter() {
        for x in lane_segment_x.iter() {
            commands.spawn((
                Mesh2d(world_meshes.lane_dash.0.clone()),
                MeshMaterial2d(world_meshes.lane_dash.1.clone()),
                Transform::from_xyz(x.clone(), y.clone(), LANE_LAYER_Z),
                ScrollingWorld,
                LaneDash,
            ));
        }
    }

    // Spawn random things outside the lane lines
    let item_dimension = 50.0;
    let left_range = (
        world_limits.pavement_limits.0,
        world_limits.lane_limits.0 - item_dimension,
    );
    let right_range = (
        world_limits.lane_limits.1 + item_dimension,
        world_limits.pavement_limits.1,
    );
    // NOTE(@xiyuoh) these are NOT other VEHICLES, these are static objects in
    // the world. Other cars should not be part of ScrollingWorld as they should
    // also have some sort of velocity.
    let mut rng = rand::rng();
    let n_objects = rng.random_range(10..20);
    for i in 0..n_objects {
        let x = if i % 2 == 0 {
            rng.random_range(left_range.0..left_range.1)
        } else {
            rng.random_range(right_range.0..right_range.1)
        };
        let y = rng.random_range(0.0..world_limits.full_runway);

        commands.spawn((
            ((
                Mesh2d(world_meshes.random_box.0.clone()),
                MeshMaterial2d(world_meshes.random_box.1.clone()),
                Transform::from_xyz(x, y, ENV_LAYER_Z),
            )),
            ScrollingWorld,
        ));
    }
}

fn create_dotted_line_mesh(window_height: f32) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let interval = 30.0;
    let (dash_width, dash_height) = (2.0, 20.0);

    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut index_count = 0;

    let mut y = -window_height;
    while y < window_height {
        let x_min = -dash_width / 2.0;
        let x_max = dash_width / 2.0;
        let y_min = y - dash_height / 2.0;
        let y_max = y + dash_height / 2.0;

        positions.extend_from_slice(&[
            [x_min, y_min, 0.0],
            [x_max, y_min, 0.0],
            [x_max, y_max, 0.0],
            [x_min, y_max, 0.0],
        ]);
        indices.extend_from_slice(&[
            index_count,
            index_count + 1,
            index_count + 2,
            index_count,
            index_count + 2,
            index_count + 3,
        ]);

        index_count += 4;
        y += interval;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

fn on_abandon_trip(_trigger: Trigger<AbandonTrip>, mut vehicle_state: ResMut<VehicleState>) {
    info!("Received request to abandon trip!");
    vehicle_state.reset();
}
