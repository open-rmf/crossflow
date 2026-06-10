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

fn main() -> Result<(), Box<dyn Error>> {
    // Create a new regsitry with the default message types pre-registered.
    let mut registry = DiagramElementRegistry::new();

    // Register calculator-inspired node builders from the calculator_ops_catalog library.
    calculator_ops_catalog::register(&mut registry);

    // Enable Python scripting
    let py_event_loop = registry.enable_python().unwrap();
    py_event_loop.spawn_thread_and_run();

    // Run the basic executor
    let result = basic_executor::run(registry);

    // Shut down the python event loop
    py_event_loop.stop().unwrap();

    // Return the result of the executor's run
    result
}
// ANCHOR_END: calculator_example
