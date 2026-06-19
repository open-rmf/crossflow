use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{self, Response},
};
#[cfg(feature = "router")]
use axum::{Router, routing::post};
#[cfg(feature = "debug")]
use axum::{
    extract::ws,
    routing::{self},
};
use bevy_ecs::{prelude::Entity, schedule::IntoScheduleConfigs};
use crossflow::{
    Diagram, DiagramElementRegistry, DiagramError, DiagramErrorCode, DiagramOperation,
    InferenceBoundaryConditions, MetadataAccess, Outcome, PortRef, RequestExt, TracedEvent, trace,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::error::TryRecvError;
use tracing::error;
#[cfg(feature = "debug")]
use tracing::warn;

#[cfg(feature = "debug")]
use super::websocket::{WebsocketSinkExt, WebsocketStreamExt};
use crate::api::error_responses::WorkflowCancelledResponse;

#[cfg(feature = "debug")]
type BroadcastRecvError = tokio::sync::broadcast::error::RecvError;

type WorkflowResponseResult =
    Result<(Outcome<serde_json::Value>, Entity), Box<dyn Error + Send + Sync>>;
type WorkflowResponseSender = tokio::sync::oneshot::Sender<WorkflowResponseResult>;

type WorkflowFeedback = TracedEvent;

#[derive(bevy_ecs::component::Component)]
struct FeedbackSender(tokio::sync::broadcast::Sender<WorkflowFeedback>);

pub struct Context {
    diagram: Diagram,
    request: serde_json::Value,
    registry: Arc<Mutex<DiagramElementRegistry>>,
    response_tx: WorkflowResponseSender,
    feedback_tx: Option<FeedbackSender>,
}

#[derive(Clone)]
pub struct ExecutorState {
    pub registry: Arc<Mutex<DiagramElementRegistry>>,
    pub send_chan: tokio::sync::mpsc::Sender<Context>,
    pub despawn_chan: tokio::sync::mpsc::Sender<Entity>,
    pub response_timeout: Duration,
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[cfg_attr(test, derive(serde::Serialize))]
#[derive(Deserialize)]
pub struct PostRunRequest {
    pub diagram: Diagram,
    pub request: serde_json::Value,
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityRequest {
    pub candidates: Vec<CompatibilityCandidate>,
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityCandidate {
    pub id: String,
    pub diagram: Diagram,
    #[serde(default)]
    pub focus_ports: Vec<PortRef>,
    #[serde(default)]
    pub source_port: Option<PortRef>,
    #[serde(default)]
    pub target_port: Option<PortRef>,
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityResponse {
    pub results: Vec<CompatibilityResult>,
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityResult {
    pub id: String,
    pub status: CompatibilityStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub provisional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityStatus {
    Compatible,
    Incompatible,
    Unknown,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Sends a request to the executor system and wait for the response.
pub async fn post_run(
    state: State<ExecutorState>,
    Json(body): Json<PostRunRequest>,
) -> response::Result<Json<serde_json::Value>> {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    if let Err(err) = state
        .send_chan
        .send(Context {
            registry: state.registry.clone(),
            diagram: body.diagram,
            request: body.request,
            response_tx,
            feedback_tx: None,
        })
        .await
    {
        error!("{}", err);
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into());
    }

    let workflow_response = match response_rx.await {
        Ok(response) => response,
        Err(err) => {
            error!("{}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into());
        }
    };

    let response = (match workflow_response {
        Ok((outcome, workflow)) => {
            let result = outcome.await;
            if let Err(err) = state.despawn_chan.send(workflow).await {
                error!("Failed to request workflow despawn: {err}");
            }

            match result {
                Ok(response) => Ok(response),
                Err(err) => Err(WorkflowCancelledResponse(&err).into()),
            }
        }
        Err(err) => Err(Response::builder()
            .status(StatusCode::UNPROCESSABLE_ENTITY)
            .body(err.to_string())
            .map_or(StatusCode::INTERNAL_SERVER_ERROR.into(), |resp| resp.into())),
    } as response::Result<serde_json::Value>)?;

    Ok(Json(response))
}

pub async fn post_compatibility(
    state: State<ExecutorState>,
    Json(body): Json<CompatibilityRequest>,
) -> response::Result<Json<CompatibilityResponse>> {
    let registry = state.registry.lock().map_err(|err| {
        error!("failed to lock registry for compatibility check: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let results = body
        .candidates
        .into_iter()
        .map(|candidate| check_compatibility_candidate(&registry, candidate))
        .collect();

    Ok(Json(CompatibilityResponse { results }))
}

fn check_compatibility_candidate(
    registry: &DiagramElementRegistry,
    candidate: CompatibilityCandidate,
) -> CompatibilityResult {
    let stream_names = root_stream_names(&candidate.diagram);
    let boundary = match InferenceBoundaryConditions::json_messages(registry, stream_names) {
        Ok(boundary) => boundary,
        Err(err) => {
            return CompatibilityResult {
                id: candidate.id,
                status: CompatibilityStatus::Unknown,
                reason: err.to_string(),
                provisional: false,
                source_type: None,
                target_type: None,
            };
        }
    };

    let mut focus_ports = candidate.focus_ports.clone();
    if let Some(source_port) = &candidate.source_port {
        focus_ports.push(source_port.clone());
    }
    if let Some(target_port) = &candidate.target_port {
        focus_ports.push(target_port.clone());
    }
    focus_ports.sort();
    focus_ports.dedup();

    if focus_ports.is_empty() {
        return CompatibilityResult {
            id: candidate.id,
            status: CompatibilityStatus::Unknown,
            reason: "no ports were provided for compatibility checking".to_string(),
            provisional: false,
            source_type: None,
            target_type: None,
        };
    }

    let inference =
        match candidate
            .diagram
            .infer_message_types_for_ports(registry, boundary, focus_ports)
        {
            Ok(inference) => inference,
            Err(err) => {
                if is_missing_context_error(&err) {
                    return CompatibilityResult {
                        id: candidate.id,
                        status: CompatibilityStatus::Compatible,
                        reason: format!("connection needs more type context: {err}"),
                        provisional: true,
                        source_type: None,
                        target_type: None,
                    };
                }

                return CompatibilityResult {
                    id: candidate.id,
                    status: compatibility_error_status(&err),
                    reason: err.to_string(),
                    provisional: false,
                    source_type: None,
                    target_type: None,
                };
            }
        };

    let source_type = candidate
        .source_port
        .as_ref()
        .and_then(|port| inference.get(port).copied());
    let target_type = candidate
        .target_port
        .as_ref()
        .and_then(|port| inference.get(port).copied());

    let source_type_name = source_type.and_then(|message_type| {
        registry
            .message_type_name(message_type)
            .ok()
            .map(ToOwned::to_owned)
    });
    let target_type_name = target_type.and_then(|message_type| {
        registry
            .message_type_name(message_type)
            .ok()
            .map(ToOwned::to_owned)
    });

    let (Some(source_type), Some(target_type)) = (source_type, target_type) else {
        let provisional = candidate.source_port.is_some() || candidate.target_port.is_some();
        return CompatibilityResult {
            id: candidate.id,
            status: CompatibilityStatus::Compatible,
            reason: if provisional {
                "focused ports can be inferred, but the connection needs more peer type context"
                    .to_string()
            } else {
                "focused ports can be inferred".to_string()
            },
            provisional,
            source_type: source_type_name,
            target_type: target_type_name,
        };
    };

    match can_connect_message_types(registry, source_type, target_type) {
        Ok(Some(reason)) => CompatibilityResult {
            id: candidate.id,
            status: CompatibilityStatus::Compatible,
            reason,
            provisional: false,
            source_type: source_type_name,
            target_type: target_type_name,
        },
        Ok(None) => CompatibilityResult {
            id: candidate.id,
            status: CompatibilityStatus::Incompatible,
            reason: format!(
                "{} cannot be delivered to {}",
                source_type_name
                    .as_deref()
                    .unwrap_or("[unknown source type]"),
                target_type_name
                    .as_deref()
                    .unwrap_or("[unknown target type]"),
            ),
            provisional: false,
            source_type: source_type_name,
            target_type: target_type_name,
        },
        Err(err) => CompatibilityResult {
            id: candidate.id,
            status: CompatibilityStatus::Unknown,
            reason: err.to_string(),
            provisional: false,
            source_type: source_type_name,
            target_type: target_type_name,
        },
    }
}

fn is_missing_context_error(error: &DiagramError) -> bool {
    matches!(
        &error.code,
        DiagramErrorCode::CannotInferType(_)
            | DiagramErrorCode::NoConnection(_)
            | DiagramErrorCode::UnknownPort(_)
    )
}

fn compatibility_error_status(error: &DiagramError) -> CompatibilityStatus {
    if is_missing_context_error(error) {
        CompatibilityStatus::Unknown
    } else {
        CompatibilityStatus::Incompatible
    }
}

fn can_connect_message_types(
    registry: &DiagramElementRegistry,
    source_type: usize,
    target_type: usize,
) -> Result<Option<String>, DiagramErrorCode> {
    if source_type == target_type {
        return Ok(Some("message types match exactly".to_string()));
    }

    if registry.can_convert(source_type, target_type)? {
        return Ok(Some(
            "registered message conversion is available".to_string(),
        ));
    }

    if registry
        .json_message_index()
        .is_ok_and(|json_type| target_type == json_type)
        && registry.can_seralize(source_type)?
    {
        return Ok(Some(
            "source can be implicitly serialized to JSON".to_string(),
        ));
    }

    if registry
        .json_message_index()
        .is_ok_and(|json_type| source_type == json_type)
        && registry.can_deserialize(target_type)?
    {
        return Ok(Some(
            "JSON can be implicitly deserialized for the target".to_string(),
        ));
    }

    if registry
        .script_message_index()
        .is_ok_and(|script_type| target_type == script_type)
        && registry.into_script_message(source_type)?
    {
        return Ok(Some(
            "source can be implicitly converted to ScriptMessage".to_string(),
        ));
    }

    if registry
        .script_message_index()
        .is_ok_and(|script_type| source_type == script_type)
        && registry.from_script_message(target_type)?
    {
        return Ok(Some(
            "ScriptMessage can be implicitly converted for the target".to_string(),
        ));
    }

    Ok(None)
}

fn root_stream_names(diagram: &Diagram) -> Vec<String> {
    diagram
        .ops
        .values()
        .filter_map(|op| match op.as_ref() {
            DiagramOperation::StreamOut(stream_out) => Some(stream_out.name().to_string()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crossflow::{
        Blocking, BufferAccess, BufferKey, Builder, IntoCallback, JsonMessage, NextOperation, Node,
        NodeBuilderOptions, output_ref,
    };
    use serde_json::json;

    fn test_registry() -> DiagramElementRegistry {
        let mut registry = DiagramElementRegistry::new();
        registry
            .register_message::<i64>()
            .with_mapping_into::<f64>(|value| value as f64);
        registry.register_node_builder(
            NodeBuilderOptions::new("json_to_i64"),
            |builder: &mut Builder, _config: ()| {
                builder.create_map_block(|request: JsonMessage| request.as_i64().unwrap_or(0))
            },
        );
        registry.register_node_builder(
            NodeBuilderOptions::new("json_identity"),
            |builder: &mut Builder, _config: ()| {
                builder.create_map_block(|request: JsonMessage| request)
            },
        );
        registry.register_node_builder(
            NodeBuilderOptions::new("i64_to_json"),
            |builder: &mut Builder, _config: ()| {
                builder.create_map_block(|request: i64| JsonMessage::from(request))
            },
        );
        registry.register_node_builder(
            NodeBuilderOptions::new("f64_to_json"),
            |builder: &mut Builder, _config: ()| {
                builder.create_map_block(|request: f64| JsonMessage::from(request))
            },
        );
        registry
    }

    fn node_pair_diagram(source_builder: &str, target_builder: &str) -> Diagram {
        Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "source",
            "ops": {
                "source": {
                    "type": "node",
                    "builder": source_builder,
                    "next": "target"
                },
                "target": {
                    "type": "node",
                    "builder": target_builder,
                    "next": { "builtin": "terminate" }
                }
            }
        }))
        .unwrap()
    }

    fn node_pair_candidate(
        id: &str,
        source_builder: &str,
        target_builder: &str,
    ) -> CompatibilityCandidate {
        let source_port: PortRef = output_ref(&"source".into()).next().into();
        let target_port: PortRef = (&NextOperation::Name("target".into())).into();
        CompatibilityCandidate {
            id: id.to_string(),
            diagram: node_pair_diagram(source_builder, target_builder),
            focus_ports: vec![source_port.clone(), target_port.clone()],
            source_port: Some(source_port),
            target_port: Some(target_port),
        }
    }

    fn status_for(source_builder: &str, target_builder: &str) -> CompatibilityResult {
        check_compatibility_candidate(
            &test_registry(),
            node_pair_candidate("candidate", source_builder, target_builder),
        )
    }

    #[test]
    fn compatibility_exact_node_to_node_match() {
        let result = status_for("json_to_i64", "i64_to_json");
        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(!result.provisional);
        assert!(result.reason.contains("match"));
    }

    #[test]
    fn compatibility_registered_conversion() {
        let result = status_for("json_to_i64", "f64_to_json");
        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(!result.provisional);
        assert!(result.reason.contains("conversion"));
    }

    #[test]
    fn compatibility_implicit_json_serialization() {
        let result = status_for("json_to_i64", "json_identity");
        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(result.reason.contains("serialized"));
    }

    #[test]
    fn compatibility_implicit_json_deserialization() {
        let result = status_for("json_identity", "i64_to_json");
        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(result.reason.contains("deserialized"));
    }

    #[test]
    fn compatibility_incompatible_custom_node_pair() {
        let mut registry = test_registry();
        registry.register_node_builder(
            NodeBuilderOptions::new("bool_to_json"),
            |builder: &mut Builder, _config: ()| {
                builder.create_map_block(|request: bool| JsonMessage::from(request))
            },
        );
        let result = check_compatibility_candidate(
            &registry,
            node_pair_candidate("candidate", "json_to_i64", "bool_to_json"),
        );
        assert_eq!(result.status, CompatibilityStatus::Incompatible);
        assert!(!result.provisional);
    }

    #[test]
    fn compatibility_ignores_unfocused_unfinished_ports() {
        let registry = test_registry();
        let source_port: PortRef = output_ref(&"source".into()).next().into();
        let target_port: PortRef = (&NextOperation::Name("target".into())).into();
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "source",
            "ops": {
                "source": {
                    "type": "node",
                    "builder": "json_to_i64",
                    "next": "target"
                },
                "target": {
                    "type": "node",
                    "builder": "i64_to_json",
                    "next": { "builtin": "terminate" }
                },
                "unfinished": {
                    "type": "buffer"
                }
            }
        }))
        .unwrap();
        let result = check_compatibility_candidate(
            &registry,
            CompatibilityCandidate {
                id: "candidate".to_string(),
                diagram,
                focus_ports: vec![source_port.clone(), target_port.clone()],
                source_port: Some(source_port),
                target_port: Some(target_port),
            },
        );
        assert_eq!(result.status, CompatibilityStatus::Compatible);
    }

    #[test]
    fn compatibility_focused_unknown_builder_reports_failure() {
        let result = check_compatibility_candidate(
            &test_registry(),
            node_pair_candidate("candidate", "json_to_i64", "missing_builder"),
        );
        assert_eq!(result.status, CompatibilityStatus::Incompatible);
        assert!(!result.provisional);
        assert!(result.reason.contains("missing_builder"));
    }

    #[test]
    fn compatibility_one_sided_message_port_is_provisional() {
        let source_port: PortRef = output_ref(&"source".into()).next().into();
        let result = check_compatibility_candidate(
            &test_registry(),
            CompatibilityCandidate {
                id: "one-sided".to_string(),
                diagram: node_pair_diagram("json_to_i64", "i64_to_json"),
                focus_ports: vec![source_port.clone()],
                source_port: Some(source_port),
                target_port: None,
            },
        );

        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(result.provisional);
    }

    #[test]
    fn compatibility_allows_incomplete_buffer_connections_provisionally() {
        for operation_type in ["listen", "join", "buffer_access"] {
            let buffer_port: PortRef = (&NextOperation::Name("buffer".into())).into();
            let diagram = Diagram::from_json(json!({
                "version": "0.1.0",
                "start": { "builtin": "dispose" },
                "ops": {
                    "buffer": {
                        "type": "buffer"
                    },
                    "consumer": {
                        "type": operation_type,
                        "buffers": ["buffer"],
                        "next": { "builtin": "dispose" }
                    }
                }
            }))
            .unwrap();

            let result = check_compatibility_candidate(
                &test_registry(),
                CompatibilityCandidate {
                    id: operation_type.to_string(),
                    diagram,
                    focus_ports: vec![buffer_port.clone()],
                    source_port: None,
                    target_port: None,
                },
            );

            assert_eq!(result.status, CompatibilityStatus::Compatible);
            assert!(result.provisional);
            assert!(result.reason.contains("more type context"));
        }
    }

    #[test]
    fn compatibility_allows_listen_output_missing_context_provisionally() {
        let source_port: PortRef = output_ref(&"listen".into()).next().into();
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "buffer",
            "ops": {
                "buffer": {
                    "type": "buffer"
                },
                "listen": {
                    "type": "listen",
                    "buffers": ["buffer"],
                    "next": { "builtin": "dispose" }
                }
            }
        }))
        .unwrap();

        let result = check_compatibility_candidate(
            &test_registry(),
            CompatibilityCandidate {
                id: "listen-output".to_string(),
                diagram,
                focus_ports: vec![source_port.clone()],
                source_port: Some(source_port),
                target_port: None,
            },
        );

        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(result.provisional);
        assert!(result.reason.contains("more type context"));
    }

    #[test]
    fn compatibility_allows_buffer_access_output_missing_context_provisionally() {
        let source_port: PortRef = output_ref(&"buffer_access".into()).next().into();
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "buffer",
            "ops": {
                "buffer": {
                    "type": "buffer"
                },
                "buffer_access": {
                    "type": "buffer_access",
                    "buffers": ["buffer"],
                    "next": { "builtin": "dispose" }
                }
            }
        }))
        .unwrap();

        let result = check_compatibility_candidate(
            &test_registry(),
            CompatibilityCandidate {
                id: "buffer-access-output".to_string(),
                diagram,
                focus_ports: vec![source_port.clone()],
                source_port: Some(source_port),
                target_port: None,
            },
        );

        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(result.provisional);
        assert!(result.reason.contains("more type context"));
    }

    #[test]
    fn compatibility_does_not_allow_hard_buffer_layout_mismatch_provisionally() {
        let mut registry = test_registry();
        registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .register_node_builder(
                NodeBuilderOptions::new("listen_string_buffer"),
                |builder: &mut Builder, _config: ()| -> Node<Vec<BufferKey<String>>, usize, ()> {
                    builder.create_node(
                        (|Blocking { request, .. }: Blocking<Vec<BufferKey<String>>>,
                          _access: BufferAccess<String>| {
                            request.len()
                        })
                        .into_callback(),
                    )
                },
            )
            .with_listen();

        let source_port: PortRef = output_ref(&"listen".into()).next().into();
        let target_port: PortRef = (&NextOperation::Name("listen_string_buffer".into())).into();
        let buffer_port: PortRef = (&NextOperation::Name("buffer".into())).into();
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": { "builtin": "dispose" },
            "ops": {
                "buffer": {
                    "type": "buffer"
                },
                "listen": {
                    "type": "listen",
                    "buffers": { "foo": "buffer" },
                    "next": "listen_string_buffer"
                },
                "listen_string_buffer": {
                    "type": "node",
                    "builder": "listen_string_buffer",
                    "next": { "builtin": "terminate" }
                }
            }
        }))
        .unwrap();

        let result = check_compatibility_candidate(
            &registry,
            CompatibilityCandidate {
                id: "hard-buffer-mismatch".to_string(),
                diagram,
                focus_ports: vec![
                    source_port.clone(),
                    target_port.clone(),
                    buffer_port.clone(),
                ],
                source_port: Some(source_port),
                target_port: Some(target_port),
            },
        );

        assert_eq!(result.status, CompatibilityStatus::Incompatible);
        assert!(!result.provisional);
    }

    #[test]
    fn compatibility_allows_incomplete_buffer_connection() {
        let buffer_port: PortRef = (&NextOperation::Name("buffer".into())).into();
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": { "builtin": "dispose" },
            "ops": {
                "buffer": {
                    "type": "buffer"
                },
                "listen": {
                    "type": "listen",
                    "buffers": ["buffer"],
                    "next": { "builtin": "dispose" }
                }
            }
        }))
        .unwrap();

        let result = check_compatibility_candidate(
            &test_registry(),
            CompatibilityCandidate {
                id: "candidate".to_string(),
                diagram,
                focus_ports: vec![buffer_port],
                source_port: None,
                target_port: None,
            },
        );

        assert_eq!(result.status, CompatibilityStatus::Compatible);
        assert!(result.provisional);
        assert!(result.reason.contains("more type context"));
    }
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugSessionEnd {
    Ok(serde_json::Value),
    Err(String),
}

