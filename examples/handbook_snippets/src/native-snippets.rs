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

use crossflow::bevy_app::{App, Update};
use crossflow::prelude::*;

use async_std::future::{pending, timeout};
use bevy_ecs::prelude::*;
use bevy_derive::*;
use bevy_time::Time;
use glam::Vec2;

use serde::{Serialize, Deserialize};
use serde_json::{Value as Json, Error};

use std::{
    collections::HashMap,
    time::{Duration, SystemTime}
};

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

// ANCHOR: spawn_trivial_async_service
let async_service: Service<String, String> = app.spawn_service(trivial_async_service);
// ANCHOR_END: spawn_trivial_async_service

// ANCHOR: spawn_hello_continuous_service
let continuous_service: Service<String, String> = app.spawn_continuous_service(
    Update,
    hello_continuous_service
);
// ANCHOR_END: spawn_hello_continuous_service

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
let storage = commands.spawn(()).id();

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
let storage = commands.spawn(()).id();

commands
    // Ask for three values to be summed
    .request(vec![-1.1, 5.0, 3.1], sum_service)
    // Convert the resulting value to a string
    .map_block(|value| value.to_string())
    // Send the string through the parsing service
    // which may produce streams of u32, i32, and f32
    .then(parsing_service)
    // Collect the parsed values in an entity
    .collect_streams(storage)
    // Detach this series so we can safely drop the tail
    .detach();
// ANCHOR_END: simple_series
    });

    app.world_mut().command(|commands| {

    type Request = String;
    type Response = String;
// ANCHOR: trivial_workflow
let workflow: Service<Request, Response> = commands.spawn_io_workflow(
    |scope: Scope<Request, Response>, builder: &mut Builder| {
        builder.connect(scope.start, scope.terminate);
    }
);
// ANCHOR_END: trivial_workflow

// ANCHOR: trivial_workflow_concise
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder.connect(scope.start, scope.terminate);
    }
);
// ANCHOR_END: trivial_workflow_concise
        help_service_infer_type::<String, String, ()>(workflow);

// ANCHOR: sum_service_workflow
// Spawn a service that we can use inside a workflow
let service = commands.spawn_service(sum);

// Spawn a workflow and use the service inside it
let workflow = commands.spawn_io_workflow(
    move |scope, builder| {
        let node = builder.create_node(service);
        builder.connect(scope.start, node.input);
        builder.connect(node.output, scope.terminate);
    }
);
// ANCHOR_END: sum_service_workflow

// ANCHOR: sum_nested_service_workflow
let workflow = commands.spawn_io_workflow(
    move |scope, builder| {
        // Spawn a service using the builder's commands
        let service = builder.commands().spawn_service(sum);

        // Create the node using the newly spawned service
        let node = builder.create_node(service);
        builder.connect(scope.start, node.input);
        builder.connect(node.output, scope.terminate);
    }
);
// ANCHOR_END: sum_nested_service_workflow

// ANCHOR: sum_callback_workflow
// Define a closure to perform a sum
let f = |request: Vec<f32>| -> f32 {
    request.into_iter().fold(0.0, |a, b| a + b)
};
// Convert the closure into a Callback
let callback = f.into_blocking_callback();

// Spawn a workflow and use the callback inside it
let workflow = commands.spawn_io_workflow(
    move |scope, builder| {
        let node = builder.create_node(callback);
        builder.connect(scope.start, node.input);
        builder.connect(node.output, scope.terminate);
    }
);
// ANCHOR_END: sum_callback_workflow

// ANCHOR: sum_map_workflow
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        let node = builder.create_map_block(|request: Vec<f32>| {
            request.into_iter().fold(0.0, |a, b| a + b)
        });

        builder.connect(scope.start, node.input);
        builder.connect(node.output, scope.terminate);
    }
);
// ANCHOR_END: sum_map_workflow

// ANCHOR: async_map_workflow
let workflow = commands.spawn_io_workflow(
    |scope, builder| {

        let node = builder.create_map_async(get_page_title);

        builder.connect(scope.start, node.input);
        builder.connect(node.output, scope.terminate);
    }
);
// ANCHOR_END: async_map_workflow

// ANCHOR: async_map_nested_workflow
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        let node = builder.create_map_async(|url: String| {
            async move {
                let http_response = trpl::get(&url).await;
                let response_text = http_response.text().await;
                trpl::Html::parse(&response_text)
                    .select_first("title")
                    .map(|title| title.inner_html())
            }
        });

        builder.connect(scope.start, node.input);
        builder.connect(node.output, scope.terminate);
    }
);
// ANCHOR_END: async_map_nested_workflow

// ANCHOR: basic_connect_nodes
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        let sum_node = builder.create_map_block(|request: Vec<f32>| {
            request.into_iter().fold(0.0, |a, b| a + b)
        });
        let double_node = builder.create_map_block(|request: f32| {
            2.0 * request
        });

        builder.connect(scope.start, sum_node.input);
        builder.connect(sum_node.output, double_node.input);
        builder.connect(double_node.output, scope.terminate);
    }
);
// ANCHOR_END: basic_connect_nodes

let sum = (|request: Vec<f32>|
    request.into_iter().fold(0.0, |a, b| a + b)
).into_blocking_map();
let double = (|request: f32| 2.0 * request).into_blocking_map();

// ANCHOR: chain_services
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .then(sum)
            .then(double)
            .connect(scope.terminate);
    }
);
// ANCHOR_END: chain_services

// ANCHOR: chain_maps
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .map_block(|request: Vec<f32>| {
                request.into_iter().fold(0.0, |a, b| a + b)
            })
            .map_block(|request: f32| {
                2.0 * request
            })
            .connect(scope.terminate);
    }
);
// ANCHOR_END: chain_maps

