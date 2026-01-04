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

use crossflow::bevy_ecs::prelude::Commands;
use crossflow::prelude::*;
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use std::collections::HashMap;

fn main() {

}

#[allow(unused)]
fn examples(mut commands: Commands) {
// ANCHOR: new_diagram_element_registry
use crossflow::prelude::*;
let mut registry = DiagramElementRegistry::new();
// ANCHOR_END: new_diagram_element_registry

// ANCHOR: minimal_add_example
registry.register_node_builder(
    NodeBuilderOptions::new("add"),
    |builder, config: f32| {
        builder.create_map_block(move |request: f32| {
            request + config
        })
    }
);
// ANCHOR_END: minimal_add_example

// ANCHOR: build_workflow_example
use serde_json::json;
let diagram_json = json!(
    {
        "version": "0.1.0",
        "start": "add_5",
        "ops": {
            "add_5": {
                "type": "node",
                "builder": "add",
                "config": 5,
                "next": { "builtin": "terminate" }
            }
        }
    }
);

let diagram = Diagram::from_json(diagram_json).unwrap();
let workflow = diagram.spawn_io_workflow(&mut commands, &registry).unwrap();
// ANCHOR_END: build_workflow_example
help_service_infer_type::<(), (), ()>(workflow);

// ANCHOR: minimal_multiply_by_example
let options = NodeBuilderOptions::new("multiply_by");
// ANCHOR_END: minimal_multiply_by_example

// ANCHOR: default_display_text
let options = NodeBuilderOptions::new("multiply_by")
    .with_default_display_text("Multiply By");
// ANCHOR_END: default_display_text

// ANCHOR: node_builder_description
let description = "Multiply the input value by the configured value.";

let options = NodeBuilderOptions::new("multiply_by")
    .with_default_display_text("Multiply By")
    .with_description(description);
// ANCHOR_END: node_builder_description

// ANCHOR: custom_config
/// A custom node builder config for a greeting node
#[derive(Deserialize, JsonSchema)]
struct GreetConfig {
    /// How should the person be greeted?
    greeting: String,
    /// Should the greeting be printed out?
    print: bool,
}

registry.register_node_builder(
    NodeBuilderOptions::new("greet"),
    |builder, config: GreetConfig| {
        let GreetConfig { greeting, print } = config;
        builder.create_map_block(
            move |name: String| {
                let message = format!("{greeting}{name}");
                if print {
                    println!("{message}");
                }

                message
            }
        )
    }
);
// ANCHOR_END: custom_config
}