#[cfg(feature = "debug")]
impl DebugSessionEnd {
    fn err_from_status_code(status_code: StatusCode) -> Self {
        Self::Err(status_code.to_string())
    }
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugSessionFeedback {
    OperationStarted(String),
}

#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum DebugSessionMessage {
    Feedback(DebugSessionFeedback),
    Finish(DebugSessionEnd),
}

/// Start a debug session.
#[cfg(feature = "debug")]
async fn ws_debug<W, R, Text>(mut write: W, mut read: R, state: State<ExecutorState>)
where
    W: WebsocketSinkExt<DebugSessionMessage>,
    R: WebsocketStreamExt<PostRunRequest, Text>,
    Text: std::ops::Deref<Target = str>,
{
    let req: PostRunRequest = if let Some(req) = read.next_json().await {
        req
    } else {
        return;
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let (feedback_tx, mut feedback_rx) = tokio::sync::broadcast::channel(10);
    if let Err(err) = state
        .send_chan
        .send(Context {
            registry: state.registry.clone(),
            diagram: req.diagram,
            request: req.request,
            response_tx,
            feedback_tx: Some(FeedbackSender(feedback_tx)),
        })
        .await
    {
        error!("{}", err);
        write
            .send_json(&DebugSessionMessage::Finish(
                DebugSessionEnd::err_from_status_code(StatusCode::INTERNAL_SERVER_ERROR),
            ))
            .await;
        return;
    }

    let write = tokio::sync::Mutex::new(write);

    let process_response = async || {
        let response_result = response_rx.await;

        let workflow_response = match response_result {
            Ok(response) => response,
            Err(err) => {
                error!("{}", err);
                write
                    .lock()
                    .await
                    .send_json(&DebugSessionMessage::Finish(
                        DebugSessionEnd::err_from_status_code(StatusCode::INTERNAL_SERVER_ERROR),
                    ))
                    .await;
                return;
            }
        };

        match workflow_response {
            Ok((outcome, workflow)) => {
                let result = outcome.await;
                if let Err(err) = state.despawn_chan.send(workflow).await {
                    error!("Failed to request workflow despawn: {err}");
                }

                match result {
                    Ok(response) => {
                        write
                            .lock()
                            .await
                            .send_json(&DebugSessionMessage::Finish(DebugSessionEnd::Ok(response)))
                            .await;
                    }
                    Err(err) => {
                        write
                            .lock()
                            .await
                            .send_json(&DebugSessionMessage::Finish(DebugSessionEnd::Err(
                                err.to_string(),
                            )))
                            .await;
                    }
                }
            }
            Err(err) => {
                write
                    .lock()
                    .await
                    .send_json(&DebugSessionMessage::Finish(DebugSessionEnd::Err(
                        err.to_string(),
                    )))
                    .await;
                return;
            }
        };
    };

    let mut process_feedback = async || loop {
        let feedback = feedback_rx.recv().await;

        match feedback {
            Ok(feedback) => {
                let op_id =
                    trace_event_operation_id(&feedback).unwrap_or_else(|| "[unknown]".to_string());

                write
                    .lock()
                    .await
                    .send_json(&DebugSessionMessage::Feedback(
                        DebugSessionFeedback::OperationStarted(op_id),
                    ))
                    .await;
            }
            Err(e) => match e {
                BroadcastRecvError::Closed => {
                    break;
                }
                BroadcastRecvError::Lagged(_) => {
                    warn!("{}", e);
                    break;
                }
            },
        }
    };

    tokio::select! {
        _ = process_response() => {},
        _ = process_feedback() => {},
    };
}

#[cfg(feature = "debug")]
fn trace_event_operation_id(feedback: &TracedEvent) -> Option<String> {
    let info = match &feedback.event {
        trace::TracedEventKind::MessageSent(msg) => msg
            .input
            .info
            .as_ref()
            .or_else(|| msg.output.iter().find_map(|source| source.info.as_ref())),
        trace::TracedEventKind::BufferEvent(buffer) => buffer.accessor.info.as_ref(),
        trace::TracedEventKind::OutputDisposed(disposed) => disposed.trigger.info.as_ref(),
        trace::TracedEventKind::SessionEvent(_) | trace::TracedEventKind::Broken(_) => None,
    }?;

    info.id().as_ref().map(ToString::to_string)
}

#[derive(bevy_ecs::prelude::Resource)]
struct RequestReceiver(tokio::sync::mpsc::Receiver<Context>);

/// Receiver for workflows that need to be despawned.
#[derive(bevy_ecs::prelude::Resource)]
struct WorkflowDespawnReceiver(tokio::sync::mpsc::Receiver<Entity>);

/// Receives a request from executor service and schedules the workflow.
fn execute_requests(
    mut rx: bevy_ecs::system::ResMut<RequestReceiver>,
    mut cmds: bevy_ecs::system::Commands,
    mut app_exit_events: bevy_ecs::event::EventWriter<bevy_app::AppExit>,
) {
    let rx = &mut rx.0;
    match rx.try_recv() {
        Ok(ctx) => {
            let registry = &*ctx.registry.lock().unwrap();
            let maybe_outcome = match ctx.diagram.spawn_io_workflow(&mut cmds, registry) {
                Ok(workflow) => {
                    let series = cmds.request(ctx.request, workflow);
                    let session = series.session_id();
                    let outcome: Outcome<serde_json::Value> = series.outcome();
                    if let Some(feedback_tx) = ctx.feedback_tx {
                        cmds.entity(session).insert(feedback_tx);
                    }
                    Ok((outcome, workflow.provider()))
                }
                Err(err) => Err(err.into()),
            };
            // assuming that workflows are automatically cancelled when the promise is dropped.
            if let Err(_) = ctx.response_tx.send(maybe_outcome) {
                error!("failed to send response")
            }
        }
        Err(err) => match err {
            TryRecvError::Empty => {}
            TryRecvError::Disconnected => {
                app_exit_events.write_default();
            }
        },
    }
}

fn debug_feedback(
    mut op_started: bevy_ecs::event::EventReader<trace::TracedEvent>,
    feedback_query: bevy_ecs::system::Query<(Entity, &FeedbackSender)>,
) {
    for ev in op_started.read() {
        for (session, channel) in &feedback_query {
            if ev.event.is_for_session(session) {
                let _ = channel.0.send(ev.clone());
            }
        }
    }
}

fn despawn_workflows(
    mut receiver: bevy_ecs::system::ResMut<WorkflowDespawnReceiver>,
    mut commands: bevy_ecs::system::Commands,
) {
    while let Ok(workflow) = receiver.0.try_recv() {
        let Ok(mut e) = commands.get_entity(workflow) else {
            continue;
        };

        e.despawn();
    }
}

#[non_exhaustive]
pub struct ExecutorOptions {
    pub response_timeout: Duration,
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        Self {
            response_timeout: Duration::from_secs(15),
        }
    }
}