// ANCHOR: fork_result_workflow
let workflow: Service<Json, Result<SchemaV2, Error>> = commands.spawn_io_workflow(
    |scope, builder| {
        let parse_schema_v2 = builder.create_map_block(|request: Json| {
            // Try parsing the JSON with Schema V2
            serde_json::from_value::<SchemaV2>(request.clone())
                // If an error happened, pass along the original request message
                // so we can try parsing it with a different schema.
                .map_err(|_| request)
        });

        let parse_schema_v1 = builder.create_map_block(|request: Json| {
            // Try parsing the JSON with Schema V1 since V2 failed.
            serde_json::from_value::<SchemaV1>(request)
                // If the parsing was successful, upgrade the parsed value to
                // SchemaV2.
                .map(|value| value.upgrade_to_schema_v2())
        });

        let to_ok = builder.create_map_block(|request: SchemaV2| {
            Ok(request)
        });

        // Create a fork-result operation. We get back a tuple whose first element
        // is an InputSlot that lets us feed messages into the fork-result, and
        // whose second element is a struct containing two fields: ok and err,
        // each representing a different Output and therefore diverging branches
        // in the workflow.
        let (fork_result_input, fork_result) = builder.create_fork_result();

        builder.connect(scope.start, parse_schema_v2.input);
        builder.connect(parse_schema_v2.output, fork_result_input);

        // If parsing SchemaV2 was successful, wrap it back in Ok and terminate
        builder.connect(fork_result.ok, to_ok.input);
        builder.connect(to_ok.output, scope.terminate);

        // If we failed to parse the Json as SchemaV2 then try using SchemaV1 instead
        builder.connect(fork_result.err, parse_schema_v1.input);

        // If parsing SchemaV1 also fails then we have no more fallback, so just
        // pass back the result, whether it was successful or failed.
        builder.connect(parse_schema_v1.output, scope.terminate);
    }
);
// ANCHOR_END: fork_result_workflow

// ANCHOR: fork_result_chain
let workflow: Service<Json, Result<SchemaV2, Error>> = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .map_block(|message: Json| {
                // Try parsing the JSON with Schema V2
                serde_json::from_value::<SchemaV2>(message.clone())
                    // If an error happened, pass along the original request message
                    // so we can try parsing it with a different schema.
                    .map_err(|_| message)
            })
            .fork_result(
                |ok| {
                    // If ok, wrap the message in Ok and connect it to terminate
                    ok.map_block(|msg| Ok(msg)).connect(scope.terminate);
                },
                |err| {
                    err
                    .map_block(|message: Json| {
                        // Try parsing the JSON with Schema V1 since V2 failed.
                        serde_json::from_value::<SchemaV1>(message)
                            // If the parsing was successful, upgrade the parsed
                            // value to SchemaV2.
                            .map(|value| value.upgrade_to_schema_v2())
                    })
                    // End this branch by feeding it into the terminate operation
                    .connect(scope.terminate);
                }
            );
    }
);
// ANCHOR_END: fork_result_chain

// ANCHOR: branch_for_err
let workflow: Service<Json, Result<SchemaV2, Error>> = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .map_block(|request: Json| {
                // Try parsing the JSON with Schema V2
                serde_json::from_value::<SchemaV2>(request.clone())
                    // If an error happened, pass along the original request message
                    // so we can try parsing it with a different schema.
                    .map_err(|_| request)
            })
            // Create a branch that handles an Err value. This creates a
            // fork-result under the hood.
            .branch_for_err(|chain|
                chain
                .map_block(|request: Json| {
                    // Try parsing the JSON with Schema V1 since V2 failed.
                    serde_json::from_value::<SchemaV1>(request)
                        // If the parsing was successful, upgrade the parsed value to
                        // SchemaV2.
                        .map(|value| value.upgrade_to_schema_v2())
                })
                // End this branch by feeding it into the terminate operation
                .connect(scope.terminate)
            )
            // Continue the original chain, but only for Ok values.
            .map_block(|ok| Ok(ok))
            .connect(scope.terminate);
    }
);
// ANCHOR_END: branch_for_err

// ANCHOR: fork_option_workflow
let workflow: Service<(), f32> = commands.spawn_io_workflow(
    |scope, builder| {
        let get_random = builder.create_map_block(|request: ()| {
            // Generate some random number between 0.0 and 1.0
            rand::random::<f32>()
        });

        let less_than_half = builder.create_map_block(|value: f32| {
            if value < 0.5 {
                Some(value)
            } else {
                None
            }
        });

        // Create a fork-option operation. We get back a tuple whose first element
        // is an InputSlot that lets us feed messages into the fork-option, and
        // whose second element is a struct containing two fields: some and none,
        // each representing a different Output and therefore diverging branches
        // in the workflow.
        let (fork_option_input, fork_option) = builder.create_fork_option();

        // Chain the three operations together.
        builder.connect(scope.start, get_random.input);
        builder.connect(get_random.output, less_than_half.input);
        builder.connect(less_than_half.output, fork_option_input);

        // Trigger the randomizer again if the value was not less than one-half.
        // This creates a cycle in the workflow.
        builder.connect(fork_option.none, get_random.input);

        // Terminate the workflow if it was less than one-half.
        // The value produced by the randomizer will be the workflow's output.
        builder.connect(fork_option.some, scope.terminate);
    }
);
// ANCHOR_END: fork_option_workflow

// ANCHOR: fork_option_chain
let workflow: Service<(), f32> = commands.spawn_io_workflow(
    |scope, builder| {
        // Make a small chain that returns a Node. We need to create an explicit
        // Node for get_random because we will need to refer to its InputSlot
        // later to create a cycle.
        let get_random: Node<(), f32> = builder
            .chain(scope.start)
            .map_block_node(|request: ()| rand::random::<f32>());

        builder
            .chain(get_random.output)
            .map_block(|value: f32| {
                if value < 0.5 {
                    Some(value)
                } else {
                    None
                }
            })
            // This creates a fork-option and sends None values back to the
            // get_random node. This creates a cycle in the workflow.
            .branch_for_none(|none| none.connect(get_random.input))
            // As we continue the chain, only Some(T) values will reach this
            // point, so the chain simplifies the Option<T> to just a T. We can
            // now connect this directly to the terminate operation.
            .connect(scope.terminate);
    }
);
// ANCHOR_END: fork_option_chain

// Dummy providers to build the emergency_stop_workflow
let move_to_pick_pregrasp = (|_: ()| { }).into_blocking_map();
let grasp_item = (|_: ()| { }).into_blocking_map();
let move_to_placement = (|_: ()| { }).into_blocking_map();
let release_item = (|_: ()| { }).into_blocking_map();
let emergency_stop = (|_: ()| { }).into_blocking_map();

