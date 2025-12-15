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

use bevy_ecs::prelude::*;
use bevy_derive::*;
use glam::Vec2;

fn main() {
    let mut app = App::new();
    app.add_plugins(CrossflowExecutorApp::default());

    let service = app.spawn_service(update_page_title);

    let entity = app
        .world_mut()
        .spawn(Url(args.url))
        .id();

    let mut promise = app.world_mut().command(|commands| {
        commands.request(entity, service).take_response()
    });

    // Create a tokio runtime and drive it on another thread
    let (finish, finished) = tokio::sync::oneshot::channel();
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    app.world_mut().insert_resource(TokioRuntime(Arc::clone(&rt)));
    let tokio_thread = std::thread::spawn(move || {
        let _ = rt.block_on(finished);
    });

    let start = std::time::Instant::now();
    let time_limit = std::time::Duration::from_secs(5);
    while std::time::Instant::now() - start < time_limit {
        if let Some(response) = promise.peek().as_ref().available() {
            if response.is_ok() {
                let title = app.world().get::<PageTitle>(entity).unwrap();
                println!("Fetched title: {}", **title);
            } else {
                println!("Error encountered while trying to update title");
            }

            let _ = finish.send(());
            let _ = tokio_thread.join();
            return;
        }

        app.update();
    }

    panic!("Service failed to run within time limit of {time_limit:?}");
}

#[derive(Component, Deref)]
struct Speed(f32);

fn move_towards_target(
    In(srv): ContinuousServiceInput<Vec2, Result<(), ()>>,
    mut query: ContinuousQuery<Vec2, Result<(), ()>>,
    velocities: Query<&Speed>,
    time: Res<Time>,
    mut current_position: Local<Vec2>,
) {
    let Some(mut requests) = query.get_mut(&srv.key) else {
        // The service provider has been despawned, so this continuous service
        // can no longer function.
        return;
    };

    // Get the oldest active request for this service. Orders will be indexed
    // from 0 to N-1 from oldest to newest. When an order is completed, all
    // orders after it will shift down by an index on the next update cycle.
    let Some(mut order) = requests.get_mut(0) else {
        // There are no active requests, so no need to do anything
        return;
    };

    let dt = time.delta_secs_f64() as f32;
    let Ok(velocity) = velocities.get(srv.key.provider()) else {
        // The velocity setting has been taken from the service, so it can no
        // longer complete requests.
        order.respond(Err(()));
        return;
    };

    let target = *order.request();
    match move_to(*current_position, target, **velocity, dt) {
        Ok(_) => {
            // The agent arrived
            *current_position = target;
            order.respond(Ok(()));
        }
        Err(new_position) => {
            // The agent made progress but did not arrive
            *current_position = new_position;
        }
    }
}

struct LaunchRequest {
    target: Vec2,
    speed: f32,
}

// fn launch_towards_target(
//     In(srv): ContinuousServiceInput<Vec2, ()>,
//     mut query: ContinuousQuery<Vec2, ()>,

// )

fn move_to(
    current: Vec2,
    target: Vec2,
    velocity: f32,
    dt: f32,
) -> Result<(), Vec2> {
    let dx = f32::max(0.0, velocity * dt);
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

    return Err(current + u*distance);
}
// ANCHOR_END: example