/// Use this to set up a full-fledged bevy App to be used as a diagram execution server.
/// Pass in just the main subapp using `&mut app.sub_apps_mut().main`.
pub fn setup_bevy_app(
    app: &mut bevy_app::SubApp,
    registry: DiagramElementRegistry,
    options: &ExecutorOptions,
) -> ExecutorState {
    let (request_tx, request_rx) = tokio::sync::mpsc::channel::<Context>(10);
    let (despawn_tx, despawn_rx) = tokio::sync::mpsc::channel(10);
    app.insert_resource(RequestReceiver(request_rx));
    app.insert_resource(WorkflowDespawnReceiver(despawn_rx));
    app.add_systems(bevy_app::Update, execute_requests);
    app.add_systems(bevy_app::Update, debug_feedback.after(execute_requests));
    app.add_systems(bevy_app::Update, despawn_workflows);

    ExecutorState {
        registry: Arc::new(Mutex::new(registry)),
        send_chan: request_tx,
        despawn_chan: despawn_tx,
        response_timeout: options.response_timeout,
    }
}

/// Use this for WASM builds to set up a SubApp that does not belong to any App.
/// WASM builds need to use just a plain SubApp because the full-fledged App
/// struct no longer implements Send as of Bevy 0.16.
pub fn setup_bevy_app_wasm(
    app: &mut bevy_app::SubApp,
    registry: DiagramElementRegistry,
    options: &ExecutorOptions,
) -> ExecutorState {
    setup_subapp_defaults(app);
    setup_bevy_app(app, registry, options)
}