// ANCHOR: emergency_stop_workflow
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        // Create nodes for performing a pick and place
        let move_to_pick_pregrasp = builder.create_node(move_to_pick_pregrasp);
        let grasp_item = builder.create_node(grasp_item);
        let move_to_placement = builder.create_node(move_to_placement);
        let release_item = builder.create_node(release_item);

        // Also create a node that monitors whether an emergency stop is needed
        let emergency_stop = builder.create_node(emergency_stop);

        // Create a fork-clone operation. We get back a tuple whose first element
        // is an InputSlot that lets us feed messages into the fork-clone, and
        // whose second element is a struct that allows us to spawn outputs for
        // the fork.
        let (fork_clone_input, fork_clone) = builder.create_fork_clone();

        // Send the scope input message to be cloned
        builder.connect(scope.start, fork_clone_input);

        // When the scope starts, begin moving the robot to the pregrasp pose
        let cloned = fork_clone.clone_output(builder);
        builder.connect(cloned, move_to_pick_pregrasp.input);

        // When the scope starts, also start monitoring whether an emergency
        // stop is needed. If this gets triggered it will terminate the workflow
        // immediately.
        let cloned = fork_clone.clone_output(builder);
        builder.connect(cloned, emergency_stop.input);

        // Connect the happy path together
        builder.connect(move_to_pick_pregrasp.output, grasp_item.input);
        builder.connect(grasp_item.output, move_to_placement.input);
        builder.connect(move_to_placement.output, release_item.input);
        builder.connect(release_item.output, scope.terminate);

        // Connect the emergency stop to terminate
        builder.connect(emergency_stop.output, scope.terminate);
    }
);
// ANCHOR_END: emergency_stop_workflow

// Dummy providers to build the emergency_stop_chain
let move_to_pick_pregrasp = (|_: ()| { }).into_blocking_map();
let grasp_item = (|_: ()| { }).into_blocking_map();
let move_to_placement = (|_: ()| { }).into_blocking_map();
let release_item = (|_: ()| { }).into_blocking_map();
let emergency_stop = (|_: ()| { }).into_blocking_map();

// ANCHOR: emergency_stop_chain
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .branch_clone(|chain|
                // This is a parallel branch fed with a clone of the scope input.
                chain
                .then(emergency_stop)
                .connect(scope.terminate)
            )
            // As we continue to build this chain, we are creating a branch that
            // will run in parallel to the one defined inside of .branch_clone(_).
            // This is where we'll define the happy path sequence of the
            // pick-and-place routine.
            .then(move_to_pick_pregrasp)
            .then(grasp_item)
            .then(move_to_placement)
            .then(release_item)
            .connect(scope.terminate);
    }
);
// ANCHOR_END: emergency_stop_chain

// Dummy providers to build the use_elevator_workflow
let move_robot_to_elevator = (|_: ()| { }).into_blocking_map();
let on_robot_near_elevator = (|_: ()| { }).into_blocking_map();
let send_elevator_to_location = (|_: ()| { }).into_blocking_map();
let use_elevator = (|_: ((), ())| { }).into_blocking_map();

// ANCHOR: use_elevator_workflow
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        // Create nodes for moving the robot and summoning the elevator.
        let move_robot_to_elevator = builder.create_node(move_robot_to_elevator);
        let on_robot_near_elevator = builder.create_node(on_robot_near_elevator);
        let send_elevator_to_level = builder.create_node(send_elevator_to_location);
        let use_elevator = builder.create_node(use_elevator);

        // Create a fork-clone operation. We get back a tuple whose first element
        // is an InputSlot that lets us feed messages into the fork-clone, and
        // whose second element is a struct that allows us to spawn outputs for
        // the fork.
        let (fork_clone_input, fork_clone) = builder.create_fork_clone();

        // Send the scope input message to be cloned
        builder.connect(scope.start, fork_clone_input);

        // When the scope starts, begin sending the robot to the elevator
        let cloned = fork_clone.clone_output(builder);
        builder.connect(cloned, move_robot_to_elevator.input);

        // When the scope starts, also start detecting whether the robot is
        // near the elevator so we know when to summon the elevator
        let cloned = fork_clone.clone_output(builder);
        builder.connect(cloned, on_robot_near_elevator.input);

        // When the robot has made it close enough to the elevator, begin
        // summoning the elevator
        builder.connect(on_robot_near_elevator.output, send_elevator_to_level.input);

        // Create a join operation that will activate when the robot has reached
        // the elevator lobby and the elevator has arrived on the correct floor.
        let both_arrived = builder.join((
            move_robot_to_elevator.output,
            send_elevator_to_level.output,
        ))
        .output();

        // When the robot has reached the elevator lobby and the elevator has
        // arived on the correct floor, have the robot use the elevator.
        builder.connect(both_arrived, use_elevator.input);

        // When the robot is done using the elevator, the workflow is finished.
        builder.connect(use_elevator.output, scope.terminate);
    }
);
// ANCHOR_END: use_elevator_workflow

// Dummy providers to build the use_elevator_chain
let move_robot_to_elevator = (|_: ()| { }).into_blocking_map();
let on_robot_near_elevator = (|_: ()| { }).into_blocking_map();
let send_elevator_to_location = (|_: ()| { }).into_blocking_map();
let use_elevator = (|_: ((), ())| { }).into_blocking_map();

// ANCHOR: use_elevator_chain
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .fork_clone((
                |chain: Chain<_>| {
                    // This branch moves the robot to the elevator
                    chain
                    .then(move_robot_to_elevator)
                    .output()
                },
                |chain: Chain<_>| {
                    // This branch monitors the robot and then summons the lift
                    chain
                    .then(on_robot_near_elevator)
                    .then(send_elevator_to_location)
                    .output()
                }
            ))
            .join(builder)
            .then(use_elevator)
            .connect(scope.terminate);
    }
);
// ANCHOR_END: use_elevator_chain

let pick_item = (|_: WorkcellTask| { }).into_blocking_map();
let move_to_location = (|_: MobileRobotTask| { }).into_blocking_map();
let hand_off_item = (|_: ((), ())| { }).into_blocking_map();

// ANCHOR: unzip_workflow
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        // Create ndoes for picking an item and moving to a pickup location
        let pick_item = builder.create_node(pick_item);
        let move_to_location = builder.create_node(move_to_location);
        let hand_off_item = builder.create_node(hand_off_item);

        // Create a blocking map to transform the workflow input data into two
        // separate messages to send to two different branches.
        //
        // This returns a tuple with two elements. We will send each element to
        // a different branch at the same time.
        let transform_inputs = builder.create_map_block(|request: PickupTask| {
            (
                WorkcellTask {
                    workcell: request.workcell,
                    item: request.item,
                },
                MobileRobotTask {
                    vehicle: request.vehicle,
                    location: request.location,
                }
            )
        });

        // Create the unzip forking
        let (unzip_input, unzip) = builder.create_unzip();
        // Destructure the unzipped outputs
        let (workcell_task, mobile_robot_task) = unzip;

        // Synchronize when the workcell and mobile robot are both ready for the
        // item to be handed off
        let both_ready = builder.join((
            pick_item.output,
            move_to_location.output,
        ))
        .output();

        // Connect all the nodes
        builder.connect(scope.start, transform_inputs.input);
        builder.connect(transform_inputs.output, unzip_input);
        builder.connect(workcell_task, pick_item.input);
        builder.connect(mobile_robot_task, move_to_location.input);
        builder.connect(both_ready, hand_off_item.input);
        builder.connect(hand_off_item.output, scope.terminate);
    }
);
// ANCHOR_END: unzip_workflow

