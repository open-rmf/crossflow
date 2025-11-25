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

use crossflow::bevy_app::App;
use crossflow::prelude::*;

use bevy_ecs::prelude::*;
use bevy_derive::*;
use glam::Vec2;

fn main() {
    let mut app = App::new();

    // ANCHOR: spawn_sum
    let sum_service: Service<Vec<f32>, f32> = app.spawn_service(sum);
    // ANCHOR_END: spawn_sum

    // ANCHOR: spawn_apply_offset
    let apply_offset_service: Service<Vec2, Vec2> = app.spawn_service(
        apply_offset
        .with(|mut srv: EntityWorldMut| {
            srv.insert(Offset(Vec2::new(-2.0, 3.0)));
        })
    );
    // ANCHOR_END: spawn_apply_offset


}

// ANCHOR: sum_fn
fn sum(In(input): BlockingServiceInput<Vec<f32>>) -> f32 {
    let mut sum = 0.0;
    for value in input.request {
        sum += value;
    }
    sum
}
// ANCHOR_END: sum_fn

// ANCHOR: apply_offset_fn
#[derive(Component, Deref)]
struct Offset(Vec2);

fn apply_offset(
    In(input): BlockingServiceInput<Vec2>,
    offsets: Query<&Offset>,
) -> Vec2 {
    let offset = offsets
        .get(input.provider)
        .map(|offset| **offset)
        .unwrap_or(Vec2::ZERO);

    input.request + offset
}
// ANCHOR_END: apply_offset_fn