/// We need to manually setup the SubApp the way it would be setup by a regular
/// App, because we no longer get the benefit of a regular App in this highly
/// async environment.
///
/// This function definition is based on [`bevy_app::App::default()`]
fn setup_subapp_defaults(app: &mut bevy_app::SubApp) {
    use bevy_ecs::schedule::ScheduleLabel;
    app.update_schedule = Some(bevy_app::Main.intern());

    app.init_resource::<bevy_ecs::reflect::AppTypeRegistry>();
    app.register_type::<bevy_ecs::name::Name>();
    app.register_type::<bevy_ecs::hierarchy::ChildOf>();
    app.register_type::<bevy_ecs::hierarchy::Children>();

    app.add_plugins(bevy_app::MainSchedulePlugin);
    app.add_systems(
        bevy_app::First,
        bevy_ecs::event::event_update_system
            .in_set(bevy_ecs::event::EventUpdates)
            .run_if(bevy_ecs::event::event_update_condition),
    );
    app.add_event::<bevy_app::AppExit>();
}

#[cfg(feature = "router")]
pub(super) fn new_router(
    app: &mut bevy_app::App,
    registry: DiagramElementRegistry,
    options: ExecutorOptions,
) -> Router {
    let executor_state = setup_bevy_app(&mut app.sub_apps_mut().main, registry, &options);

    let router = Router::new()
        .route("/run", post(post_run))
        .route("/compatibility", post(post_compatibility));

    #[cfg(feature = "debug")]
    let router = router.route(
        "/debug",
        routing::any(
            async |ws: ws::WebSocketUpgrade, state: State<ExecutorState>| {
                ws.on_upgrade(|socket| {
                    use futures_util::StreamExt;

                    let (write, read) = socket.split();
                    ws_debug(write, read, state)
                })
            },
        ),
    );

    let router = router.with_state(executor_state);
    router
}