let pick_item = (|_: WorkcellTask| { }).into_blocking_map();
let move_to_location = (|_: MobileRobotTask| { }).into_blocking_map();
let hand_off_item = (|_: ((), ())| { }).into_blocking_map();

// ANCHOR: unzip_chain
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .map_block(|request: PickupTask| {
                (
                    WorkcellTask {
                        workcell: request.workcell,
                        item: request.item,
                    },
                    MobileRobotTask {
                        vehicle: request.vehicle,
                        location: request.location,
                    }
                )
            })
            .fork_unzip((
                |workcell_branch: Chain<_>| workcell_branch.then(pick_item).output(),
                |amr_branch: Chain<_>| amr_branch.then(move_to_location).output(),
            ))
            .join(builder)
            .then(hand_off_item)
            .connect(scope.terminate);
    }
);
// ANCHOR_END: unzip_chain

type T = String;
// ANCHOR: minimal_workflow_stream_example
let workflow = commands.spawn_workflow(
    |scope: Scope<_, _, StreamOf<T>>, builder| {
        /* ... */
    }
);
// ANCHOR_END: minimal_workflow_stream_example
help_service_infer_type::<String, String, StreamOf<T>>(workflow);

let deposit_apples = commands.spawn_service(
    |In(input): BlockingServiceInput<Vec<Apple>, StreamOf<Apple>>| {
        for apple in input.request {
            input.streams.send(apple);
        }
    }
);

let try_take_apple = commands.spawn_service(
    |
        In(input): BlockingServiceInput<((), BufferKey<Apple>)>,
        mut access: BufferAccessMut<Apple>,
    | {
        access.get_mut(&input.request.1).ok()?.pull()
    }
);

let chop_apple = commands.spawn_service(
    |In(input): BlockingServiceInput<Apple, StreamOf<AppleSlice>>| {
        input.streams.send(AppleSlice { });
    }
);

// ANCHOR: apple_stream_out
let workflow = commands.spawn_workflow(
    |scope: Scope<_, _, StreamOf<AppleSlice>>, builder| {
        // Create the service nodes that will be involved in chopping apples
        let deposit_apples = builder.create_node(deposit_apples);
        let try_take_apple = builder.create_node(try_take_apple);
        let (have_apple_input, have_apple) = builder.create_fork_option();
        let chop_apple = builder.create_node(chop_apple);

        // Create a buffer to hold apples that are waiting to be chopped, and
        // an operation to access that buffer.
        let apple_buffer = builder.create_buffer(BufferSettings::keep_all());
        let access_apple_buffer = builder.create_buffer_access(apple_buffer);

        // Connect the scope input message to the deposit_apples service
        builder.connect(scope.start, deposit_apples.input);

        // Connect the stream of incoming apples to the apple buffer
        builder.connect(deposit_apples.streams, apple_buffer.input_slot());

        // When done depositing apples, start trying to take them by accessing
        // the buffer
        builder.connect(deposit_apples.output, access_apple_buffer.input);
        builder.connect(access_apple_buffer.output, try_take_apple.input);

        // Try to take an apple and check if we ran out
        builder.connect(try_take_apple.output, have_apple_input);
        // If there's another apple, send it to be chopped
        builder.connect(have_apple.some, chop_apple.input);
        // If there are no more apples, terminate the workflow
        builder.connect(have_apple.none, scope.terminate);

        // Stream the apple slices out of the scope, and then cycle back to
        // taking another apple when the current one is finished.
        builder.connect(chop_apple.streams, scope.streams);
        builder.connect(chop_apple.output, access_apple_buffer.input);
    }
);
// ANCHOR_END: apple_stream_out

// ANCHOR: navigation_streams_workflow
// This service will have a mobile robot approach a door.
let approach_door = commands.spawn_service(
    |In(input): BlockingServiceInput<(), NavigationStreams>| {
        input.streams.log.send(String::from("approaching door"));
        /* ... approach the door ... */
    }
);

// open_door is not a navigation service so it will only have one
// output stream: log messages.
let open_door = commands.spawn_service(
    |In(input): BlockingServiceInput<(), StreamOf<String>>| {
        input.streams.send(String::from("opening door"));
        /* ... open the door ... */
    }
);

// This service will have a mobile robot move through a door.
let move_through_door = commands.spawn_service(
    |In(input): BlockingServiceInput<(), NavigationStreams>| {
        input.streams.log.send(String::from("moving through door"));
        /* ... move through the door ... */
    }
);

// This workflow will handle the whole process of moving a robot through a door,
// while streaming navigation and log information from all the services it runs.
let workflow = commands.spawn_workflow(
    |scope: Scope<_, _, NavigationStreams>, builder| {
        let approach_door = builder.create_node(approach_door);
        let open_door = builder.create_node(open_door);
        let move_through_door = builder.create_node(move_through_door);

        // Connect nodes together
        builder.connect(scope.start, approach_door.input);
        builder.connect(approach_door.output, open_door.input);
        builder.connect(open_door.output, move_through_door.input);
        builder.connect(move_through_door.output, scope.terminate);

        // Connect node streams to scope streams
        builder.connect(approach_door.streams.log, scope.streams.log);
        builder.connect(approach_door.streams.location, scope.streams.location);
        builder.connect(approach_door.streams.error, scope.streams.error);

        builder.connect(open_door.streams, scope.streams.log);

        builder.connect(move_through_door.streams.log, scope.streams.log);
        builder.connect(move_through_door.streams.location, scope.streams.location);
        builder.connect(move_through_door.streams.error, scope.streams.error);
    }
);
// ANCHOR_END: navigation_streams_workflow

