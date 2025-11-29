use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flow {
    pub id: String,
    pub flow_type: FlowType,
    pub intercepted: bool,
    pub is_replay: bool,
    pub modified: bool,
    pub marked: String,
    pub comment: String,
    pub timestamp_created: f64,
    pub client_conn: Option<Connection>,
    pub server_conn: Option<Connection>,
    pub error: Option<FlowError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlowType {
    Http,
    Tcp,
    Udp,
    Dns,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTTPFlow {
    #[serde(flatten)]
    pub flow: Flow,
    pub request: HTTPRequest,
    pub response: Option<HTTPResponse>,
    pub websocket: Option<WebSocketFlow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTTPRequest {
    pub method: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub http_version: String,
    pub headers: Vec<(String, String)>,
    pub content: Option<Vec<u8>>,
    pub content_length: Option<usize>,
    pub content_hash: Option<String>,
    pub timestamp_start: Option<f64>,
    pub timestamp_end: Option<f64>,
    pub pretty_host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTTPResponse {
    pub http_version: String,
    pub status_code: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub content: Option<Vec<u8>>,
    pub content_length: Option<usize>,
    pub content_hash: Option<String>,
    pub timestamp_start: Option<f64>,
    pub timestamp_end: Option<f64>,
    pub trailers: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketFlow {
    pub messages_meta: WebSocketMessagesMeta,
    pub closed_by_client: Option<bool>,
    pub close_code: Option<u16>,
    pub close_reason: Option<String>,
    pub timestamp_end: Option<f64>,
    pub messages: Vec<WebSocketMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessagesMeta {
    pub content_length: usize,
    pub count: usize,
    pub timestamp_last: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub content: Vec<u8>,
    pub from_client: bool,
    pub timestamp: f64,
    pub message_type: WebSocketMessageType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSocketMessageType {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub peername: Option<(String, u16)>,
    pub sockname: Option<(String, u16)>,
    pub address: Option<(String, u16)>,
    pub tls_established: bool,
    pub cert: Option<Certificate>,
    pub sni: Option<String>,
    pub cipher: Option<String>,
    pub alpn: Option<String>,
    pub tls_version: Option<String>,
    pub timestamp_start: Option<f64>,
    pub timestamp_tcp_setup: Option<f64>,
    pub timestamp_tls_setup: Option<f64>,
    pub timestamp_end: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub keyinfo: String,
    pub sha256: String,
    pub notbefore: i64,
    pub notafter: i64,
    pub serial: String,
    pub subject: IndexMap<String, String>,
    pub issuer: IndexMap<String, String>,
    pub altnames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowError {
    pub msg: String,
    pub timestamp: f64,
}

impl Flow {
    pub fn new(flow_type: FlowType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            flow_type,
            intercepted: false,
            is_replay: false,
            modified: false,
            marked: String::new(),
            comment: String::new(),
            timestamp_created: chrono::Utc::now().timestamp() as f64,
            client_conn: None,
            server_conn: None,
            error: None,
        }
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn set_error(&mut self, msg: String) {
        self.error = Some(FlowError {
            msg,
            timestamp: chrono::Utc::now().timestamp() as f64,
        });
    }

    pub fn resume(&mut self) {
        self.intercepted = false;
    }

    pub fn kill(&mut self) {
        self.set_error("Connection killed.".to_string());
    }

    pub fn killable(&self) -> bool {
        !self.is_replay && self.error.is_none()
    }
}

impl HTTPFlow {
    pub fn new(request: HTTPRequest) -> Self {
        Self {
            flow: Flow::new(FlowType::Http),
            request,
            response: None,
            websocket: None,
        }
    }

    pub fn with_response(mut self, response: HTTPResponse) -> Self {
        self.response = Some(response);
        self
    }

    pub fn with_websocket(mut self, websocket: WebSocketFlow) -> Self {
        self.websocket = Some(websocket);
        self
    }

    pub fn backup(&mut self) {
        // In the original mitmproxy, this creates a backup for revert functionality
        // For now, we'll mark it as modified
        self.flow.modified = true;
    }

    pub fn revert(&mut self) {
        // In a full implementation, this would restore from backup
        self.flow.modified = false;
    }

    pub fn copy(&self) -> Self {
        let mut new_flow = self.clone();
        new_flow.flow.id = Uuid::new_v4().to_string();
        new_flow.flow.is_replay = true;
        new_flow
    }

    pub fn to_json(&self) -> serde_json::Value {
        // Convert to the same JSON format as mitmproxy
        let mut json = serde_json::json!({
            "id": self.flow.id,
            "intercepted": self.flow.intercepted,
            "is_replay": self.flow.is_replay,
            "type": "http",
            "modified": self.flow.modified,
            "marked": self.flow.marked,
            "comment": self.flow.comment,
            "timestamp_created": self.flow.timestamp_created,
        });

        if let Some(client_conn) = &self.flow.client_conn {
            json["client_conn"] = serde_json::to_value(client_conn).unwrap();
        }

        if let Some(server_conn) = &self.flow.server_conn {
            json["server_conn"] = serde_json::to_value(server_conn).unwrap();
        }

        if let Some(error) = &self.flow.error {
            json["error"] = serde_json::to_value(error).unwrap();
        }

        json["request"] = serde_json::to_value(&self.request).unwrap();

        if let Some(response) = &self.response {
            json["response"] = serde_json::to_value(response).unwrap();
        }

        if let Some(websocket) = &self.websocket {
            json["websocket"] = serde_json::to_value(websocket).unwrap();
        }

        json
    }
}

impl HTTPRequest {
    pub fn new(
        method: String,
        scheme: String,
        host: String,
        port: u16,
        path: String,
    ) -> Self {
        let pretty_host = if port == 80 && scheme == "http" || port == 443 && scheme == "https" {
            host.clone()
        } else {
            format!("{}:{}", host, port)
        };

        Self {
            method,
            scheme,
            host,
            port,
            path,
            http_version: "HTTP/1.1".to_string(),
            headers: Vec::new(),
            content: None,
            content_length: None,
            content_hash: None,
            timestamp_start: None,
            timestamp_end: None,
            pretty_host,
        }
    }

    pub fn url(&self) -> String {
        format!("{}://{}{}", self.scheme, self.pretty_host, self.path)
    }

    pub fn set_content(&mut self, content: Vec<u8>) {
        self.content_length = Some(content.len());
        if !content.is_empty() {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&content);
            self.content_hash = Some(format!("{:x}", hasher.finalize()));
        }
        self.content = Some(content);
    }

    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    pub fn set_header(&mut self, name: String, value: String) {
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
        self.headers.push((name, value));
    }
}

impl HTTPResponse {
    pub fn new(status_code: u16, reason: String) -> Self {
        Self {
            http_version: "HTTP/1.1".to_string(),
            status_code,
            reason,
            headers: Vec::new(),
            content: None,
            content_length: None,
            content_hash: None,
            timestamp_start: None,
            timestamp_end: None,
            trailers: None,
        }
    }

    pub fn set_content(&mut self, content: Vec<u8>) {
        self.content_length = Some(content.len());
        if !content.is_empty() {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&content);
            self.content_hash = Some(format!("{:x}", hasher.finalize()));
        }
        self.content = Some(content);
    }

    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    pub fn set_header(&mut self, name: String, value: String) {
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
        self.headers.push((name, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_creation() {
        let flow = Flow::new(FlowType::Http);
        assert!(!flow.id.is_empty());
        assert!(!flow.intercepted);
        assert!(!flow.is_replay);
        assert!(!flow.modified);
    }

    #[test]
    fn test_http_request_url() {
        let request = HTTPRequest::new(
            "GET".to_string(),
            "https".to_string(),
            "example.com".to_string(),
            443,
            "/path".to_string(),
        );
        assert_eq!(request.url(), "https://example.com/path");
    }

    #[test]
    fn test_http_request_pretty_host() {
        let request = HTTPRequest::new(
            "GET".to_string(),
            "https".to_string(),
            "example.com".to_string(),
            443,
            "/".to_string(),
        );
        assert_eq!(request.pretty_host, "example.com");

        let request = HTTPRequest::new(
            "GET".to_string(),
            "https".to_string(),
            "example.com".to_string(),
            8443,
            "/".to_string(),
        );
        assert_eq!(request.pretty_host, "example.com:8443");
    }
}