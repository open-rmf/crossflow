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

//! This example code is used by the handbook to provide code snippets for users.
//! We manage the code snippets inside this example binary to ensure that the
//! example code in the handbook remains up to date with the API of crossflow.
//!
//! Spacing and style may be unusual in this file so that the snippets appear
//! as intended in the handbook. We have disabled rustfmt on this file for that
//! reason.
//!
//! Whenever changes are made to this file, be mindful of the ANCHOR and
//! ANCHOR_END markers because these determine what code is being displayed in
//! the handbook.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused)]

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

    {
        let service = apply_offset_service;
        let request_msg = Vec2::ZERO;
        app.world_mut().command(|commands| {
// ANCHOR: request_service_example
let mut response = commands.request(request_msg, service).take_response();
// ANCHOR_END: request_service_example

// ANCHOR: peek_promise
if let Some(available) = response.peek().as_ref().available() {
    println!("The final response is {available}");
}
// ANCHOR_END: peek_promise

commands.serve(async move {
// ANCHOR: await_promise
match response.await {
    PromiseState::Available(available) => {
        println!("The final response is {available}");
    }
    PromiseState::Cancelled(cancellation) => {
        println!("The request was cancelled: {cancellation}");
    }
    PromiseState::Disposed => {
        // This generally should not happen. It means something wiped out the
        // entities of your request or service.
        println!("Somehow the request was disposed");
    }
    PromiseState::Taken => {
        println!("The final response was taken before you began awaiting the promise");
    }
    PromiseState::Pending => {
        // The promise cannot have this state after being awaited
        unreachable!();
    }
}
// ANCHOR_END: await_promise
});
        });
    }

// ANCHOR: spawn_parsing
let parsing_service = app.spawn_service(parse_values);
// ANCHOR_END: spawn_parsing

    {
        let service = parsing_service;
        app.world_mut().command(|commands| {
// ANCHOR: take_recipient
let mut recipient = commands.request(String::from("-3.14"), parsing_service).take();
// ANCHOR_END: take_recipient

            commands.serve(async move {
// ANCHOR: receive_streams
let _ = recipient.response.await;
println!("The service has finished running.");
while let Some(value) = recipient.streams.parsed_as_u32.recv().await {
    println!("Parsed an unsigned integer: {value}");
}

while let Some(value) = recipient.streams.parsed_as_i32.recv().await {
    println!("Parsed a signed integer: {value}");
}

while let Some(value) = recipient.streams.parsed_as_f32.recv().await {
    println!("Parsed a floating point number: {value}");
}
// ANCHOR_END: receive_streams

// ANCHOR: receive_streams_parallel
use tokio::select;

let next_u32 = recipient.streams.parsed_as_u32.recv();
let next_i32 = recipient.streams.parsed_as_i32.recv();
let next_f32 = recipient.streams.parsed_as_f32.recv();
select! {
    recv = next_u32 => {
        if let Some(value) = recv {
            println!("Received an unsigned integer: {value}");
        }
    }
    recv = next_i32 => {
        if let Some(value) = recv {
            println!("Received a signed integer: {value}");
        }
    }
    recv = next_f32 => {
        if let Some(value) = recv {
            println!("Received a floating point number: {value}");
        }
    }
}
// ANCHOR_END: receive_streams_parallel

            });

// ANCHOR: collect_streams
// Spawn an entity to be used to store the values coming out of the streams.
let storage = commands.spawn(());

// Request the service, but set an entity to collect the streams before we
// take the response.
let response = commands
    .request(String::from("-5"), parsing_service)
    .collect_streams(storage)
    .take_response();

// Save the entity in a resource to keep track of it.
// You could also save this inside a component or move it
// into a callback, or anything else that suits your needs.
commands.insert_resource(StreamStorage(storage));
// ANCHOR_END: collect_streams
        });
    }

    app.world_mut().command(|commands| {
// ANCHOR: simple_series
let storage = commands.spawn(());

commands
    // Ask for three values to be summed
    .request(vec![-1.1, 5.0, 3.1], sum_service)
    // Convert the resulting value to a string
    .map_block(|value| value.to_string())
    // Send the string through the parsing service
    // which will produce a u32, i32, and f32
    .then(parsing_service)
    // Collect the parsed values in an entity
    .collect_streams(storage)
    // Detach this series so we can safely drop the tail
    .detach();
// ANCHOR_END: simple_series
    });
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

// ANCHOR: blocking_service_streams
// ANCHOR: parsed_streams_struct
#[derive(StreamPack)]
struct ParsedStreams {
    /// Values that were successfully parsed to u32
    parsed_as_u32: u32,
    /// Values that were successfully parsed to i32
    parsed_as_i32: i32,
    /// Values that were successfully parsed to f32
    parsed_as_f32: f32,
}
// ANCHOR_END: parsed_streams_struct

fn parse_values(In(srv): BlockingServiceInput<String, ParsedStreams>) {
    if let Ok(value) = srv.request.parse::<u32>() {
        srv.streams.parsed_as_u32.send(value);
    }

    if let Ok(value) = srv.request.parse::<i32>() {
        srv.streams.parsed_as_i32.send(value);
    }

    if let Ok(value) = srv.request.parse::<f32>() {
        srv.streams.parsed_as_f32.send(value);
    }
}
// ANCHOR_END: blocking_service_streams

// ANCHOR: query_stream_storage
use crossflow::Collection;

#[derive(Resource, Deref)]
struct StreamStorage(Entity);

fn print_streams(
    storage: Res<StreamStorage>,
    mut query_u32: Query<&mut Collection<NamedValue<u32>>>,
    mut query_i32: Query<&mut Collection<NamedValue<i32>>>,
    mut query_f32: Query<&mut Collection<NamedValue<f32>>>,
) {
    if let Ok(mut collection_u32) = query_u32.get_mut(*storage) {
        for item in collection_u32.items.drain(..) {
            println!(
                "Received {} from a stream named {} in session {}",
                item.data.value,
                item.data.name,
                item.session,
            );
        }
    }

    if let Ok(mut collection_i32) = query_i32.get_mut(*storage) {
        for item in collection_i32.items.drain(..) {
            println!(
                "Received {} from a stream named {} in session {}",
                item.data.value,
                item.data.name,
                item.session,
            );
        }
    }

    if let Ok(mut collection_f32) = query_f32.get_mut(*storage) {
        for item in collection_f32.items.drain(..) {
            println!(
                "Received {} from a stream named {} in session {}",
                item.data.value,
                item.data.name,
                item.session,
            );
        }
    }
}
// ANCHOR_END: query_stream_storage
