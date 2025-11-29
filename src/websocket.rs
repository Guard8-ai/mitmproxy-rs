use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio_tungstenite::tungstenite::Message;

use crate::flow::{WebSocketFlow, WebSocketMessage, WebSocketMessageType, WebSocketMessagesMeta};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct WebSocketConnection {
    pub messages: VecDeque<WebSocketMessage>,
    pub closed_by_client: Option<bool>,
    pub close_code: Option<u16>,
    pub close_reason: Option<String>,
    pub timestamp_end: Option<f64>,
    pub max_messages: usize,
}

impl WebSocketConnection {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: VecDeque::new(),
            closed_by_client: None,
            close_code: None,
            close_reason: None,
            timestamp_end: None,
            max_messages,
        }
    }

    pub fn add_message(&mut self, message: WebSocketMessage) {
        self.messages.push_back(message);

        // Limit message buffer size
        while self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    pub fn close(&mut self, by_client: bool, code: Option<u16>, reason: Option<String>) {
        self.closed_by_client = Some(by_client);
        self.close_code = code;
        self.close_reason = reason;
        self.timestamp_end = Some(chrono::Utc::now().timestamp() as f64);
    }

    pub fn to_flow(&self) -> WebSocketFlow {
        let messages: Vec<WebSocketMessage> = self.messages.iter().cloned().collect();

        let content_length = messages.iter().map(|m| m.content.len()).sum();
        let timestamp_last = messages.last().map(|m| m.timestamp);

        WebSocketFlow {
            messages_meta: WebSocketMessagesMeta {
                content_length,
                count: messages.len(),
                timestamp_last,
            },
            closed_by_client: self.closed_by_client,
            close_code: self.close_code,
            close_reason: self.close_reason.clone(),
            timestamp_end: self.timestamp_end,
            messages,
        }
    }

    pub fn from_tungstenite_message(
        msg: &Message,
        from_client: bool,
    ) -> Result<WebSocketMessage> {
        let timestamp = chrono::Utc::now().timestamp() as f64;

        let (content, message_type) = match msg {
            Message::Text(text) => (text.as_bytes().to_vec(), WebSocketMessageType::Text),
            Message::Binary(data) => (data.clone(), WebSocketMessageType::Binary),
            Message::Ping(data) => (data.clone(), WebSocketMessageType::Ping),
            Message::Pong(data) => (data.clone(), WebSocketMessageType::Pong),
            Message::Close(close_frame) => {
                let content = if let Some(frame) = close_frame {
                    format!("{}: {}", frame.code, frame.reason).into_bytes()
                } else {
                    Vec::new()
                };
                (content, WebSocketMessageType::Close)
            }
            Message::Frame(_) => {
                return Err(Error::internal("Raw frames not supported"));
            }
        };

        Ok(WebSocketMessage {
            content,
            from_client,
            timestamp,
            message_type,
        })
    }

    pub fn to_tungstenite_message(ws_msg: &WebSocketMessage) -> Result<Message> {
        match ws_msg.message_type {
            WebSocketMessageType::Text => {
                let text = String::from_utf8(ws_msg.content.clone())
                    .map_err(|e| Error::internal(format!("Invalid UTF-8 in text message: {}", e)))?;
                Ok(Message::Text(text))
            }
            WebSocketMessageType::Binary => Ok(Message::Binary(ws_msg.content.clone())),
            WebSocketMessageType::Ping => Ok(Message::Ping(ws_msg.content.clone())),
            WebSocketMessageType::Pong => Ok(Message::Pong(ws_msg.content.clone())),
            WebSocketMessageType::Close => {
                // Parse close code and reason from content
                let content_str = String::from_utf8_lossy(&ws_msg.content);
                if let Some((code_str, reason)) = content_str.split_once(": ") {
                    if let Ok(code) = code_str.parse::<u16>() {
                        let reason_owned = reason.to_string();
                        return Ok(Message::Close(Some(
                            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                code: code.into(),
                                reason: reason_owned.into(),
                            },
                        )));
                    }
                }
                Ok(Message::Close(None))
            }
        }
    }

    pub fn get_messages_in_range(
        &self,
        start: Option<usize>,
        limit: Option<usize>,
    ) -> Vec<&WebSocketMessage> {
        let start = start.unwrap_or(0);
        let end = if let Some(limit) = limit {
            std::cmp::min(start + limit, self.messages.len())
        } else {
            self.messages.len()
        };

        self.messages
            .range(start..end)
            .collect()
    }

    pub fn filter_messages<F>(&self, predicate: F) -> Vec<&WebSocketMessage>
    where
        F: Fn(&WebSocketMessage) -> bool,
    {
        self.messages.iter().filter(|msg| predicate(msg)).collect()
    }

    pub fn get_message_stats(&self) -> WebSocketStats {
        let mut stats = WebSocketStats::default();

        for message in &self.messages {
            stats.total_messages += 1;
            stats.total_bytes += message.content.len();

            if message.from_client {
                stats.client_messages += 1;
                stats.client_bytes += message.content.len();
            } else {
                stats.server_messages += 1;
                stats.server_bytes += message.content.len();
            }

            match message.message_type {
                WebSocketMessageType::Text => stats.text_messages += 1,
                WebSocketMessageType::Binary => stats.binary_messages += 1,
                WebSocketMessageType::Ping => stats.ping_messages += 1,
                WebSocketMessageType::Pong => stats.pong_messages += 1,
                WebSocketMessageType::Close => stats.close_messages += 1,
            }
        }

        stats
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WebSocketStats {
    pub total_messages: usize,
    pub total_bytes: usize,
    pub client_messages: usize,
    pub client_bytes: usize,
    pub server_messages: usize,
    pub server_bytes: usize,
    pub text_messages: usize,
    pub binary_messages: usize,
    pub ping_messages: usize,
    pub pong_messages: usize,
    pub close_messages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketUpgradeInfo {
    pub upgrade_request_headers: Vec<(String, String)>,
    pub upgrade_response_headers: Vec<(String, String)>,
    pub websocket_key: String,
    pub websocket_accept: String,
    pub websocket_protocol: Option<String>,
    pub websocket_extensions: Vec<String>,
}

impl WebSocketUpgradeInfo {
    pub fn from_headers(
        request_headers: &[(String, String)],
        response_headers: &[(String, String)],
    ) -> Self {
        let websocket_key = Self::get_header_value(request_headers, "sec-websocket-key")
            .unwrap_or_default();
        let websocket_accept = Self::get_header_value(response_headers, "sec-websocket-accept")
            .unwrap_or_default();
        let websocket_protocol = Self::get_header_value(response_headers, "sec-websocket-protocol");

        let websocket_extensions = Self::get_header_value(response_headers, "sec-websocket-extensions")
            .map(|ext| ext.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        Self {
            upgrade_request_headers: request_headers.to_vec(),
            upgrade_response_headers: response_headers.to_vec(),
            websocket_key,
            websocket_accept,
            websocket_protocol,
            websocket_extensions,
        }
    }

    fn get_header_value(headers: &[(String, String)], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == name.to_lowercase())
            .map(|(_, v)| v.clone())
    }

    pub fn validate_upgrade(&self) -> Result<()> {
        // Validate WebSocket key/accept pair
        if self.websocket_key.is_empty() {
            return Err(Error::invalid_request("Missing WebSocket key"));
        }

        if self.websocket_accept.is_empty() {
            return Err(Error::invalid_request("Missing WebSocket accept"));
        }

        // In a full implementation, you would validate that the accept value
        // is correctly computed from the key
        // accept = base64(sha1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_connection() {
        let mut conn = WebSocketConnection::new(100);
        assert_eq!(conn.messages.len(), 0);

        let message = WebSocketMessage {
            content: b"Hello, WebSocket!".to_vec(),
            from_client: true,
            timestamp: chrono::Utc::now().timestamp() as f64,
            message_type: WebSocketMessageType::Text,
        };

        conn.add_message(message);
        assert_eq!(conn.messages.len(), 1);

        let flow = conn.to_flow();
        assert_eq!(flow.messages.len(), 1);
        assert_eq!(flow.messages_meta.count, 1);
        assert_eq!(flow.messages_meta.content_length, 17);
    }

    #[test]
    fn test_message_limit() {
        let mut conn = WebSocketConnection::new(2);

        for i in 0..5 {
            let message = WebSocketMessage {
                content: format!("Message {}", i).into_bytes(),
                from_client: true,
                timestamp: chrono::Utc::now().timestamp() as f64,
                message_type: WebSocketMessageType::Text,
            };
            conn.add_message(message);
        }

        // Should only keep the last 2 messages
        assert_eq!(conn.messages.len(), 2);
        assert_eq!(
            String::from_utf8_lossy(&conn.messages[0].content),
            "Message 3"
        );
        assert_eq!(
            String::from_utf8_lossy(&conn.messages[1].content),
            "Message 4"
        );
    }

    #[test]
    fn test_websocket_stats() {
        let mut conn = WebSocketConnection::new(100);

        // Add different types of messages
        conn.add_message(WebSocketMessage {
            content: b"text".to_vec(),
            from_client: true,
            timestamp: 0.0,
            message_type: WebSocketMessageType::Text,
        });

        conn.add_message(WebSocketMessage {
            content: vec![1, 2, 3, 4],
            from_client: false,
            timestamp: 0.0,
            message_type: WebSocketMessageType::Binary,
        });

        conn.add_message(WebSocketMessage {
            content: b"ping".to_vec(),
            from_client: true,
            timestamp: 0.0,
            message_type: WebSocketMessageType::Ping,
        });

        let stats = conn.get_message_stats();
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.total_bytes, 11); // 4 + 4 + 4 - 1 = 11
        assert_eq!(stats.client_messages, 2);
        assert_eq!(stats.server_messages, 1);
        assert_eq!(stats.text_messages, 1);
        assert_eq!(stats.binary_messages, 1);
        assert_eq!(stats.ping_messages, 1);
    }

    #[test]
    fn test_websocket_upgrade_info() {
        let request_headers = vec![
            ("sec-websocket-key".to_string(), "test-key".to_string()),
            ("sec-websocket-version".to_string(), "13".to_string()),
        ];

        let response_headers = vec![
            ("sec-websocket-accept".to_string(), "test-accept".to_string()),
            ("sec-websocket-protocol".to_string(), "chat".to_string()),
        ];

        let upgrade_info = WebSocketUpgradeInfo::from_headers(&request_headers, &response_headers);

        assert_eq!(upgrade_info.websocket_key, "test-key");
        assert_eq!(upgrade_info.websocket_accept, "test-accept");
        assert_eq!(upgrade_info.websocket_protocol, Some("chat".to_string()));
    }
}