#[cfg(feature = "router")]
#[cfg(test)]
mod tests {
    #[cfg(feature = "debug")]
    use axum::extract::ws;
    use axum::{
        body,
        http::{Request, header},
    };
    use crossflow::{
        CrossflowExecutorApp, NextOperation, NodeBuilderOptions, OperationRef, output_ref,
    };
    #[cfg(feature = "debug")]
    use futures_util::SinkExt;
    use mime_guess::mime;
    use serde_json::json;
    use std::thread;
    use tower::ServiceExt;

    use super::*;

    struct TestFixture<CleanupFn> {
        router: Router,
        cleanup_test: CleanupFn,
    }

    async fn setup_test() -> TestFixture<impl FnOnce()> {
        let mut registry = DiagramElementRegistry::new();
        registry.register_node_builder(NodeBuilderOptions::new("add7"), |builder, _config: ()| {
            builder.create_map_block(|req: i32| req + 7)
        });

        let (send_stop, mut recv_stop) = tokio::sync::oneshot::channel::<()>();
        let (router_sender, router_receiver) = tokio::sync::oneshot::channel();

        let join_handle = thread::spawn(move || {
            // We need to instantiate the App inside the thread that it will run
            // inside because App is no longer Send as of Bevy 0.14.
            let mut app = bevy_app::App::new();
            app.add_plugins(CrossflowExecutorApp::default());
            app.add_systems(
                bevy_app::Update,
                move |mut app_exit: bevy_ecs::event::EventWriter<bevy_app::AppExit>| {
                    if let Ok(_) = recv_stop.try_recv() {
                        app_exit.write_default();
                    }
                },
            );

            let router = new_router(&mut app, registry, ExecutorOptions::default());
            let _ = router_sender.send(router);

            app.run();
        });

        let router = router_receiver.await.unwrap();

        TestFixture {
            router,
            cleanup_test: move || {
                send_stop.send(()).unwrap();
                join_handle.join().unwrap();
            },
        }
    }

