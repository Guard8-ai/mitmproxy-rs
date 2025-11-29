//! WebSocket layer implementation
//! This mirrors the Python WebSocket layer in mitmproxy/proxy/layers/websocket.py

use crate::proxy::{Layer, Context, AnyEvent, CommandGenerator, SimpleCommandGenerator};
use crate::flow::{WebSocketMessage, WebSocketMessageType};
use tokio_tungstenite::tungstenite::Message;

/// WebSocket layer for handling WebSocket connections
#[derive(Debug)]
pub struct WebSocketLayer {
    _context: Context,
}

impl WebSocketLayer {
    pub fn new(context: Context) -> Self {
        Self { _context: context }
    }

    /// Convert WebSocket message to tungstenite message
    pub fn to_tungstenite_message(ws_msg: &WebSocketMessage) -> crate::Result<Message> {
        match ws_msg.message_type {
            WebSocketMessageType::Text => {
                let text = String::from_utf8(ws_msg.content.clone())
                    .map_err(|e| crate::Error::Other(format!("Invalid UTF-8 in WebSocket message: {}", e)))?;
                Ok(Message::Text(text))
            }
            WebSocketMessageType::Binary => Ok(Message::Binary(ws_msg.content.clone())),
            WebSocketMessageType::Ping => Ok(Message::Ping(ws_msg.content.clone())),
            WebSocketMessageType::Pong => Ok(Message::Pong(ws_msg.content.clone())),
            WebSocketMessageType::Close => {
                // For close messages, we just send a generic close frame
                // The actual close code/reason will be handled by the connection close logic
                Ok(Message::Close(None))
            }
        }
    }
}

impl Layer for WebSocketLayer {
    fn handle_event(&mut self, _event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        // TODO: Implement WebSocket event handling
        Box::new(SimpleCommandGenerator::empty())
    }

    fn layer_name(&self) -> &'static str {
        "WebSocketLayer"
    }
}