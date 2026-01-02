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

fn main() {

}

#[allow(unused)]
fn new_diagram_element_registry(mut commands: Commands) {
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

}

fn help_service_infer_type<Request, Response, Streams>(_service: Service<Request, Response, Streams>) {
    // Do nothing
}
