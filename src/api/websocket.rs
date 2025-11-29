use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, warn};

use crate::flow::HTTPFlow;
use crate::proxy::ProxyServer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterUpdate {
    pub name: String,
    pub expr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowUpdate {
    pub flow: serde_json::Value,
    pub matching_filters: std::collections::HashMap<String, bool>,
}

pub async fn handle_socket(socket: WebSocket, proxy: Arc<ProxyServer>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = broadcast::channel::<WebSocketMessage>(100);

    // Spawn task to send messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json_msg = serde_json::to_string(&msg).unwrap_or_default();
            if sender.send(Message::Text(json_msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from client
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = handle_client_message(&text, &tx, &proxy).await {
                    warn!("Error handling WebSocket message: {}", e);
                }
            }
            Ok(Message::Close(_)) => {
                debug!("WebSocket connection closed");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
}

async fn handle_client_message(
    text: &str,
    tx: &broadcast::Sender<WebSocketMessage>,
    proxy: &Arc<ProxyServer>,
) -> Result<(), Box<dyn std::error::Error>> {
    let msg: serde_json::Value = serde_json::from_str(text)?;

    match msg["type"].as_str() {
        Some("flows/updateFilter") => {
            let payload = &msg["payload"];
            let name = payload["name"].as_str().unwrap_or("");
            let expr = payload["expr"].as_str().unwrap_or("");

            debug!("Updating filter '{}': {}", name, expr);

            // TODO: Implement filter parsing and matching
            let matching_flow_ids: Vec<String> = if expr.is_empty() {
                vec![]
            } else {
                // For now, return all flow IDs as matching
                proxy
                    .get_flows()
                    .await
                    .iter()
                    .map(|f| f.flow.id.clone())
                    .collect()
            };

            let response = WebSocketMessage {
                msg_type: "flows/filterUpdate".to_string(),
                payload: json!({
                    "name": name,
                    "matching_flow_ids": matching_flow_ids
                }),
            };

            let _ = tx.send(response);
        }
        Some(other) => {
            warn!("Unsupported WebSocket message type: {}", other);
        }
        None => {
            warn!("WebSocket message missing type field");
        }
    }

    Ok(())
}

// Function to broadcast flow updates to all connected clients
pub async fn broadcast_flow_update(
    flow: &HTTPFlow,
    update_type: &str,
    tx: &broadcast::Sender<WebSocketMessage>,
) {
    let flow_json = flow.to_json();

    // TODO: Apply filters per connection
    let matching_filters = std::collections::HashMap::new();

    let msg = WebSocketMessage {
        msg_type: update_type.to_string(),
        payload: json!({
            "flow": flow_json,
            "matching_filters": matching_filters
        }),
    };

    let _ = tx.send(msg);
}

pub async fn broadcast_flows_reset(tx: &broadcast::Sender<WebSocketMessage>) {
    let msg = WebSocketMessage {
        msg_type: "flows/reset".to_string(),
        payload: json!({}),
    };

    let _ = tx.send(msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_message_serialization() {
        let msg = WebSocketMessage {
            msg_type: "flows/add".to_string(),
            payload: json!({"test": "value"}),
        };

        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("flows/add"));
        assert!(serialized.contains("test"));
        assert!(serialized.contains("value"));
    }
}