#[allow(unused)]
fn more_examples(mut commands: Commands) {
let mut registry = DiagramElementRegistry::new();

// ANCHOR: custom_config_with_examples
use crossflow::{prelude::*, ConfigExample};
use serde::{Serialize, Deserialize};

/// A custom node builder config for a greeting node
#[derive(Serialize, Deserialize, JsonSchema)]
struct GreetConfig {
    /// How should the person be greeted?
    greeting: String,
    /// Should the greeting be printed out?
    print: bool,
}

let examples = vec![
    ConfigExample::new(
        "Say hello and print the message",
        GreetConfig {
            greeting: String::from("Hello, "),
            print: true,
        }
    ),
    ConfigExample::new(
        "Say guten tag and do not print the message",
        GreetConfig {
            greeting: String::from("Guten tag "),
            print: false,
        }
    ),
];

registry.register_node_builder(
    NodeBuilderOptions::new("greet")
        .with_description("Turn a name into a greeting")
        .with_config_examples(examples),
    |builder, config: GreetConfig| {
        let GreetConfig { greeting, print } = config;
        builder.create_map_block(
            move |name: String| {
                let message = format!("{greeting}{name}");
                if print {
                    println!("{message}");
                }

                message
            }
        )
    }
);
// ANCHOR_END: custom_config_with_examples

// ANCHOR: division_example
use anyhow::anyhow;
registry.register_node_builder_fallible(
    NodeBuilderOptions::new("divide_by"),
    |builder, config: f64| {
        if config == 0.0 {
            return Err(anyhow!("Cannot divide by zero"));
        }

        let node = builder.create_map_block(move |request: f64| request / config);
        Ok(node)
    }
);
// ANCHOR_END: division_example

// ANCHOR: get_url_header_example
/// A request to get some information from a web page.
///
/// We implement Joined so this struct can be created by the join operation.
///
/// We implement Clone, Serialize, Deserialize, and JsonSchema so this struct
/// can support the default message operations.
#[derive(Joined, Clone, Serialize, Deserialize, JsonSchema)]
struct WebPageQuery {
    url: String,
    element: String,
}

registry
    .register_node_builder(
        NodeBuilderOptions::new("get_url_header"),
        |builder, config: ()| {
            builder.create_map_async(|query: WebPageQuery| {
                async move {
                    let page = fetch_content_from_url(query.url).await?;
                    page
                        .get(&query.element)
                        .cloned()
                        .ok_or_else(|| FetchError::ElementMissing(query.element))
                }
            })
        }
    )
    .with_join()
    .with_result();
// ANCHOR_END: get_url_header_example

// ANCHOR: opt_out_example
use tokio::sync::mpsc::UnboundedReceiver;
registry
    .opt_out()
    .no_cloning()
    .no_serializing()
    .no_deserializing()
    .register_node_builder(
        NodeBuilderOptions::new("stream_out"),
        |builder, config: ()| {
            builder.create_map(|input: AsyncMap<UnboundedReceiver<f32>, StreamOf<f32>>| {
                async move {
                    let mut receiver = input.request;
                    let stream = input.streams;

                    while let Some(msg) = receiver.recv().await {
                        stream.send(msg);
                    }
                }
            })
        }
    )
    .with_common_response();
// ANCHOR_END: opt_out_example

// ANCHOR: state_update_example
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
struct State {
    position: [f32; 2],
    battery_level: f32,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
enum Error {
    LostConnection
}

registry
    .register_node_builder(
        NodeBuilderOptions::new("navigate_to"),
        |builder, config: ()| {
            builder.create_map(
                |input: AsyncMap<[f32; 2], StreamOf<Result<State, Error>>>| {
                    async move {
                        let destination = input.request;
                        let stream = input.streams;

                        let mut update_receiver = navigate_to(destination);
                        while let Some(update) = update_receiver.recv().await {
                            stream.send(update);
                        }
                    }
                }
            )
        }
    );

// Explicitly register the message of the stream so we can add .with_result to it.
registry
    .register_message::<Result<State, Error>>()
    .with_result();
// ANCHOR_END: state_update_example

fn navigate_to(_: [f32; 2]) -> UnboundedReceiver<Result<State, Error>> {
    tokio::sync::mpsc::unbounded_channel().1
}

#[derive(Clone, Serialize, Deserialize, Debug, JsonSchema)]
enum MoveRobotError {

}

#[derive(Clone, Serialize, Deserialize, Debug, JsonSchema)]
enum LocalizationError {

}

#[derive(Clone, Serialize, Deserialize, Debug, JsonSchema)]
enum MoveElevatorError {

}

let enter_elevator = |_: &String, _: &String| {
    (|_: BlockingMap<()>| { Ok::<(), MoveRobotError>(()) }).as_map()
};
let move_elevator = |_: &String, _: &String| {
    (|_: BlockingMap<()>| { Ok::<(), MoveElevatorError>(()) }).as_map()
};
let localize_robot = |_: &String, _: &String| {
    (|_: BlockingMap<()>| { Ok::<(), LocalizationError>(()) }).as_map()
};

let exit_elevator = |_: &String, _: &String| {
    (|_: BlockingMap<()>| { Ok::<(), MoveRobotError>(()) }).as_map()
};


// ANCHOR: elevator_example
use crossflow::{prelude::*, SectionBuilderOptions};

/// The kind of section produced by the "use_elevator" section builder.
#[derive(Section)]
struct UseElevatorSection {
    /// Begin using the elevator by having the robot enter it.
    begin: InputSlot<()>,
    /// Signal that the robot failed to enter the elevator.
    enter_elevator_failure: Output<MoveRobotError>,
    /// Signal that the elevator failed to reach its destination.
    move_elevator_error: Output<MoveElevatorError>,
    /// Retry moving the elevator. Trigger this when a move_elevator_error is
    /// resolved.
    retry_elevator_move: InputSlot<()>,
    /// Signal that localization failed.
    localization_error: Output<LocalizationError>,
    /// Retry localizing the robot at the new floor. Trigger this when a
    /// localization_error is resolved.
    retry_localization: InputSlot<()>,
    /// Signal that the robot failed to exit the elevator.
    exit_elevator_failure: Output<MoveRobotError>,
    /// Retry exiting the elevator. Trigger this when an exit_elevator_failure
    /// is resolved.
    retry_elevator_exit: InputSlot<()>,
    /// The robot has successfully exited the elevator at the destination floor.
    success: Output<()>,
}

/// The config data structure for the "use_elevator" section builder.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
struct UseElevatorConfig {
    elevator_id: String,
    robot_id: String,
    to_floor: String,
}

registry.register_section_builder(
    SectionBuilderOptions::new("use_elevator")
        .with_default_display_text("Use Elevator")
        .with_description("Have a robot use an elevator"),
    move |builder: &mut Builder, config: UseElevatorConfig| {
        let UseElevatorConfig { elevator_id, robot_id, to_floor } = config;

        // Create a node for entering the elevator
        let enter_elevator = builder.create_node(enter_elevator(&robot_id, &elevator_id));

        // Create a fork-result that splits based on whether the robot
        // successfully entered the elevator
        let (enter_elevator_result, enter_elevator_fork) = builder.create_fork_result();
        builder.connect(enter_elevator.output, enter_elevator_result);

        // Create a node to move the elevator if the robot successfully entered
        let move_elevator = builder.create_node(move_elevator(&elevator_id, &to_floor));
        builder.connect(enter_elevator_fork.ok, move_elevator.input);

        // Create a fork-result that splits based on whether the elevator
        // successfully arrived at its destination
        let (move_elevator_result, move_elevator_fork) = builder.create_fork_result();
        builder.connect(move_elevator.output, move_elevator_result);

        // Create a node to localize the robot once the elevator arrives at the
        // correct floor
        let localize_robot = builder.create_node(localize_robot(&robot_id, &to_floor));
        builder.connect(move_elevator_fork.ok, localize_robot.input);

        // Create a fork-result that splits based on whether the robot
        // successfully localized
        let (localize_result, localize_fork) = builder.create_fork_result();
        builder.connect(localize_robot.output, localize_result);

        // Create a node to exit the elevator after the robot has localized
        let exit_elevator = builder.create_node(exit_elevator(&robot_id, &elevator_id));
        builder.connect(localize_fork.ok, exit_elevator.input);

        // Create a fork-result that splits based on whether the robot
        // successfully exited the lift
        let (exit_elevator_result, exit_elevator_fork) = builder.create_fork_result();
        builder.connect(exit_elevator.output, exit_elevator_result);

        UseElevatorSection {
            begin: enter_elevator.input,
            enter_elevator_failure: enter_elevator_fork.err,
            move_elevator_error: move_elevator_fork.err,
            retry_elevator_move: move_elevator.input,
            localization_error: localize_fork.err,
            retry_localization: localize_robot.input,
            exit_elevator_failure: exit_elevator_fork.err,
            retry_elevator_exit: exit_elevator.input,
            success: exit_elevator_fork.ok,
        }
    }
);
// ANCHOR_END: elevator_example

struct Robot {}
struct Door {}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
enum UseDoorError {}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
struct DoorState {}

// ANCHOR: section_operation_support
#[derive(Section)]
struct UseDoorSection {
    /// Robot and Door cannot be cloned, serialized, or deserialized, so we
    /// disable those operations. But the overall message can still be unzipped,
    /// so we enable the minimal version of unzip, which will register both Robot
    /// and Door, but without the common operations (clone, serialize, and deserialize).
    #[message(no_clone, no_serialize, no_deserialize, unzip_minimal)]
    begin: InputSlot<(Robot, Door)>,

    /// UseDoorError is cloneable, serializable, and deserializable, so we don't
    /// need to disable anything here. The overall message type is a result, so
    /// we can mark this as a result to register the fork-result operation.
    #[message(result)]
    outcome: Output<Result<(), UseDoorError>>,

    /// We can also expose buffers inside the section.
    door_state: Buffer<DoorState>,
}
// ANCHOR_END: section_operation_support

}

fn help_service_infer_type<Request, Response, Streams>(_service: Service<Request, Response, Streams>) {
    // Do nothing
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
enum FetchError {
    MissingUrl,
    ElementMissing(String),
}

async fn fetch_content_from_url(_: String) -> Result<HashMap<String, String>, FetchError> {
    Err(FetchError::MissingUrl)
}
