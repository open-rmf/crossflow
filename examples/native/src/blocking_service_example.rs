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
use crossflow::bevy_app::App;
use crossflow::prelude::*;

use bevy_derive::*;
use bevy_ecs::prelude::*;
use glam::Vec2;

fn main() {
    let mut app = App::new();
    app.add_plugins(CrossflowExecutorApp::default());

    let offset = Vec2::new(-2.0, 3.0);

    let service = app.spawn_service(apply_offset.with(|mut srv: EntityWorldMut| {
        srv.insert(Offset(offset));
    }));

    let mut promise = app
        .world_mut()
        .command(|commands| commands.request(Vec2::ZERO, service).take_response());

    for _ in 0..5 {
        if let Some(response) = promise.peek().as_ref().available() {
            assert_eq!(response, &offset);
            println!("Successfully applied offset: {response:?}");
            return;
        }

        app.update();
    }

    panic!("Service failed to run after multiple updates");
}

#[derive(Component, Deref)]
struct Offset(Vec2);

fn apply_offset(In(input): BlockingServiceInput<Vec2>, offsets: Query<&Offset>) -> Vec2 {
    let offset = offsets
        .get(input.provider)
        .map(|offset| **offset)
        .unwrap_or(Vec2::ZERO);

    input.request + offset
}
// ANCHOR_END: example
