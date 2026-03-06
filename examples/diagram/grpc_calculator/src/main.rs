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

use crossflow_diagram_editor::basic_executor::{BasicExecutorSetup, DiagramElementRegistry};
use prost::Message;
use std::fs::File;
use std::io::Read;
use tonic::{Request, Response, Status, transport::Server};

use crossflow::{
    CrossflowExecutorApp, Diagram, DiagramError, Outcome, RequestExt, RunCommandsOnWorldExt,
};
use std::str::FromStr;

#[derive(Clone, PartialEq, ::prost::Message)]
struct RuntimeConfigWrapper {
    #[prost(int32, tag = "1")]
    pub port: i32,
    #[prost(message, optional, tag = "6")]
    pub any: Option<AnyWrapper>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct AnyWrapper {
    #[prost(bytes = "vec", tag = "2")]
    pub value: Vec<u8>,
}

pub mod crossflow_service {
    tonic::include_proto!("crossflow_service");
}

use crossflow_service::crossflow_trigger_service_server::{
    CrossflowTriggerService, CrossflowTriggerServiceServer,
};
use crossflow_service::{CrossflowServiceConfig, TriggerRequest, TriggerResponse};

#[derive(Default)]
pub struct TriggerService {}

#[tonic::async_trait]
impl CrossflowTriggerService for TriggerService {
    async fn trigger(
        &self,
        request: Request<TriggerRequest>,
    ) -> Result<Response<TriggerResponse>, Status> {
        let req = request.into_inner();
        let diagram_path = req.diagram_path;
        let request_json = req.request;

        println!(
            "Received request: path: {}, request: {}",
            diagram_path, request_json
        );

        let result = tokio::task::spawn_blocking(move || {
            let mut registry = DiagramElementRegistry::new();
            calculator_ops_catalog::register(&mut registry);

            let BasicExecutorSetup { mut app, registry } = BasicExecutorSetup::minimal(registry);
            app.add_plugins(CrossflowExecutorApp::default());
            let file =
                File::open(&diagram_path).map_err(|e| format!("Failed to open diagram: {}", e))?;
            let diagram =
                Diagram::from_reader(file).map_err(|e| format!("Failed to read diagram: {}", e))?;

            let request_val = serde_json::Value::from_str(&request_json)
                .map_err(|e| format!("Invalid JSON: {}", e))?;

            let mut outcome = app
                .world_mut()
                .command(|cmds| -> Result<Outcome<serde_json::Value>, DiagramError> {
                    let workflow = diagram.spawn_io_workflow(cmds, &registry)?;
                    Ok(cmds.request(request_val, workflow).outcome())
                })
                .map_err(|e| format!("Failed to spawn workflow: {}", e))?;

            while outcome.is_pending() {
                app.update();
            }

            match outcome.try_recv().unwrap() {
                Ok(response) => Ok(response.to_string()),
                Err(err) => Err(format!("Execution error: {}", err)),
            }
        })
        .await
        .map_err(|e| Status::internal(format!("Task panicked: {}", e)))?
        .map_err(Status::internal)?;

        Ok(Response::new(TriggerResponse { result }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut port = 50051; // Default port

    let result = (|| -> Result<CrossflowServiceConfig, Box<dyn std::error::Error>> {
        let mut file = File::open("/etc/intrinsic/runtime_config.pb")?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let wrapper = RuntimeConfigWrapper::decode(&buffer[..])?;
        if wrapper.port != 0 {
            port = wrapper.port as u16;
        }

        let config_bytes = wrapper.any.ok_or("config not found in wrapper")?.value;
        Ok(CrossflowServiceConfig::decode(&config_bytes[..])?)
    })();

    match result {
        Ok(config) => {
            println!("Successfully parsed config: {:?}", config);
        }
        Err(e) => println!("Failed to parse config: {}. Proceeding with defaults.", e),
    }

    let addr = format!("[::]:{}", port).parse()?;
    let service = TriggerService::default();

    println!("TriggerService listening on {}", addr);

    Server::builder()
        .add_service(CrossflowTriggerServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Request;

    #[tokio::test]
    async fn test_trigger_service() {
        let service = TriggerService::default();
        let diagram_path = "diagrams/multiply_by_3.json";

        let req = TriggerRequest {
            diagram_path: diagram_path.to_string(),
            request: "10".to_string(),
        };

        let response = service.trigger(Request::new(req)).await;
        assert!(response.is_ok());
        let response_inner = response.unwrap().into_inner();

        println!("Response: {}", response_inner.result);
        assert_eq!(response_inner.result, "30.0");
    }
}