    fn new_add7_diagram() -> Diagram {
        Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "add7",
            "ops": {
                "add7": {
                    "type": "node",
                    "builder": "add7",
                    "next": { "builtin": "terminate" },
                },
            },
        }))
        .unwrap()
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_post_run() {
        let TestFixture {
            router,
            cleanup_test,
        } = setup_test().await;

        let diagram = new_add7_diagram();

        let request_body = PostRunRequest {
            diagram,
            request: serde_json::Value::from(5),
        };
        let response = router
            .oneshot(
                Request::post("/run")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                    .body(serde_json::to_string(&request_body).unwrap())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            mime::APPLICATION_JSON
        );
        let resp_bytes = body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let resp_str = str::from_utf8(&resp_bytes).unwrap();
        let resp: i32 = serde_json::from_str(resp_str).unwrap();
        assert_eq!(resp, 12);

        cleanup_test();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_post_compatibility() {
        let TestFixture {
            router,
            cleanup_test,
        } = setup_test().await;

        let source_port: PortRef = output_ref(&"add7".into()).next().into();
        let target_port: PortRef = OperationRef::Terminate(Default::default()).into();
        let request_body = CompatibilityRequest {
            candidates: vec![CompatibilityCandidate {
                id: "add7-to-terminate".to_string(),
                diagram: new_add7_diagram(),
                focus_ports: vec![source_port.clone(), target_port.clone()],
                source_port: Some(source_port),
                target_port: Some(target_port),
            }],
        };
        let response = router
            .oneshot(
                Request::post("/compatibility")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                    .body(serde_json::to_string(&request_body).unwrap())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let resp_bytes = body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let resp_str = str::from_utf8(&resp_bytes).unwrap();
        let resp: CompatibilityResponse = serde_json::from_str(resp_str).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].status, CompatibilityStatus::Compatible);
        assert!(!resp.results[0].provisional);

        cleanup_test();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_post_compatibility_serializes_provisional_result() {
        let TestFixture {
            router,
            cleanup_test,
        } = setup_test().await;

        let buffer_port: PortRef = (&NextOperation::Name("buffer".into())).into();
        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": { "builtin": "dispose" },
            "ops": {
                "buffer": {
                    "type": "buffer"
                },
                "listen": {
                    "type": "listen",
                    "buffers": ["buffer"],
                    "next": { "builtin": "dispose" }
                }
            }
        }))
        .unwrap();
        let request_body = CompatibilityRequest {
            candidates: vec![CompatibilityCandidate {
                id: "buffer-to-listen".to_string(),
                diagram,
                focus_ports: vec![buffer_port],
                source_port: None,
                target_port: None,
            }],
        };
        let response = router
            .oneshot(
                Request::post("/compatibility")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                    .body(serde_json::to_string(&request_body).unwrap())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let resp_bytes = body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let resp_str = str::from_utf8(&resp_bytes).unwrap();
        assert!(resp_str.contains("\"provisional\":true"));
        let resp: CompatibilityResponse = serde_json::from_str(resp_str).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].status, CompatibilityStatus::Compatible);
        assert!(resp.results[0].provisional);

        cleanup_test();
    }

    #[cfg(feature = "debug")]
    struct WsTestFixture<CleanupFn> {
        executor_state: ExecutorState,
        cleanup_test: CleanupFn,
    }

    #[cfg(feature = "debug")]
    fn setup_ws_test() -> WsTestFixture<impl FnOnce()> {
        let (send_stop, mut recv_stop) = tokio::sync::oneshot::channel::<()>();
        let (send_executor_state, recv_executor_state) = std::sync::mpsc::channel();
        let join_handle = thread::spawn(move || {
            let mut app = bevy_app::App::new();
            app.add_plugins(CrossflowExecutorApp::default());
            app.add_systems(
                bevy_app::Update,
                move |mut app_exit: bevy_ecs::event::EventWriter<bevy_app::AppExit>| {
                    if let Ok(_) = recv_stop.try_recv() {
                        app_exit.write_default();
                    }
                },
            );

            let mut registry = DiagramElementRegistry::new();
            registry
                .register_node_builder(NodeBuilderOptions::new("add7"), |builder, _config: ()| {
                    builder.create_map_block(|req: i32| req + 7)
                });
            let executor_state = setup_bevy_app(
                &mut app.sub_apps_mut().main,
                registry,
                &ExecutorOptions::default(),
            );
            let _ = send_executor_state.send(executor_state);
            app.run();
        });
        let executor_state = recv_executor_state.recv().unwrap();

        WsTestFixture {
            executor_state,
            cleanup_test: move || {
                send_stop.send(()).unwrap();
                join_handle.join().unwrap();
            },
        }
    }

    #[cfg(feature = "debug")]
    #[ignore = "tracing events in `crossflow` is delayed"]
    #[tokio::test]
    #[test_log::test]
    async fn test_ws_debug() {
        use futures_util::StreamExt;

        let WsTestFixture {
            executor_state,
            cleanup_test,
        } = setup_ws_test();

        let mut diagram = new_add7_diagram();
        diagram.default_trace = crossflow::TraceToggle::On;

        let request_body = PostRunRequest {
            diagram,
            request: serde_json::Value::from(5),
        };

        // Need to use "futures" channels rather than "tokio" channels as they implement `Sink` and
        // `Stream`
        let (socket_write, mut test_rx) = futures_channel::mpsc::channel(1024);
        let (mut test_tx, socket_read) = futures_channel::mpsc::channel(1024);

        tokio::spawn(ws_debug(socket_write, socket_read, State(executor_state)));

        test_tx
            .send(Ok(ws::Message::Text(
                serde_json::to_string(&request_body).unwrap().into(),
            )))
            .await
            .unwrap();

        // there should be 2 feedback messages
        for _ in 0..2 {
            let msg = test_rx.next().await.unwrap();
            let feedback_msg: DebugSessionMessage =
                serde_json::from_slice(msg.into_text().unwrap().as_bytes()).unwrap();
            let feedback = match feedback_msg {
                DebugSessionMessage::Feedback(feedback) => feedback,
                _ => {
                    panic!("expected feedback message");
                }
            };
            assert!(matches!(
                feedback,
                DebugSessionFeedback::OperationStarted(_)
            ));
        }

        let resp_msg = test_rx.next().await.unwrap();
        let resp_text = resp_msg.into_text().unwrap();
        let resp: DebugSessionEnd = serde_json::from_slice(resp_text.as_bytes()).unwrap();
        let resp = match resp {
            DebugSessionEnd::Ok(resp) => resp,
            _ => {
                panic!("expected response to be Ok");
            }
        };
        assert_eq!(resp, serde_json::Value::from(12));

        cleanup_test();
    }
}