// ANCHOR: buffer_settings_keep_all
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        let lidar_buffer = builder.create_buffer(BufferSettings::keep_all());
        let camera_buffer = builder.create_buffer(BufferSettings::keep_all());

        let localization_data = builder.join(
            LocalizationData::select_buffers(lidar_buffer, camera_buffer)
        );
    }
);
// ANCHOR_END: buffer_settings_keep_all
        help_service_infer_type::<(), (), ()>(workflow);

// ANCHOR: join_settings_clone
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        let location_buffer = builder.create_buffer(BufferSettings::default());
        let camera_buffer = builder.create_buffer(BufferSettings::default());

        let image_stamped = builder.join(ImageStamped::select_buffers(
            // Use .join_by_cloning() to have the location data cloned instead
            // of pulled for each join operation.
            location_buffer.join_by_cloning(),
            camera_buffer,
        ));
    }
);
// ANCHOR_END: join_settings_clone
        help_service_infer_type::<(), (), ()>(workflow);

let traffic_signal_service = commands.spawn_service(
    |_: In<BlockingService<(), StreamOf<TrafficSignal>>>| { }
);
let approach_intersection = commands.spawn_service(
    |_: In<BlockingService<()>>| { [1_f32, 2_f32] }
);
let send_robot_command = commands.spawn_service(
    |_: In<BlockingService<RobotCommand>>| -> Option<()> { None }
);

// ANCHOR: listen_example
/// Derive the Accessor trait so this struct of keys can be constructed by the
/// listen operation. Note that Accessor also requires Clone to be defined.
#[derive(Accessor, Clone)]
struct IntersectionKeys {
    signal: BufferKey<TrafficSignal>,
    arrival: BufferKey<[f32; 2]>,
}

/// Define a device that evaluates whether or not the robot should proceed
/// across the intersection.
fn proceed_or_stop(
    In(keys): In<IntersectionKeys>,
    signal_access: BufferAccess<TrafficSignal>,
    mut arrival_access: BufferAccessMut<[f32; 2]>,
) -> Option<RobotCommand> {
    let signal_buffer = signal_access.get(&keys.signal).ok()?;
    let mut arrival_buffer = arrival_access.get_mut(&keys.arrival).ok()?;

    // Get a reference to the newest message if one is available
    let signal = signal_buffer.newest()?;
    // Pull the value from this buffer is one is available
    let arrived = arrival_buffer.pull()?;

    match signal {
        TrafficSignal::Green => Some(RobotCommand::Go),
        TrafficSignal::Red => Some(RobotCommand::Stop),
    }
}

let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        // Create buffers to store the two state variables:
        // * what the current traffic signal is
        // * whether the robot has reached the intersection
        let signal = builder.create_buffer(Default::default());
        let arrived = builder.create_buffer(Default::default());

        // Create services to update the two state variables
        let traffic_signal_service = builder.create_node(traffic_signal_service);
        let approach_intersection = builder.create_node(approach_intersection);

        // Activate both of the state update services at startup
        builder
            .chain(scope.start)
            .fork_clone((
                |chain: Chain<_>| chain.connect(traffic_signal_service.input),
                |chain: Chain<_>| chain.connect(approach_intersection.input),
            ));

        // Connect the services to their respective buffers. For the traffic
        // signal we connect its stream, because the signal will be changing
        // over time. For the arrival state we connect the output of the
        // approach_intersection service because the robot will only ever
        // approach the intersection once.
        builder.connect(traffic_signal_service.streams, signal.input_slot());
        builder.connect(approach_intersection.output, arrived.input_slot());

        builder
            // Create a listen operation that will activate whenever either the
            // traffic signal or arrival buffer has an update
            .listen(IntersectionKeys::select_buffers(signal, arrived))
            // When an update happens, provide both buffer keys to a node that
            // will evaluate if the robot is ready to cross
            .then(proceed_or_stop.into_blocking_callback())
            // If no decision can be made yet (e.g. one of the buffers is
            // unavailable) then just dispose this message
            .dispose_on_none()
            // If a decision was made, send a command to the robot
            .then(send_robot_command)
            // The previous service will return None if the command was to stop.
            // If the command was to go, then it will return Some after the robot
            // finishes crossing the intersection.
            .dispose_on_none()
            // Terminate when the robot has finished crossing.
            .connect(scope.terminate);
    }
);
// ANCHOR_END: listen_example

// ANCHOR: explicit_workflow_settings
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder.connect(scope.start, scope.terminate);

        // Return explicit workflow settings.
        WorkflowSettings::new()
            .uninterruptible()
            .with_delivery(DeliverySettings::Serial)
    }
);
// ANCHOR_END: explicit_workflow_settings
        help_service_infer_type::<(), (), ()>(workflow);

// ANCHOR: explicit_delivery_settings
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder.connect(scope.start, scope.terminate);

        // Return explicit delivery settings.
        // The scope settings will be the default (interruptible).
        DeliverySettings::Serial
    }
);
// ANCHOR_END: explicit_delivery_settings
        help_service_infer_type::<(), (), ()>(workflow);

// ANCHOR: explicit_scope_settings
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder.connect(scope.start, scope.terminate);

        // Return explicit scope settings.
        // The delivery settings will be the default (parallel).
        ScopeSettings::uninterruptible()
    }
);
// ANCHOR_END: explicit_scope_settings
        help_service_infer_type::<(), (), ()>(workflow);

// ANCHOR: default_workflow_settings
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder.connect(scope.start, scope.terminate);

        // Simply don't return anything from the closure
        // to get the default workflow settings.
    }
);
// ANCHOR_END: default_workflow_settings
        help_service_infer_type::<(), (), ()>(workflow);

// ANCHOR: inner_scope_settings
let workflow = commands.spawn_io_workflow(
    |scope, builder| {
        builder
            .chain(scope.start)
            .then_io_scope(
                |scope, builder| {
                    builder.connect(scope.start, scope.terminate);

                    // Set only this nested scope to be uninterruptible.
                    ScopeSettings::uninterruptible()
                }
            )
            .connect(scope.terminate);
    }
);
// ANCHOR_END: inner_scope_settings
        help_service_infer_type::<(), (), ()>(workflow);
    });
}

enum TrafficSignal {
    Green,
    Red,
}

enum RobotCommand {
    Go,
    Stop,
}

struct LidarData {}
struct CameraData {}

// ANCHOR: LocalizationData
#[derive(Joined)]
struct LocalizationData {
    lidar: LidarData,
    camera: CameraData,
}
// ANCHOR_END: LocalizationData

