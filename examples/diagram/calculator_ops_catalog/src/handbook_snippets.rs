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
