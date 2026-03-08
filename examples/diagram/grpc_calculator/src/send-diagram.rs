/*
 * Copyright (C) 2026 Open Source Robotics Foundation
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

use crossflow_service::crossflow_service_client::CrossflowServiceClient;
use crossflow_service::TriggerRequest;
use tonic::transport::Channel;

pub mod crossflow_service {
    tonic::include_proto!("crossflow_service");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "http://[::1]:50051";
    let channel = Channel::from_static(addr).connect().await?;
    let mut client = CrossflowServiceClient::new(channel);

    let diagram_json_str = r#"
    {
        "$schema": "https://raw.githubusercontent.com/open-rmf/crossflow/refs/heads/main/diagram.schema.json",
        "version": "0.1.0",
        "description": "Basic workflow that multiplies the interger input by 3.",
        "input_examples": [
            {
            "description": "Multiply 123 by 3 to get 369",
            "value": "123"
            },
            {
            "description": "Multiply 456 by 3 to get 1368",
            "value": "456"
            }
        ],
        "start": "mul3",
        "ops": {
            "mul3": {
            "type": "node",
            "builder": "mul",
            "config": 3,
            "next": { "builtin": "terminate" }
            }
        }
    }
    "#;

    let request = TriggerRequest {
        diagram: diagram_json_str.to_string(),
        request: "10".to_string(),
    };
    let response = client.trigger(request).await?;
    let response_inner = response.into_inner();
    println!("Response: {}", response_inner.result);

    Ok(())
}