#[derive(Joined)]
struct ImageStamped {
    location: Location,
    image: CameraData,
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
    if let Ok(mut collection_u32) = query_u32.get_mut(**storage) {
        for item in collection_u32.items.drain(..) {
            println!(
                "Received {} from a stream named {} in session {}",
                item.data.value,
                item.data.name,
                item.session,
            );
        }
    }

    if let Ok(mut collection_i32) = query_i32.get_mut(**storage) {
        for item in collection_i32.items.drain(..) {
            println!(
                "Received {} from a stream named {} in session {}",
                item.data.value,
                item.data.name,
                item.session,
            );
        }
    }

    if let Ok(mut collection_f32) = query_f32.get_mut(**storage) {
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

// ANCHOR: trivial_async_service
async fn trivial_async_service(In(srv): AsyncServiceInput<String>) -> String {
    return srv.request;
}
// ANCHOR_END: trivial_async_service

// ANCHOR: page_title_service
async fn page_title_service(In(srv): AsyncServiceInput<String>) -> Option<String> {
    let response = trpl::get(&srv.request).await;
    let response_text = response.text().await;
    trpl::Html::parse(&response_text)
        .select_first("title")
        .map(|title| title.inner_html())
}
// ANCHOR_END: page_title_service

// ANCHOR: get_page_title
async fn get_page_title(url: String) -> Option<String> {
    let http_response = trpl::get(&url).await;
    let response_text = http_response.text().await;
    trpl::Html::parse(&response_text)
        .select_first("title")
        .map(|title| title.inner_html())
}
// ANCHOR_END: get_page_title

// ANCHOR: insert_page_title
/// A component that stores a web page title inside an Entity
#[derive(Component)]
struct PageTitle(String);

/// A request that specifies a URL whose page title should be stored inside an Entity
struct PageTitleRequest {
    url: String,
    insert_into: Entity,
}

/// A service that fetches the page title of a URL and stores that into an Entity.
async fn insert_page_title(
    In(srv): AsyncServiceInput<PageTitleRequest>
) -> Result<(), ()> {
    let response = trpl::get(&srv.request.url).await;
    let response_text = response.text().await;
    let title = trpl::Html::parse(&response_text)
        .select_first("title")
        .map(|title| title.inner_html())
        .ok_or(())?;

    let insert_into = srv.request.insert_into;
    // Use the async channel to insert a PageTitle component into the entity
    // specified by the request, then await confirmation that the command is finished.
    srv.channel.command(
        move |commands| {
            commands.entity(insert_into).insert(PageTitle(title));
        }
    )
        .await
        .take()
        .available()
        .ok_or(())
}
// ANCHOR_END: insert_page_title

// ANCHOR: fetch_page_title
#[derive(Clone, Component, Deref)]
struct Url(String);

use std::future::Future;
fn fetch_page_title(
    In(srv): AsyncServiceInput<Entity>,
    url: Query<&Url>,
) -> impl Future<Output = Result<String, ()>> + use<> {
    // Use a query to get the Url component of this entity
    let url = url.get(srv.request).cloned();

    async move {
        // Make sure the query for the Url component was successful
        let url = url.map_err(|_| ())?.0;

        // Fetch the page title of the website stored in the Url component of
        // the requested entity.
        let response = trpl::get(&url).await;
        let response_text = response.text().await;
        trpl::Html::parse(&response_text)
            .select_first("title")
            .map(|title| title.inner_html())
            .ok_or(())
    }
}
// ANCHOR_END: fetch_page_title

// ANCHOR: hello_continuous_service
fn hello_continuous_service(
    In(srv): ContinuousServiceInput<String, String>,
    mut query: ContinuousQuery<String, String>,
) {
    let Some(mut orders) = query.get_mut(&srv.key) else {
        // The service provider has despawned, so we can no longer do anything
        return;
    };

    orders.for_each(|order| {
        let name = order.request();
        let response = format!("Hello, {name}!");
        order.respond(response);
    });
}
// ANCHOR_END: hello_continuous_service

fn help_service_infer_type<Request, Response, Streams>(_service: Service<Request, Response, Streams>) {
    // Do nothing
}

#[derive(Serialize, Deserialize)]
struct SchemaV1 {

}

impl SchemaV1 {
    fn upgrade_to_schema_v2(self) -> SchemaV2 {
        SchemaV2 { }
    }
}

#[derive(Serialize, Deserialize)]
struct SchemaV2 {

}

struct Item {}
struct Workcell {}

#[derive(Clone)]
struct Location {}
struct Vehicle {}


struct WorkcellTask {
    workcell: Workcell,
    item: Item,
}

struct MobileRobotTask {
    vehicle: Vehicle,
    location: Location,
}

struct PickupTask {
    item: Item,
    workcell: Workcell,
    location: Location,
    vehicle: Vehicle,
}

struct Pickup {
    item: Item,
    location: Location,
}

enum NavigationError {
    MissingGraph,
}

#[derive(Clone, Resource)]
struct NavigationGraph;

// ANCHOR: async_streams_example
// ANCHOR: navigation_streams
#[derive(StreamPack)]
struct NavigationStreams {
    log: String,
    location: Vec2,
    error: NavigationError,
}
// ANCHOR_END: navigation_streams

#[derive(Clone)]
struct NavigationRequest {
    destination: Vec2,
    robot_position_key: BufferKey<Vec2>,
}

fn navigate(
    In(input): AsyncServiceInput<NavigationRequest, NavigationStreams>,
    nav_graph: Res<NavigationGraph>,
) -> impl Future<Output = Result<(), NavigationError>> + use<> {
    // Clone the nevigation graph resource so we can move the clone into the
    // async block.
    let nav_graph = (*nav_graph).clone();

    // Create a callback for fetching the latest position
    let get_position = |
        In(key): In<BufferKey<Vec2>>,
        access: BufferAccess<Vec2>,
    | {
        access.get_newest(&key).cloned()
    };
    let get_position = get_position.into_blocking_callback();

    // Unpack the input into simpler variables
    let NavigationRequest { destination, robot_position_key } = input.request;
    let location_stream = input.streams.location;

    // Begin an async block that will run in the AsyncComputeTaskPool
    async move {
        loop {
            // Fetch the latest position using the async channel
            let position = input.channel.query(
                robot_position_key.clone(),
                get_position.clone()
            )
                .await
                .take()
                .available()
                .flatten();

            let Some(position) = position else {
                // Position has not been reported yet, so just try again later.
                continue;
            };

            // Send the current position out over an async stream
            location_stream.send(position);

            // TODO: Command the robot to proceed towards its destination
            // TODO: Break the loop when the robot arrives at its destination
        }

        Ok(())
    }
}
// ANCHOR_END: async_streams_example

// ANCHOR: continuous_streams_example
fn continuous_navigate(
    In(srv): ContinuousServiceInput<NavigationRequest, Result<(), NavigationError>, NavigationStreams>,
    mut continuous: ContinuousQuery<NavigationRequest, Result<(), NavigationError>, NavigationStreams>,
    position_access: BufferAccess<Vec2>,
    nav_graph: Res<NavigationGraph>,
) {
    let Some(mut orders) = continuous.get_mut(&srv.key) else {
        // The service provider has despawned, so we can no longer do anything
        return;
    };

    orders.for_each(|order| {
        let NavigationRequest { destination, robot_position_key } = order.request().clone();
        let Some(position) = position_access.get_newest(&robot_position_key) else {
            // Position is not available yet
            return;
        };

        order.streams().location.send(*position);

        // TODO: Command the robot to proceed towards its destination
        // TODO: Use order.respond(Ok(())) when the robot arrives at its destination
    });
}
// ANCHOR_END: continuous_streams_example

struct Apple {}

struct AppleSlice {}

fn buffer_access_example() {
// ANCHOR: buffer_access_example
use crossflow::{prelude::*, testing::*};

/// Use mutable access (BufferAccessMut) to push values into a buffer. The
/// values are guaranteed to be present in the buffer before this service
/// finishes running. Therefore any service that accesses the buffer after this
/// service finishes is guaranteed to find the values present inside.
fn push_values(
    In(input): In<(Vec<i32>, BufferKey<i32>)>,
    mut access: BufferAccessMut<i32>,
) {
    let Ok(mut access) = access.get_mut(&input.1) else {
        return;
    };
    for value in input.0 {
        access.push(value);
    }
}

/// Use read-only access (BufferAccess) to look through the values in the
/// buffer. We pick out the largest value and clone it to pass it along. This
/// read-only access cannot pull or modify the data inside the buffer in any
/// way, it can only view and clone (if the data is cloneable) from the buffer.
fn get_largest_value(
    In(input): In<((), BufferKey<i32>)>,
    access: BufferAccess<i32>,
) -> Option<i32> {
    let access = access.get(&input.1).ok()?;
    access.iter().max().cloned()
}

let mut context = TestingContext::minimal_plugins();

let workflow = context.spawn_io_workflow(|scope, builder| {
    let buffer = builder.create_buffer(BufferSettings::keep_all());
    builder
        .chain(scope.start)
        .with_access(buffer)
        .then(push_values.into_blocking_callback())
        .with_access(buffer)
        .then(get_largest_value.into_blocking_callback())
        .connect(scope.terminate);
});

let mut promise = context.command(|commands| {
    commands.request(vec![-3, 2, 10], workflow).take_response()
});

context.run_with_conditions(&mut promise, Duration::from_secs(1));

let r = promise.take().available().unwrap().unwrap();
assert_eq!(r, 10);
// ANCHOR_END: buffer_access_example
}



struct Order {}

#[derive(Resource)]
struct WorkingHours {
    open: SystemTime,
    close: SystemTime,
}

// ANCHOR: gate_example
fn manage_opening_time(
    In(input): In<(SystemTime, BufferKey<Order>)>,
    mut gate: BufferGateAccessMut,
    hours: Res<WorkingHours>,
) {
    let Ok(mut gate) = gate.get_mut(input.1) else {
        return;
    };

    let time = input.0;
    if time < hours.open || hours.close < time {
        gate.close_gate();
    } else {
        gate.open_gate();
    }
}
// ANCHOR_END: gate_example

// ANCHOR: fibonacci_example
fn fibonacci_example(
    In(input): BlockingServiceInput<u32, StreamOf<u32>>
) {
    let order = input.request;
    let stream = input.streams;

    let mut current = 0;
    let mut next = 1;
    for _ in 0..order {
        stream.send(current);

        let sum = current + next;
        current = next;
        next = sum;
    }
}
// ANCHOR_END: fibonacci_example

// ANCHOR: fibonacci_string_example
fn fibonacci_string_example(
    In(input): BlockingServiceInput<u32, (StreamOf<u32>, StreamOf<String>)>,
) {
    let order = input.request;
    let u32_stream = input.streams.0;
    let string_stream = input.streams.1;

    let mut current = 0;
    let mut next = 1;
    for _ in 0..order {
        u32_stream.send(current);
        string_stream.send(format!("{current}"));

        let sum = current + next;
        current = next;
        next = sum;
    }
}
// ANCHOR_END: fibonacci_string_example

// ANCHOR: fibonacci_stream_pack_example
#[derive(StreamPack)]
struct FibonacciStreams {
    integers: u32,
    strings: String,
}

fn fibonacci_stream_pack_example(
    In(input): BlockingServiceInput<u32, FibonacciStreams>,
) {
    let order = input.request;
    let streams = input.streams;

    let mut current = 0;
    let mut next = 1;
    for _ in 0..order {
        streams.integers.send(current);
        streams.strings.send(format!("{current}"));

        let sum = current + next;
        current = next;
        next = sum;
    }
}
// ANCHOR_END: fibonacci_stream_pack_example

#[allow(unused)]
fn delivery_instructions_demo(commands: &mut Commands) {
async fn my_service(In(input): AsyncServiceInput<String>) {
    // Create a future that will never finish
    let never = pending::<()>();
    // Wait on the never-ending future until a timeout finishes.
    // This creates an artifical delay for the async service.
    let _ = timeout(Duration::from_secs(2), never).await;

    println!("{}", input.request);
}

// ANCHOR: always_serial_example
let service = commands.spawn_service(
    my_service
    .serial()
);
// ANCHOR_END: always_serial_example

// ANCHOR: delivery_label
#[derive(Debug, Clone, PartialEq, Eq, Hash, DeliveryLabel)]
struct MyDeliveryLabel {
    set: String,
}
// ANCHOR_END: delivery_label

// ANCHOR: set_instructions
// Spawn the service as normal
let service = commands.spawn_service(my_service);

// Create delivery instructions
let instructions = DeliveryInstructions::new(
    MyDeliveryLabel {
        set: String::from("my_set")
    }
);

// Add the instructions while requesting
let promise = commands.request(
    String::from("hello"),
    service.instruct(instructions),
);
// ANCHOR_END: set_instructions

// ANCHOR: preempt_example
// Create a label
let label = MyDeliveryLabel {
    set: String::from("my_set")
};

// Make instructions that include preempting
let preempt_instructions = DeliveryInstructions::new(label).preempt();

let promise = commands.request(
    String::from("hello"),
    service.instruct(preempt_instructions)
);
// ANCHOR_END: preempt_example

let label = MyDeliveryLabel {
    set: String::from("my_set")
};

// ANCHOR: ensure_example
let instructions = DeliveryInstructions::new(label)
    .preempt()
    .ensure();

let promise = commands.request(
    String::from("hello"),
    service.instruct(instructions)
);
// ANCHOR_END: ensure_example
}

#[allow(unused)]
fn callback_demo(commands: &mut Commands) {
// ANCHOR: callback_example
// We can access this resource from the callback
#[derive(Resource)]
struct Greeting {
    prefix: String,
}

// Make an fn that defines the callback implementation
fn perform_greeting(
    In(input): BlockingCallbackInput<String>,
    greeting: Res<Greeting>,
) -> String {
    let name = input.request;
    let prefix = &greeting.prefix;
    format!("{prefix}{name}")
}

// Convert the fn into a callback.
// This is necessary to initialize the fn as a bevy system.
let callback = perform_greeting.as_callback();

// Use the callback in a request
let promise = commands.request(String::from("Bob"), callback);
// ANCHOR_END: callback_example

// ANCHOR: async_callback_example
async fn page_title_callback(In(srv): AsyncCallbackInput<String>) -> Option<String> {
    let response = trpl::get(&srv.request).await;
    let response_text = response.text().await;
    trpl::Html::parse(&response_text)
        .select_first("title")
        .map(|title| title.inner_html())
}

let callback = page_title_callback.as_callback();
let promise = commands.request(String::from("https://example.com"), callback);
// ANCHOR_END: async_callback_example

// ANCHOR: closure_callback_example
// Make an closure that defines the callback implementation
let perform_greeting = |
    In(input): BlockingCallbackInput<String>,
    greeting: Res<Greeting>,
| {
    let name = input.request;
    let prefix = &greeting.prefix;
    format!("{prefix}{name}")
};

// Convert the fn into a callback.
// This is necessary to initialize the fn as a bevy system.
let callback = perform_greeting.as_callback();

// Use the callback in a request
let promise = commands.request(String::from("Bob"), callback);
// ANCHOR_END: closure_callback_example

// ANCHOR: async_closure_callback_example
let page_title_callback = |In(srv): AsyncCallbackInput<String>| {
    async move {
        let response = trpl::get(&srv.request).await;
        let response_text = response.text().await;
        trpl::Html::parse(&response_text)
            .select_first("title")
            .map(|title| title.inner_html())
    }
};

let callback = page_title_callback.as_callback();
let promise = commands.request(String::from("https://example.com"), callback);
// ANCHOR_END: async_closure_callback_example
}

fn agnostic_impl_demos(commands: &mut Commands) {
#[derive(Resource)]
struct Greeting {
    prefix: String,
}

// ANCHOR: agnostic_blocking_example
fn perform_greeting(
    In(name): In<String>,
    greeting: Res<Greeting>,
) -> String {
    let prefix = &greeting.prefix;
    format!("{prefix}{name}")
}

// Use as service
let greeting_service = commands.spawn_service(
    perform_greeting.into_blocking_service()
);
let promise = commands.request(String::from("Bob"), greeting_service);

// Use as callback
let greeting_callback = perform_greeting.into_blocking_callback();
let promise = commands.request(String::from("Bob"), greeting_callback);
// ANCHOR_END: agnostic_blocking_example

#[derive(Resource, Deref)]
struct Url(String);

// ANCHOR: agnostic_async_example
fn get_page_element(
    In(element): In<String>,
    url: Res<Url>,
) -> impl Future<Output = Option<String>> + use<> {
    let url = (**url).clone();
    async move {
        let content = fetch_content_from_url(url).await?;
        content.get(&element).cloned()
    }
}

let element = String::from("title");

// Use as service
let title_service = commands.spawn_service(
    get_page_element.into_async_service()
);
let promise = commands.request(element.clone(), title_service);

// Use as callback
let title_callback = get_page_element.into_async_callback();
let promise = commands.request(element, title_callback);
// ANCHOR_END: agnostic_async_example

}

async fn fetch_content_from_url(_: String) -> Option<HashMap<String, String>> {
    None
}

fn map_demo(commands: &mut Commands) {
// ANCHOR: fibonacci_map_example
fn fibonacci_map_example(input: BlockingMap<u32, StreamOf<u32>>) {
    let order = input.request;
    let stream = input.streams;

    let mut current = 0;
    let mut next = 1;
    for _ in 0..order {
        stream.send(current);

        let sum = current + next;
        current = next;
        next = sum;
    }
}

let promise = commands.request(10, fibonacci_map_example.as_map());
// ANCHOR_END: fibonacci_map_example

// ANCHOR: navigate_map_example
async fn navigate(
    input: AsyncMap<NavigationRequest, NavigationStreams>,
) -> Result<(), NavigationError> {
    // Clone the nevigation graph resource so we can move the clone into the
    // async block.
    let Some(nav_graph) = input
        .channel
        .world(|world| {
            world.resource::<NavigationGraph>().clone()
        })
        .await
        .take()
        .available()
    else {
        return Err(NavigationError::MissingGraph);
    };

    // Create a callback for fetching the latest position
    let get_position = |
        In(key): In<BufferKey<Vec2>>,
        access: BufferAccess<Vec2>,
    | {
        access.get_newest(&key).cloned()
    };
    let get_position = get_position.into_blocking_callback();

    // Unpack the input into simpler variables
    let NavigationRequest { destination, robot_position_key } = input.request;
    let location_stream = input.streams.location;

    loop {
        // Fetch the latest position using the async channel
        let position = input.channel.query(
            robot_position_key.clone(),
            get_position.clone()
        )
            .await
            .take()
            .available()
            .flatten();

        let Some(position) = position else {
            // Position has not been reported yet, so just try again later.
            continue;
        };

        // Send the current position out over an async stream
        location_stream.send(position);

        // TODO: Command the robot to proceed towards its destination
        // TODO: Break the loop when the robot arrives at its destination
    }

    Ok(())
}
// ANCHOR_END: navigate_map_example
}
