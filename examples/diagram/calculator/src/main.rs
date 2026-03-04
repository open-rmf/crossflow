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

// ANCHOR: calculator_example
use crossflow_diagram_editor::basic_executor::{self, DiagramElementRegistry, Error};
use prost::Message;
use std::fs::File;
use std::io::Read;

#[derive(Clone, PartialEq, ::prost::Message)]
struct RuntimeConfigWrapper {
    #[prost(message, optional, tag = "6")]
    pub any: Option<AnyWrapper>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct AnyWrapper {
    #[prost(bytes = "vec", tag = "2")]
    pub value: Vec<u8>,
}

// define new module for the generated code
pub mod crossflow_service {
    // include generated code
    include!(concat!(env!("OUT_DIR"), "/crossflow_service.rs"));
}

// import the struct from the new module
use crossflow_service::CrossflowServiceConfig;

fn main() -> Result<(), Box<dyn Error>> {
// create an instance of the config and print it out as an example
    let config = CrossflowServiceConfig {
        skill_registry_address: String::from("127.0.0.1:50051"),
        solution_service_address: String::from("127.0.0.1:50052"),
        diagram_path: String::from("examples/multiply_by_3.json"),
        request: String::from("10"),
    };
    println!("Successfully created config: {:?}", config);

    let mut diagram_path: String = String::from("examples/multiply_by_3.json");
    let mut request: String = String::from("10");

    // parse a .pb file as a CrossflowServiceConfig, and print it out
    let result = (|| -> Result<CrossflowServiceConfig, Box<dyn std::error::Error>> {
        let mut file = File::open("/etc/intrinsic/runtime_config.pb")?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let wrapper = RuntimeConfigWrapper::decode(&buffer[..])?;
        let config_bytes = wrapper.any.ok_or("config not found in wrapper")?.value;
        Ok(CrossflowServiceConfig::decode(&config_bytes[..])?)
    })();
    match result {
        Ok(config) => {
            println!("Successfully parsed config: {:?}", config);
            diagram_path = config.diagram_path;
            request = config.request;
        },
        Err(e) => println!("Failed to parse config: {}", e),
    }

    // Create a new regsitry with the default message types pre-registered.
    let mut registry = DiagramElementRegistry::new();

    // Register calculator-inspired node builders from the calculator_ops_catalog library.
    calculator_ops_catalog::register(&mut registry);

    // // Run the basic executor
    // basic_executor::run(registry)

    // Create args
    let args = basic_executor::Args {
        command: basic_executor::Commands::Run(basic_executor::RunArgs {
            diagram: diagram_path,
            request: request
        })};

    // Run with args
    basic_executor::run_with_args(args, registry)

}
// ANCHOR_END: calculator_example
