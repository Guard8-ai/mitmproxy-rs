use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{Json, Response},
    Form,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::flow::HTTPFlow;
use crate::proxy::ProxyServer;
use crate::{Error, Result};

// Index handler
pub async fn index() -> &'static str {
    "mitmproxy-rs API server"
}

// Filter help
pub async fn filter_help() -> Json<Value> {
    Json(json!({
        "commands": {
            "~a": "Asset content-type",
            "~b": "Body",
            "~bq": "Body request",
            "~bs": "Body response",
            "~c": "Code",
            "~d": "Domain",
            "~dst": "Destination address",
            "~e": "Error",
            "~h": "Header",
            "~hq": "Header request",
            "~hs": "Header response",
            "~http": "HTTP flow",
            "~m": "Method",
            "~marked": "Marked flow",
            "~q": "Request",
            "~s": "Response",
            "~src": "Source address",
            "~t": "Content-type",
            "~tcp": "TCP flow",
            "~tq": "Content-type request",
            "~ts": "Content-type response",
            "~u": "URL",
            "~udp": "UDP flow",
            "~websocket": "WebSocket flow"
        }
    }))
}

// WebSocket handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(proxy): State<Arc<ProxyServer>>,
) -> Response {
    ws.on_upgrade(move |socket| crate::api::websocket::handle_socket(socket, proxy))
}

// Commands
pub async fn get_commands(State(_proxy): State<Arc<ProxyServer>>) -> Json<Value> {
    Json(json!({
        "replay.client": {
            "help": "Replay a request",
            "parameters": [
                {
                    "name": "flows",
                    "type": "sequence[flow]",
                    "kind": "POSITIONAL_OR_KEYWORD"
                }
            ],
            "return_type": null,
            "signature_help": "replay.client flows"
        },
        "set": {
            "help": "Set an option value",
            "parameters": [
                {
                    "name": "option",
                    "type": "str",
                    "kind": "POSITIONAL_OR_KEYWORD"
                },
                {
                    "name": "value",
                    "type": "str",
                    "kind": "POSITIONAL_OR_KEYWORD"
                }
            ],
            "return_type": null,
            "signature_help": "set option value"
        }
    }))
}

#[derive(Deserialize)]
pub struct ExecuteCommandRequest {
    arguments: Vec<String>,
}

pub async fn execute_command(
    Path(cmd): Path<String>,
    State(_proxy): State<Arc<ProxyServer>>,
    Json(req): Json<ExecuteCommandRequest>,
) -> Json<Value> {
    match cmd.as_str() {
        "replay.client" => {
            // TODO: Implement replay functionality
            Json(json!({"value": null}))
        }
        "set" => {
            // TODO: Implement option setting
            Json(json!({"value": null}))
        }
        _ => Json(json!({"error": format!("Unknown command: {}", cmd)})),
    }
}

// Events
pub async fn get_events(State(_proxy): State<Arc<ProxyServer>>) -> Json<Vec<Value>> {
    // TODO: Implement event logging
    Json(vec![])
}

// Flows
pub async fn get_flows(State(proxy): State<Arc<ProxyServer>>) -> Json<Vec<Value>> {
    let flows = proxy.get_flows().await;
    let json_flows: Vec<Value> = flows
        .iter()
        .map(|flow| flow.to_json())
        .collect();
    Json(json_flows)
}

#[derive(Deserialize)]
pub struct DumpQuery {
    filter: Option<String>,
}

pub async fn dump_flows(
    Query(query): Query<DumpQuery>,
    State(proxy): State<Arc<ProxyServer>>,
) -> Result<Vec<u8>, StatusCode> {
    let flows = proxy.get_flows().await;

    // TODO: Apply filter if provided
    // TODO: Implement proper binary flow serialization
    let serialized = serde_json::to_vec(&flows).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(serialized)
}

pub async fn load_flows(
    State(proxy): State<Arc<ProxyServer>>,
    body: Vec<u8>,
) -> Result<(), StatusCode> {
    // TODO: Implement flow loading from binary format
    proxy.clear_flows().await;
    Ok(())
}

pub async fn resume_flows(State(proxy): State<Arc<ProxyServer>>) -> StatusCode {
    // TODO: Resume all intercepted flows
    StatusCode::OK
}

pub async fn kill_flows(State(proxy): State<Arc<ProxyServer>>) -> StatusCode {
    // TODO: Kill all killable flows
    StatusCode::OK
}

// Individual flow operations
pub async fn get_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(flow) = proxy.get_flow(&flow_id).await {
        Ok(Json(flow.to_json()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Deserialize)]
pub struct UpdateFlowRequest {
    request: Option<UpdateRequestRequest>,
    response: Option<UpdateResponseRequest>,
    marked: Option<String>,
    comment: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateRequestRequest {
    method: Option<String>,
    scheme: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
    http_version: Option<String>,
    headers: Option<Vec<(String, String)>>,
    content: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateResponseRequest {
    http_version: Option<String>,
    code: Option<u16>,
    msg: Option<String>,
    headers: Option<Vec<(String, String)>>,
    content: Option<String>,
}

pub async fn update_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
    Json(update): Json<UpdateFlowRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut flow = proxy
        .get_flow(&flow_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    flow.backup();

    // Update request
    if let Some(req_update) = update.request {
        if let Some(method) = req_update.method {
            flow.request.method = method;
        }
        if let Some(scheme) = req_update.scheme {
            flow.request.scheme = scheme;
        }
        if let Some(host) = req_update.host {
            flow.request.host = host;
        }
        if let Some(port) = req_update.port {
            flow.request.port = port;
        }
        if let Some(path) = req_update.path {
            flow.request.path = path;
        }
        if let Some(headers) = req_update.headers {
            flow.request.headers = headers;
        }
        if let Some(content) = req_update.content {
            flow.request.set_content(content.into_bytes());
        }
    }

    // Update response
    if let Some(resp_update) = update.response {
        if let Some(ref mut response) = flow.response {
            if let Some(code) = resp_update.code {
                response.status_code = code;
            }
            if let Some(msg) = resp_update.msg {
                response.reason = msg;
            }
            if let Some(headers) = resp_update.headers {
                response.headers = headers;
            }
            if let Some(content) = resp_update.content {
                response.set_content(content.into_bytes());
            }
        }
    }

    // Update metadata
    if let Some(marked) = update.marked {
        flow.flow.marked = marked;
    }
    if let Some(comment) = update.comment {
        flow.flow.comment = comment;
    }

    if proxy.update_flow(flow).await {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn delete_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> StatusCode {
    if proxy.remove_flow(&flow_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn resume_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> StatusCode {
    if let Some(mut flow) = proxy.get_flow(&flow_id).await {
        flow.flow.resume();
        proxy.update_flow(flow).await;
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn kill_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> StatusCode {
    if let Some(mut flow) = proxy.get_flow(&flow_id).await {
        if flow.flow.killable() {
            flow.flow.kill();
            proxy.update_flow(flow).await;
        }
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn duplicate_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> Result<String, StatusCode> {
    if let Some(flow) = proxy.get_flow(&flow_id).await {
        let new_flow = flow.copy();
        let new_id = new_flow.flow.id.clone();
        // TODO: Add the new flow to the proxy
        Ok(new_id)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn replay_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> StatusCode {
    if let Some(_flow) = proxy.get_flow(&flow_id).await {
        // TODO: Implement replay functionality
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn revert_flow(
    Path(flow_id): Path<String>,
    State(proxy): State<Arc<ProxyServer>>,
) -> StatusCode {
    if let Some(mut flow) = proxy.get_flow(&flow_id).await {
        if flow.flow.is_modified() {
            flow.revert();
            proxy.update_flow(flow).await;
        }
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

// Flow content operations
pub async fn get_flow_content(
    Path((flow_id, message)): Path<(String, String)>,
    State(proxy): State<Arc<ProxyServer>>,
) -> Result<Vec<u8>, StatusCode> {
    let flow = proxy.get_flow(&flow_id).await.ok_or(StatusCode::NOT_FOUND)?;

    match message.as_str() {
        "request" => Ok(flow.request.content.unwrap_or_default()),
        "response" => {
            if let Some(response) = flow.response {
                Ok(response.content.unwrap_or_default())
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

pub async fn set_flow_content(
    Path((flow_id, message)): Path<(String, String)>,
    State(proxy): State<Arc<ProxyServer>>,
    body: Vec<u8>,
) -> StatusCode {
    if let Some(mut flow) = proxy.get_flow(&flow_id).await {
        flow.backup();

        match message.as_str() {
            "request" => {
                flow.request.set_content(body);
            }
            "response" => {
                if let Some(ref mut response) = flow.response {
                    response.set_content(body);
                }
            }
            _ => return StatusCode::BAD_REQUEST,
        }

        proxy.update_flow(flow).await;
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn get_flow_content_view(
    Path((flow_id, message, content_view)): Path<(String, String, String)>,
    State(proxy): State<Arc<ProxyServer>>,
) -> Result<Json<Value>, StatusCode> {
    let flow = proxy.get_flow(&flow_id).await.ok_or(StatusCode::NOT_FOUND)?;

    let content = match message.as_str() {
        "request" => flow.request.content.unwrap_or_default(),
        "response" => {
            if let Some(response) = flow.response {
                response.content.unwrap_or_default()
            } else {
                return Err(StatusCode::NOT_FOUND);
            }
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Simple content view implementation
    let text = String::from_utf8_lossy(&content);
    Ok(Json(json!({
        "text": text,
        "view_name": content_view,
        "syntax_highlight": false,
        "description": format!("{} content", message)
    })))
}

// Clear all
pub async fn clear_all(State(proxy): State<Arc<ProxyServer>>) -> StatusCode {
    proxy.clear_flows().await;
    StatusCode::OK
}

// Options
pub async fn get_options(State(_proxy): State<Arc<ProxyServer>>) -> Json<Value> {
    // TODO: Return actual options
    Json(json!({}))
}

pub async fn set_options(
    State(_proxy): State<Arc<ProxyServer>>,
    Json(options): Json<Value>,
) -> StatusCode {
    // TODO: Update options
    StatusCode::OK
}

pub async fn save_options(State(_proxy): State<Arc<ProxyServer>>) -> StatusCode {
    // TODO: Save options to file
    StatusCode::OK
}

// State
pub async fn get_state(State(_proxy): State<Arc<ProxyServer>>) -> Json<Value> {
    Json(json!({
        "version": "0.1.0",
        "contentViews": ["auto", "text", "json", "xml", "html"],
        "servers": {},
        "platform": std::env::consts::OS
    }))
}

// Process information
pub async fn get_processes(State(_proxy): State<Arc<ProxyServer>>) -> Json<Vec<Value>> {
    // TODO: Return process list
    Json(vec![])
}

#[derive(Deserialize)]
pub struct ExecutableIconQuery {
    path: String,
}

pub async fn get_executable_icon(
    Query(query): Query<ExecutableIconQuery>,
    State(_proxy): State<Arc<ProxyServer>>,
) -> Vec<u8> {
    // Return a transparent PNG for now
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x04, 0x00, 0x00, 0x00, 0xB5, 0x1C, 0x0C, 0x02, 0x00, 0x00, 0x00,
        0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xFC, 0xFF, 0x07, 0x00,
        0x02, 0x00, 0x01, 0xFC, 0xA8, 0x51, 0x0D, 0x68, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}