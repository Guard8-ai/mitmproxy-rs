/*!
HTTP layer implementation for mitmproxy-rs

This module provides HTTP/1.1, HTTP/2, and HTTP/3 layer implementations that exactly match
the Python mitmproxy structure in `/tmp/mitmproxy-original/mitmproxy/proxy/layers/http/`.

Key components:
- HTTP events and commands matching Python's _events.py and _base.py
- HTTP/1.1 implementation matching _http1.py
- Stream management and flow generation matching __init__.py
- Integration with TLS layers for HTTPS support
*/

use crate::connection::{Connection, ConnectionState};
use crate::flow::{HTTPFlow, HTTPRequest, HTTPResponse, Flow};
use crate::proxy::context::Context;
use crate::proxy::{commands::*, events::*, layer::*, context::*};
use crate::error::ProxyError;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use bytes::Bytes;
use tokio::sync::mpsc;
use tracing::{debug, warn, error, info};
use uuid::Uuid;
use http;
use regex::Regex;
use std::sync::OnceLock;

/// HTTP/2 events that mirror Python's h2.events.Event types
/// This enum allows us to process HTTP/2 data synchronously like Python does
#[derive(Debug, Clone)]
pub enum H2Event {
    DataReceived {
        stream_id: u32,
        data: Bytes,
        end_stream: bool,
    },
    HeadersReceived {
        stream_id: u32,
        headers: Vec<(Bytes, Bytes)>,
        end_stream: bool,
    },
    StreamReset {
        stream_id: u32,
        error_code: u32,
    },
    SettingsChanged,
    GoAway {
        error_code: u32,
        last_stream_id: u32,
    },
    WindowUpdate {
        stream_id: u32,
    },
    Ping {
        ack: bool,
        data: [u8; 8],
    },
    ProtocolError {
        message: String,
    },
    ConnectionTerminated {
        error_code: u32,
        last_stream_id: u32,
    },
}

/// Stream ID type matching Python's StreamId
pub type StreamId = i32;

/// Authority parsing function matching Python's parse_authority
/// Extracts host and port from authority string (e.g., "host:port")
/// Handles IPv6 addresses in brackets: [::1]:8080
/// Returns (host, port) tuple
fn parse_authority(authority: &str, check: bool) -> Result<(String, u16), String> {
    static AUTHORITY_RE: OnceLock<Regex> = OnceLock::new();
    let re = AUTHORITY_RE.get_or_init(|| {
        Regex::new(r"^(?P<host>[^:]+|\[.+\])(?::(?P<port>\d+))?$").unwrap()
    });

    let captures = re.captures(authority).ok_or_else(|| {
        if check {
            format!("Invalid authority format: {}", authority)
        } else {
            "Invalid authority format".to_string()
        }
    })?;

    let mut host = captures.name("host").unwrap().as_str().to_string();

    // Handle IPv6 addresses in brackets
    if host.starts_with('[') && host.ends_with(']') {
        host = host[1..host.len()-1].to_string();
    }

    // Basic host validation (simplified version of Python's is_valid_host)
    if check && (host.is_empty() || host.contains('\0')) {
        return Err("Invalid host".to_string());
    }

    let port = if let Some(port_str) = captures.name("port") {
        let port: u16 = port_str.as_str().parse().map_err(|_| "Invalid port")?;
        if check && port == 0 {
            return Err("Invalid port: 0".to_string());
        }
        port
    } else {
        // Default ports based on scheme - we'll use 80 as default here
        // The caller can adjust based on scheme
        80
    };

    Ok((host, port))
}

/// HTTP Mode enumeration matching Python's HTTPMode
#[derive(Debug, Clone, PartialEq)]
pub enum HTTPMode {
    Regular,
    Transparent,
    Upstream,
}

/// Error codes for HTTP protocol errors, matching Python's ErrorCode enum
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCode {
    GenericClientError = 1,
    GenericServerError = 2,
    RequestTooLarge = 3,
    ResponseTooLarge = 4,
    ConnectFailed = 5,
    PassthroughClose = 6,
    Kill = 7,
    Http11Required = 8,
    DestinationUnknown = 9,
    ClientDisconnected = 10,
    Cancel = 11,
    RequestValidationFailed = 12,
    ResponseValidationFailed = 13,
}

impl ErrorCode {
    /// Get HTTP status code for error, matching Python's http_status_code() method
    pub fn http_status_code(&self) -> Option<u16> {
        match self {
            ErrorCode::GenericClientError
            | ErrorCode::RequestValidationFailed
            | ErrorCode::DestinationUnknown => Some(400), // BAD_REQUEST
            ErrorCode::RequestTooLarge => Some(413), // PAYLOAD_TOO_LARGE
            ErrorCode::ConnectFailed
            | ErrorCode::GenericServerError
            | ErrorCode::ResponseValidationFailed
            | ErrorCode::ResponseTooLarge => Some(502), // BAD_GATEWAY
            ErrorCode::PassthroughClose
            | ErrorCode::Kill
            | ErrorCode::Http11Required
            | ErrorCode::ClientDisconnected
            | ErrorCode::Cancel => None,
        }
    }
}

/// Base trait for HTTP events, matching Python's HttpEvent
pub trait HttpEvent: Event {
    fn stream_id(&self) -> StreamId;
}

/// HTTP request headers event, matching Python's RequestHeaders
#[derive(Debug, Clone)]
pub struct RequestHeaders {
    pub stream_id: StreamId,
    pub request: HTTPRequest,
    pub end_stream: bool,
    pub replay_flow: Option<HTTPFlow>,
}

impl Event for RequestHeaders {
    fn event_name(&self) -> &'static str {
        "RequestHeaders"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for RequestHeaders {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP response headers event, matching Python's ResponseHeaders
#[derive(Debug, Clone)]
pub struct ResponseHeaders {
    pub stream_id: StreamId,
    pub response: HTTPResponse,
    pub end_stream: bool,
}

impl Event for ResponseHeaders {
    fn event_name(&self) -> &'static str {
        "ResponseHeaders"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for ResponseHeaders {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP request data event, matching Python's RequestData
#[derive(Debug, Clone)]
pub struct RequestData {
    pub stream_id: StreamId,
    pub data: Bytes,
}

impl Event for RequestData {
    fn event_name(&self) -> &'static str {
        "RequestData"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for RequestData {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP response data event, matching Python's ResponseData
#[derive(Debug, Clone)]
pub struct ResponseData {
    pub stream_id: StreamId,
    pub data: Bytes,
}

impl Event for ResponseData {
    fn event_name(&self) -> &'static str {
        "ResponseData"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for ResponseData {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP request end of message event, matching Python's RequestEndOfMessage
#[derive(Debug, Clone)]
pub struct RequestEndOfMessage {
    pub stream_id: StreamId,
}

impl Event for RequestEndOfMessage {
    fn event_name(&self) -> &'static str {
        "RequestEndOfMessage"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for RequestEndOfMessage {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP response end of message event, matching Python's ResponseEndOfMessage
#[derive(Debug, Clone)]
pub struct ResponseEndOfMessage {
    pub stream_id: StreamId,
}

impl Event for ResponseEndOfMessage {
    fn event_name(&self) -> &'static str {
        "ResponseEndOfMessage"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for ResponseEndOfMessage {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP request protocol error event, matching Python's RequestProtocolError
#[derive(Debug, Clone)]
pub struct RequestProtocolError {
    pub stream_id: StreamId,
    pub message: String,
    pub code: ErrorCode,
}

impl Event for RequestProtocolError {
    fn event_name(&self) -> &'static str {
        "RequestProtocolError"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for RequestProtocolError {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP response protocol error event, matching Python's ResponseProtocolError
#[derive(Debug, Clone)]
pub struct ResponseProtocolError {
    pub stream_id: StreamId,
    pub message: String,
    pub code: ErrorCode,
}

impl Event for ResponseProtocolError {
    fn event_name(&self) -> &'static str {
        "ResponseProtocolError"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for ResponseProtocolError {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// Base trait for HTTP commands, matching Python's HttpCommand
pub trait HttpCommand: Command {}

/// Command to get HTTP connection, matching Python's GetHttpConnection
#[derive(Debug, Clone)]
pub struct GetHttpConnection {
    pub address: (String, u16),
    pub tls: bool,
    pub via: Option<String>, // ServerSpec equivalent
    pub transport_protocol: String,
}

impl Command for GetHttpConnection {
    fn is_blocking(&self) -> bool {
        true
    }
}
impl HttpCommand for GetHttpConnection {}

/// Command to send HTTP event, matching Python's SendHttp
#[derive(Debug, Clone)]
pub struct SendHttp {
    pub event: Box<dyn HttpEvent>,
    pub connection: Arc<Connection>,
}

impl Command for SendHttp {
    fn is_blocking(&self) -> bool {
        false
    }
}
impl HttpCommand for SendHttp {}

/// Command to drop stream, matching Python's DropStream
#[derive(Debug, Clone)]
pub struct DropStream {
    pub stream_id: StreamId,
}

impl Command for DropStream {
    fn is_blocking(&self) -> bool {
        false
    }
}
impl HttpCommand for DropStream {}

/// Receive buffer for HTTP parsing, similar to Python's ReceiveBuffer
#[derive(Debug)]
pub struct ReceiveBuffer {
    buf: Vec<u8>,
}

impl ReceiveBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn extend(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn maybe_extract_lines(&mut self) -> Option<Vec<Vec<u8>>> {
        // Look for double CRLF indicating end of headers
        if let Some(pos) = self.find_double_crlf() {
            let headers_data = self.buf.drain(..pos + 4).collect::<Vec<u8>>();
            Some(self.split_lines(&headers_data))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    fn find_double_crlf(&self) -> Option<usize> {
        self.buf.windows(4)
            .position(|window| window == b"\r\n\r\n")
    }

    fn split_lines(&self, data: &[u8]) -> Vec<Vec<u8>> {
        data.split(|&byte| byte == b'\n')
            .map(|line| {
                if line.ends_with(&[b'\r']) {
                    line[..line.len() - 1].to_vec()
                } else {
                    line.to_vec()
                }
            })
            .filter(|line| !line.is_empty())
            .collect()
    }
}

/// HTTP stream state machine, matching Python's HttpStream
#[derive(Debug)]
pub struct HttpStream {
    pub stream_id: StreamId,
    pub flow: HTTPFlow,
    pub client_state: String,
    pub server_state: String,
    pub request_body_buf: ReceiveBuffer,
    pub response_body_buf: ReceiveBuffer,
    pub child_layer: Option<Box<dyn Layer>>,
}

impl HttpStream {
    pub fn new(context: Context, stream_id: StreamId) -> Self {
        let flow = HTTPFlow::new(
            context.client_conn.clone(),
            context.server_conn.clone(),
        );

        Self {
            stream_id,
            flow,
            client_state: "uninitialized".to_string(),
            server_state: "uninitialized".to_string(),
            request_body_buf: ReceiveBuffer::new(),
            response_body_buf: ReceiveBuffer::new(),
            child_layer: None,
        }
    }

    /// Handle incoming HTTP events, matching Python's _handle_event method
    pub async fn handle_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} handling event: {:?}", self.stream_id, std::any::type_name_of_val(&*event));

        if let Some(start_event) = event.downcast_ref::<Start>() {
            return self.handle_start().await;
        }

        // Handle HTTP events based on current state
        if let Some(req_headers) = event.downcast_ref::<RequestHeaders>() {
            return self.handle_request_headers(req_headers.clone()).await;
        }

        if let Some(req_data) = event.downcast_ref::<RequestData>() {
            return self.handle_request_data(req_data.clone()).await;
        }

        if let Some(req_end) = event.downcast_ref::<RequestEndOfMessage>() {
            return self.handle_request_end(req_end.clone()).await;
        }

        if let Some(resp_headers) = event.downcast_ref::<ResponseHeaders>() {
            return self.handle_response_headers(resp_headers.clone()).await;
        }

        if let Some(resp_data) = event.downcast_ref::<ResponseData>() {
            return self.handle_response_data(resp_data.clone()).await;
        }

        if let Some(resp_end) = event.downcast_ref::<ResponseEndOfMessage>() {
            return self.handle_response_end(resp_end.clone()).await;
        }

        if let Some(req_error) = event.downcast_ref::<RequestProtocolError>() {
            return self.handle_protocol_error(req_error.message.clone()).await;
        }

        if let Some(resp_error) = event.downcast_ref::<ResponseProtocolError>() {
            return self.handle_protocol_error(resp_error.message.clone()).await;
        }

        warn!("HttpStream {} received unhandled event: {:?}",
              self.stream_id, std::any::type_name_of_val(&*event));
        Ok(vec![])
    }

    async fn handle_start(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} starting", self.stream_id);
        self.client_state = "wait_for_request_headers".to_string();
        Ok(vec![])
    }

    async fn handle_request_headers(&mut self, event: RequestHeaders) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} received request headers for {}", self.stream_id, event.request.url);

        // Update flow with request
        self.flow.request = Some(event.request.clone());
        self.flow.live = true;

        // Validate request
        if let Err(error_msg) = self.validate_request(&event.request) {
            return Ok(vec![
                Box::new(SendHttp {
                    event: Box::new(ResponseProtocolError {
                        stream_id: self.stream_id,
                        message: error_msg,
                        code: ErrorCode::RequestValidationFailed,
                    }),
                    connection: self.flow.client_conn.clone(),
                })
            ]);
        }

        // Handle CONNECT method
        if event.request.method.to_uppercase() == "CONNECT" {
            return self.handle_connect().await;
        }

        // Set appropriate scheme/host/port based on mode
        // (Implementation would depend on proxy mode configuration)

        self.client_state = if event.end_stream {
            "done".to_string()
        } else {
            "consume_request_body".to_string()
        };
        self.server_state = "wait_for_response_headers".to_string();

        Ok(vec![])
    }

    async fn handle_request_data(&mut self, event: RequestData) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} received {} bytes of request data", self.stream_id, event.data.len());
        self.request_body_buf.extend(&event.data);
        Ok(vec![])
    }

    async fn handle_request_end(&mut self, _event: RequestEndOfMessage) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} request complete", self.stream_id);

        // Finalize request body
        if let Some(ref mut request) = self.flow.request {
            request.content = self.request_body_buf.buf.clone();
            self.request_body_buf.clear();
        }

        self.client_state = "done".to_string();

        // TODO: Trigger request hook and make server connection

        Ok(vec![])
    }

    async fn handle_response_headers(&mut self, event: ResponseHeaders) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} received response headers: {} {}",
               self.stream_id, event.response.status_code, event.response.reason);

        self.flow.response = Some(event.response.clone());

        // TODO: Validate response and trigger response headers hook

        self.server_state = if event.end_stream {
            "done".to_string()
        } else {
            "consume_response_body".to_string()
        };

        Ok(vec![])
    }

    async fn handle_response_data(&mut self, event: ResponseData) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} received {} bytes of response data", self.stream_id, event.data.len());
        self.response_body_buf.extend(&event.data);
        Ok(vec![])
    }

    async fn handle_response_end(&mut self, _event: ResponseEndOfMessage) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} response complete", self.stream_id);

        // Finalize response body
        if let Some(ref mut response) = self.flow.response {
            response.content = self.response_body_buf.buf.clone();
            self.response_body_buf.clear();
        }

        self.server_state = "done".to_string();
        self.flow.live = false;

        // Check for protocol upgrades (WebSocket, etc.)
        if let Some(ref response) = self.flow.response {
            if response.status_code == 101 {
                return self.handle_protocol_upgrade().await;
            }
        }

        Ok(vec![
            Box::new(DropStream {
                stream_id: self.stream_id,
            })
        ])
    }

    async fn handle_protocol_error(&mut self, message: String) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        error!("HttpStream {} protocol error: {}", self.stream_id, message);
        self.flow.live = false;

        Ok(vec![
            Box::new(DropStream {
                stream_id: self.stream_id,
            })
        ])
    }

    async fn handle_connect(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} handling CONNECT request", self.stream_id);

        self.client_state = "done".to_string();

        // TODO: Implement CONNECT handling with tunnel creation

        Ok(vec![])
    }

    async fn handle_protocol_upgrade(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("HttpStream {} handling protocol upgrade", self.stream_id);

        // TODO: Create child layer for upgraded protocol (WebSocket, etc.)

        Ok(vec![])
    }

    fn validate_request(&self, request: &HTTPRequest) -> Result<(), String> {
        // Basic request validation matching Python's validate_request function
        let scheme = request.url.scheme();
        if scheme != "http" && scheme != "https" && !scheme.is_empty() {
            return Err(format!("Invalid request scheme: {}", scheme));
        }

        // TODO: Add more validation logic from Python implementation

        Ok(())
    }
}

impl Layer for HttpStream {
    async fn handle_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        HttpStream::handle_event(self, event).await
    }
}

/// HTTP layer manager, matching Python's HttpLayer
#[derive(Debug)]
pub struct HttpLayer {
    pub mode: HTTPMode,
    pub streams: HashMap<StreamId, HttpStream>,
    pub connections: HashMap<String, Box<dyn Layer>>, // Connection ID -> Layer
    pub command_sources: HashMap<String, StreamId>, // Command ID -> Stream ID
    pub next_stream_id: StreamId,
}

impl HttpLayer {
    pub fn new(mode: HTTPMode) -> Self {
        Self {
            mode,
            streams: HashMap::new(),
            connections: HashMap::new(),
            command_sources: HashMap::new(),
            next_stream_id: 1,
        }
    }

    /// Create a new HTTP stream, matching Python's make_stream method
    pub fn make_stream(&mut self, context: Context) -> StreamId {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2; // Odd numbers for client-initiated streams

        let stream = HttpStream::new(context, stream_id);
        self.streams.insert(stream_id, stream);

        debug!("Created HTTP stream {}", stream_id);
        stream_id
    }

    /// Route event to appropriate child layer or stream
    pub async fn route_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // Handle start event
        if event.downcast_ref::<Start>().is_some() {
            debug!("HttpLayer starting in {:?} mode", self.mode);
            return Ok(vec![]);
        }

        // Route HTTP events to streams
        if let Some(http_event) = self.try_extract_http_event(&event) {
            let stream_id = http_event.stream_id();

            if !self.streams.contains_key(&stream_id) {
                // Create new stream if it doesn't exist
                // TODO: Get proper context
                let context = Context::default();
                self.make_stream(context);
            }

            if let Some(stream) = self.streams.get_mut(&stream_id) {
                return stream.handle_event(event).await;
            }
        }

        // Route connection events to connection handlers
        // TODO: Implement connection event routing

        warn!("HttpLayer received unhandled event: {:?}", std::any::type_name_of_val(&*event));
        Ok(vec![])
    }

    fn try_extract_http_event(&self, event: &Box<dyn Event>) -> Option<&dyn HttpEvent> {
        // Try to downcast to each HTTP event type
        if let Some(e) = event.downcast_ref::<RequestHeaders>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<ResponseHeaders>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<RequestData>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<ResponseData>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<RequestEndOfMessage>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<ResponseEndOfMessage>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<RequestProtocolError>() {
            return Some(e);
        }
        if let Some(e) = event.downcast_ref::<ResponseProtocolError>() {
            return Some(e);
        }

        None
    }
}

impl Layer for HttpLayer {
    async fn handle_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        self.route_event(event).await
    }
}

/// Utility function to format HTTP error responses
pub fn format_error(status_code: u16, message: &str) -> Vec<u8> {
    let reason = match status_code {
        400 => "Bad Request",
        404 => "Not Found",
        502 => "Bad Gateway",
        _ => "Error",
    };

    format!(
        r#"<html>
<head>
    <title>{} {}</title>
</head>
<body>
    <h1>{} {}</h1>
    <p>{}</p>
</body>
</html>"#,
        status_code, reason, status_code, reason, message
    ).into_bytes()
}

/// HTTP/1.1 connection trait, matching Python's Http1Connection
pub trait Http1Connection: Layer {
    fn stream_id(&self) -> Option<StreamId>;
    fn request(&self) -> Option<&HTTPRequest>;
    fn response(&self) -> Option<&HTTPResponse>;
    fn request_done(&self) -> bool;
    fn response_done(&self) -> bool;
}

/// HTTP/1.1 Server implementation, matching Python's Http1Server
#[derive(Debug)]
pub struct Http1Server {
    pub stream_id: StreamId,
    pub request: Option<HTTPRequest>,
    pub response: Option<HTTPResponse>,
    pub request_done: bool,
    pub response_done: bool,
    pub receive_buffer: ReceiveBuffer,
    pub state: Http1ServerState,
    pub context: Context,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Http1ServerState {
    Start,
    ReadHeaders,
    ReadBody,
    Wait,
    Done,
    Passthrough,
    Errored,
}

impl Http1Server {
    pub fn new(context: Context) -> Self {
        Self {
            stream_id: 1, // Start with stream ID 1 for server
            request: None,
            response: None,
            request_done: false,
            response_done: false,
            receive_buffer: ReceiveBuffer::new(),
            state: Http1ServerState::Start,
            context,
        }
    }

    /// Send HTTP event to client, matching Python's send method
    pub async fn send_event(&mut self, event: Box<dyn HttpEvent>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let mut commands = Vec::new();

        match event.as_ref() {
            _ if event.downcast_ref::<ResponseHeaders>().is_some() => {
                let resp_headers = event.downcast_ref::<ResponseHeaders>().unwrap();
                self.response = Some(resp_headers.response.clone());

                // Convert to HTTP/1.1 if needed and assemble response head
                let mut response = resp_headers.response.clone();
                if response.version == "HTTP/2" || response.version == "HTTP/3" {
                    response.version = "HTTP/1.1".to_string();
                    if response.reason.is_empty() {
                        response.reason = self.get_status_reason(response.status_code);
                    }
                }

                let raw_response = self.assemble_response_head(&response)?;
                commands.push(Box::new(SendData {
                    connection: self.context.client_conn.clone(),
                    data: raw_response,
                }) as Box<dyn Command>);
            }
            _ if event.downcast_ref::<ResponseData>().is_some() => {
                let resp_data = event.downcast_ref::<ResponseData>().unwrap();
                if let Some(ref response) = self.response {
                    let raw_data = if self.is_chunked_encoding(response) {
                        self.encode_chunk(&resp_data.data)
                    } else {
                        resp_data.data.to_vec()
                    };

                    if !raw_data.is_empty() {
                        commands.push(Box::new(SendData {
                            connection: self.context.client_conn.clone(),
                            data: raw_data,
                        }) as Box<dyn Command>);
                    }
                }
            }
            _ if event.downcast_ref::<ResponseEndOfMessage>().is_some() => {
                if let Some(ref request) = self.request {
                    if let Some(ref response) = self.response {
                        if request.method.to_uppercase() != "HEAD" && self.is_chunked_encoding(response) {
                            commands.push(Box::new(SendData {
                                connection: self.context.client_conn.clone(),
                                data: b"0\r\n\r\n".to_vec(),
                            }) as Box<dyn Command>);
                        }
                    }
                }
                commands.extend(self.mark_done(false, true).await?);
            }
            _ if event.downcast_ref::<ResponseProtocolError>().is_some() => {
                let resp_error = event.downcast_ref::<ResponseProtocolError>().unwrap();
                if let Some(status) = resp_error.code.http_status_code() {
                    if self.response.is_none() {
                        let error_response = self.make_error_response(status, &resp_error.message)?;
                        commands.push(Box::new(SendData {
                            connection: self.context.client_conn.clone(),
                            data: error_response,
                        }) as Box<dyn Command>);
                    }
                }
                commands.push(Box::new(CloseConnection {
                    connection: self.context.client_conn.clone(),
                }) as Box<dyn Command>);
            }
            _ => {
                return Err(ProxyError::Protocol(format!("Unexpected HTTP event: {:?}",
                    std::any::type_name_of_val(event.as_ref()))));
            }
        }

        Ok(commands)
    }

    /// Read HTTP headers from buffer, matching Python's read_headers method
    pub async fn read_headers(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if let Some(data_received) = event.downcast_ref::<DataReceived>() {
            self.receive_buffer.extend(&data_received.data);

            if let Some(request_lines) = self.receive_buffer.maybe_extract_lines() {
                match self.parse_request_head(&request_lines) {
                    Ok(request) => {
                        self.request = Some(request.clone());
                        let expected_body_size = self.calculate_expected_body_size(&request)?;

                        let commands = vec![
                            Box::new(ReceiveHttp {
                                event: Box::new(RequestHeaders {
                                    stream_id: self.stream_id,
                                    request,
                                    end_stream: expected_body_size == 0,
                                    replay_flow: None,
                                }),
                            }) as Box<dyn Command>
                        ];

                        self.state = Http1ServerState::ReadBody;
                        return Ok(commands);
                    }
                    Err(e) => {
                        let error_response = self.make_error_response(400, &e)?;
                        return Ok(vec![
                            Box::new(SendData {
                                connection: self.context.client_conn.clone(),
                                data: error_response,
                            }),
                            Box::new(CloseConnection {
                                connection: self.context.client_conn.clone(),
                            }),
                        ]);
                    }
                }
            }
        } else if let Some(_connection_closed) = event.downcast_ref::<ConnectionClosed>() {
            let buf_content = self.receive_buffer.buf.clone();
            if !buf_content.iter().all(|&b| b.is_ascii_whitespace()) {
                debug!("Client closed connection before completing request headers: {:?}",
                       String::from_utf8_lossy(&buf_content));
            }
            return Ok(vec![
                Box::new(CloseConnection {
                    connection: self.context.client_conn.clone(),
                })
            ]);
        }

        Ok(vec![])
    }

    /// Parse HTTP request head, matching Python's read_request_head
    fn parse_request_head(&self, lines: &[Vec<u8>]) -> Result<HTTPRequest, String> {
        if lines.is_empty() {
            return Err("Empty request".to_string());
        }

        // Parse request line
        let request_line = String::from_utf8_lossy(&lines[0]);
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            return Err("Invalid request line".to_string());
        }

        let method = parts[0].to_string();
        let url_str = parts[1];
        let version = parts[2].to_string();

        // Parse URL
        let url = url::Url::parse(&format!("http://example.com{}", url_str))
            .map_err(|e| format!("Invalid URL: {}", e))?;

        // Parse headers
        let mut headers = std::collections::HashMap::new();
        for line in &lines[1..] {
            if line.is_empty() {
                break;
            }

            let header_line = String::from_utf8_lossy(line);
            if let Some(colon_pos) = header_line.find(':') {
                let name = header_line[..colon_pos].trim().to_string();
                let value = header_line[colon_pos + 1..].trim().to_string();
                headers.insert(name.to_lowercase(), value);
            }
        }

        Ok(HTTPRequest {
            method,
            url,
            version,
            headers,
            content: Vec::new(),
            timestamp_start: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            timestamp_end: None,
        })
    }

    /// Calculate expected body size based on headers
    fn calculate_expected_body_size(&self, request: &HTTPRequest) -> Result<usize, ProxyError> {
        if let Some(content_length) = request.headers.get("content-length") {
            content_length.parse()
                .map_err(|_| ProxyError::Protocol("Invalid Content-Length header".to_string()))
        } else if request.headers.get("transfer-encoding")
            .map(|te| te.to_lowercase().contains("chunked"))
            .unwrap_or(false) {
            Ok(usize::MAX) // Chunked encoding
        } else {
            Ok(0) // No body
        }
    }

    /// Mark request or response as done, matching Python's mark_done method
    async fn mark_done(&mut self, request: bool, response: bool) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if request {
            self.request_done = true;
        }
        if response {
            self.response_done = true;
        }

        if self.request_done && self.response_done {
            if let (Some(ref request), Some(ref response)) = (&self.request, &self.response) {
                if self.should_make_pipe(request, response) {
                    return self.make_pipe().await;
                }

                let connection_done = self.should_close_connection(request, response);
                if connection_done {
                    self.state = Http1ServerState::Done;
                    return Ok(vec![
                        Box::new(CloseConnection {
                            connection: self.context.client_conn.clone(),
                        })
                    ]);
                }
            }

            // Reset for next request
            self.request_done = false;
            self.response_done = false;
            self.request = None;
            self.response = None;
            self.stream_id += 2; // Increment by 2 for next request
            self.state = Http1ServerState::ReadHeaders;
        }

        if self.request_done && !self.response_done {
            self.state = Http1ServerState::Wait;
        }

        Ok(vec![])
    }

    fn should_make_pipe(&self, request: &HTTPRequest, response: &HTTPResponse) -> bool {
        response.status_code == 101 ||
        (response.status_code == 200 && request.method.to_uppercase() == "CONNECT")
    }

    fn should_close_connection(&self, request: &HTTPRequest, response: &HTTPResponse) -> bool {
        // Check for Connection: close header
        request.headers.get("connection")
            .map(|conn| conn.to_lowercase().contains("close"))
            .unwrap_or(false) ||
        response.headers.get("connection")
            .map(|conn| conn.to_lowercase().contains("close"))
            .unwrap_or(false) ||
        // HTTP/1.0 defaults to close unless Keep-Alive is specified
        (request.version == "HTTP/1.0" &&
         !request.headers.get("connection")
            .map(|conn| conn.to_lowercase().contains("keep-alive"))
            .unwrap_or(false))
    }

    async fn make_pipe(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        self.state = Http1ServerState::Passthrough;
        // TODO: Handle any buffered data
        Ok(vec![])
    }

    fn assemble_response_head(&self, response: &HTTPResponse) -> Result<Vec<u8>, ProxyError> {
        let mut result = format!("{} {} {}\r\n",
            response.version, response.status_code, response.reason);

        for (name, value) in &response.headers {
            result.push_str(&format!("{}: {}\r\n", name, value));
        }
        result.push_str("\r\n");

        Ok(result.into_bytes())
    }

    fn is_chunked_encoding(&self, response: &HTTPResponse) -> bool {
        response.headers.get("transfer-encoding")
            .map(|te| te.to_lowercase().contains("chunked"))
            .unwrap_or(false)
    }

    fn encode_chunk(&self, data: &[u8]) -> Vec<u8> {
        format!("{:x}\r\n", data.len()).into_bytes()
            .into_iter()
            .chain(data.iter().cloned())
            .chain(b"\r\n".iter().cloned())
            .collect()
    }

    fn get_status_reason(&self, status_code: u16) -> String {
        match status_code {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            _ => "Unknown",
        }.to_string()
    }

    fn make_error_response(&self, status_code: u16, message: &str) -> Result<Vec<u8>, ProxyError> {
        let reason = self.get_status_reason(status_code);
        let body = format_error(status_code, message);

        let response = format!(
            "HTTP/1.1 {} {}\r\n\
             Server: mitmproxy-rs\r\n\
             Connection: close\r\n\
             Content-Type: text/html\r\n\
             Content-Length: {}\r\n\
             \r\n",
            status_code, reason, body.len()
        );

        Ok(response.into_bytes()
            .into_iter()
            .chain(body.into_iter())
            .collect())
    }
}

impl Http1Connection for Http1Server {
    fn stream_id(&self) -> Option<StreamId> {
        Some(self.stream_id)
    }

    fn request(&self) -> Option<&HTTPRequest> {
        self.request.as_ref()
    }

    fn response(&self) -> Option<&HTTPResponse> {
        self.response.as_ref()
    }

    fn request_done(&self) -> bool {
        self.request_done
    }

    fn response_done(&self) -> bool {
        self.response_done
    }
}

impl Layer for Http1Server {
    fn handle_event(&mut self, event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        // Temporary placeholder implementation to match Layer trait
        // TODO: Convert async methods to proper CommandGenerator pattern
        Box::new(SimpleCommandGenerator::empty())
    }

    fn layer_name(&self) -> &'static str {
        "Http1Server"
    }
}

impl Http1Server {
    /// Read HTTP request body, matching Python's read_body method
    async fn read_body(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if let Some(data_received) = event.downcast_ref::<DataReceived>() {
            if let Some(ref request) = self.request {
                self.receive_buffer.extend(&data_received.data);

                let expected_body_size = self.calculate_expected_body_size(request)?;

                // Handle different body reading strategies
                if expected_body_size == 0 {
                    // No body expected, mark request as done
                    return self.mark_done(true, false).await;
                } else if expected_body_size == usize::MAX {
                    // Chunked encoding - process chunks
                    return self.read_chunked_body().await;
                } else {
                    // Content-Length specified
                    return self.read_content_length_body(expected_body_size).await;
                }
            }
        } else if let Some(_connection_closed) = event.downcast_ref::<ConnectionClosed>() {
            // Handle connection closed during body reading
            if let Some(ref request) = self.request {
                let expected_body_size = self.calculate_expected_body_size(request)?;
                if expected_body_size == usize::MAX || expected_body_size == usize::MAX - 1 {
                    // Read-until-EOF semantics for HTTP/1.0 or no Content-Length
                    let remaining_data = self.receive_buffer.buf.clone();
                    if !remaining_data.is_empty() {
                        let commands = vec![
                            Box::new(ReceiveHttp {
                                event: Box::new(RequestData {
                                    stream_id: self.stream_id,
                                    data: remaining_data.into(),
                                }),
                            }) as Box<dyn Command>
                        ];
                        self.receive_buffer.clear();
                        return Ok(commands);
                    }
                    return self.mark_done(true, false).await;
                }
            }
            return Ok(vec![
                Box::new(CloseConnection {
                    connection: self.context.client_conn.clone(),
                })
            ]);
        }

        Ok(vec![])
    }

    /// Read chunked request body
    async fn read_chunked_body(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let mut commands = Vec::new();

        loop {
            // Try to read chunk size line
            if let Some(line_end) = self.find_line_end() {
                let chunk_size_line = self.receive_buffer.buf.drain(..line_end + 2).collect::<Vec<u8>>();
                let chunk_size_str = String::from_utf8_lossy(&chunk_size_line[..chunk_size_line.len() - 2]);

                // Parse chunk size (hex)
                let chunk_size = match usize::from_str_radix(chunk_size_str.trim(), 16) {
                    Ok(size) => size,
                    Err(_) => {
                        return Ok(vec![
                            Box::new(ReceiveHttp {
                                event: Box::new(RequestProtocolError {
                                    stream_id: self.stream_id,
                                    message: "Invalid chunk size".to_string(),
                                    code: ErrorCode::GenericClientError,
                                }),
                            })
                        ]);
                    }
                };

                if chunk_size == 0 {
                    // Last chunk, read trailers (if any) and finish
                    if let Some(trailer_end) = self.find_double_crlf() {
                        self.receive_buffer.buf.drain(..trailer_end + 4);
                    }
                    commands.push(Box::new(ReceiveHttp {
                        event: Box::new(RequestEndOfMessage {
                            stream_id: self.stream_id,
                        }),
                    }) as Box<dyn Command>);
                    commands.extend(self.mark_done(true, false).await?);
                    return Ok(commands);
                }

                // Check if we have the full chunk + CRLF
                if self.receive_buffer.len() >= chunk_size + 2 {
                    let chunk_data = self.receive_buffer.buf.drain(..chunk_size).collect::<Vec<u8>>();
                    self.receive_buffer.buf.drain(..2); // Remove trailing CRLF

                    commands.push(Box::new(ReceiveHttp {
                        event: Box::new(RequestData {
                            stream_id: self.stream_id,
                            data: chunk_data.into(),
                        }),
                    }) as Box<dyn Command>);
                } else {
                    // Need more data
                    break;
                }
            } else {
                // Need more data for chunk size line
                break;
            }
        }

        Ok(commands)
    }

    /// Read content-length body
    async fn read_content_length_body(&mut self, expected_size: usize) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if self.receive_buffer.len() >= expected_size {
            let body_data = self.receive_buffer.buf.drain(..expected_size).collect::<Vec<u8>>();

            let mut commands = vec![
                Box::new(ReceiveHttp {
                    event: Box::new(RequestData {
                        stream_id: self.stream_id,
                        data: body_data.into(),
                    }),
                }) as Box<dyn Command>,
                Box::new(ReceiveHttp {
                    event: Box::new(RequestEndOfMessage {
                        stream_id: self.stream_id,
                    }),
                }) as Box<dyn Command>
            ];

            commands.extend(self.mark_done(true, false).await?);
            Ok(commands)
        } else {
            // Need more data
            Ok(vec![])
        }
    }

    fn find_line_end(&self) -> Option<usize> {
        self.receive_buffer.buf.windows(2)
            .position(|window| window == b"\r\n")
    }

    fn find_double_crlf(&self) -> Option<usize> {
        self.receive_buffer.buf.windows(4)
            .position(|window| window == b"\r\n\r\n")
    }

    fn try_extract_http_event(&self, event: &Box<dyn Event>) -> Option<Box<dyn HttpEvent>> {
        // Try to downcast to each HTTP event type
        if let Some(e) = event.downcast_ref::<ResponseHeaders>() {
            return Some(Box::new(e.clone()));
        }
        if let Some(e) = event.downcast_ref::<ResponseData>() {
            return Some(Box::new(e.clone()));
        }
        if let Some(e) = event.downcast_ref::<ResponseEndOfMessage>() {
            return Some(Box::new(e.clone()));
        }
        if let Some(e) = event.downcast_ref::<ResponseProtocolError>() {
            return Some(Box::new(e.clone()));
        }

        None
    }
}

/// HTTP/1.1 Client implementation, matching Python's Http1Client
#[derive(Debug)]
pub struct Http1Client {
    pub stream_id: Option<StreamId>,
    pub request: Option<HTTPRequest>,
    pub response: Option<HTTPResponse>,
    pub request_done: bool,
    pub response_done: bool,
    pub receive_buffer: ReceiveBuffer,
    pub state: Http1ClientState,
    pub context: Context,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Http1ClientState {
    Start,
    ReadHeaders,
    ReadBody,
    Wait,
    Done,
    Passthrough,
    Errored,
}

impl Http1Client {
    pub fn new(context: Context) -> Self {
        Self {
            stream_id: None,
            request: None,
            response: None,
            request_done: false,
            response_done: false,
            receive_buffer: ReceiveBuffer::new(),
            state: Http1ClientState::Start,
            context,
        }
    }

    /// Send HTTP event to server, matching Python's send method
    pub async fn send_event(&mut self, event: Box<dyn HttpEvent>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let mut commands = Vec::new();

        // Handle RequestProtocolError separately
        if let Some(req_error) = event.downcast_ref::<RequestProtocolError>() {
            commands.push(Box::new(CloseConnection {
                connection: self.context.server_conn.clone(),
            }) as Box<dyn Command>);
            return Ok(commands);
        }

        // Set stream ID if this is the first event
        if self.stream_id.is_none() {
            if let Some(req_headers) = event.downcast_ref::<RequestHeaders>() {
                self.stream_id = Some(req_headers.stream_id);
                self.request = Some(req_headers.request.clone());
            } else {
                return Err(ProxyError::Protocol("Expected RequestHeaders as first event".to_string()));
            }
        }

        // Verify stream ID matches
        if Some(event.stream_id()) != self.stream_id {
            return Err(ProxyError::Protocol("Stream ID mismatch".to_string()));
        }

        match event.as_ref() {
            _ if event.downcast_ref::<RequestHeaders>().is_some() => {
                let req_headers = event.downcast_ref::<RequestHeaders>().unwrap();
                let mut request = req_headers.request.clone();

                // Convert HTTP/2 or HTTP/3 to HTTP/1.1 if needed
                if request.version == "HTTP/2" || request.version == "HTTP/3" {
                    request.version = "HTTP/1.1".to_string();

                    // Add Host header if missing but authority is present
                    if !request.headers.contains_key("host") {
                        if let Some(authority) = request.url.host_str() {
                            let port = request.url.port().unwrap_or(if request.url.scheme() == "https" { 443 } else { 80 });
                            let host_value = if (port == 80 && request.url.scheme() == "http") ||
                                               (port == 443 && request.url.scheme() == "https") {
                                authority.to_string()
                            } else {
                                format!("{}:{}", authority, port)
                            };
                            request.headers.insert("host".to_string(), host_value);
                        }
                    }

                    // Merge multiple Cookie headers for HTTP/1.1 compatibility
                    let cookie_values: Vec<String> = request.headers.iter()
                        .filter(|(k, _)| k.to_lowercase() == "cookie")
                        .map(|(_, v)| v.clone())
                        .collect();
                    if cookie_values.len() > 1 {
                        request.headers.retain(|k, _| k.to_lowercase() != "cookie");
                        request.headers.insert("cookie".to_string(), cookie_values.join("; "));
                    }
                }

                let raw_request = self.assemble_request_head(&request)?;
                commands.push(Box::new(SendData {
                    connection: self.context.server_conn.clone(),
                    data: raw_request,
                }) as Box<dyn Command>);
            }
            _ if event.downcast_ref::<RequestData>().is_some() => {
                let req_data = event.downcast_ref::<RequestData>().unwrap();
                if let Some(ref request) = self.request {
                    let raw_data = if self.is_chunked_encoding_request(request) {
                        self.encode_chunk(&req_data.data)
                    } else {
                        req_data.data.to_vec()
                    };

                    if !raw_data.is_empty() {
                        commands.push(Box::new(SendData {
                            connection: self.context.server_conn.clone(),
                            data: raw_data,
                        }) as Box<dyn Command>);
                    }
                }
            }
            _ if event.downcast_ref::<RequestEndOfMessage>().is_some() => {
                if let Some(ref request) = self.request {
                    if self.is_chunked_encoding_request(request) {
                        // Send final chunk
                        commands.push(Box::new(SendData {
                            connection: self.context.server_conn.clone(),
                            data: b"0\r\n\r\n".to_vec(),
                        }) as Box<dyn Command>);
                    } else {
                        // Check if we need to half-close for read-until-EOF semantics
                        let expected_body_size = if let Some(ref response) = self.response {
                            self.calculate_expected_response_body_size(request, response)?
                        } else {
                            0
                        };

                        if expected_body_size == usize::MAX - 1 { // HTTP/1.0 read-until-EOF
                            commands.push(Box::new(CloseConnection {
                                connection: self.context.server_conn.clone(),
                            }) as Box<dyn Command>);
                        }
                    }
                }
                commands.extend(self.mark_done(true, false).await?);
            }
            _ => {
                return Err(ProxyError::Protocol(format!("Unexpected HTTP event: {:?}",
                    std::any::type_name_of_val(event.as_ref()))));
            }
        }

        Ok(commands)
    }

    /// Read HTTP response headers, matching Python's read_headers method
    pub async fn read_headers(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if let Some(data_received) = event.downcast_ref::<DataReceived>() {
            if self.request.is_none() {
                // Unexpected data from server
                warn!("Unexpected data from server: {:?}", String::from_utf8_lossy(&data_received.data));
                return Ok(vec![
                    Box::new(CloseConnection {
                        connection: self.context.server_conn.clone(),
                    })
                ]);
            }

            self.receive_buffer.extend(&data_received.data);

            if let Some(response_lines) = self.receive_buffer.maybe_extract_lines() {
                match self.parse_response_head(&response_lines) {
                    Ok(response) => {
                        self.response = Some(response.clone());

                        let expected_body_size = if let Some(ref request) = self.request {
                            self.calculate_expected_response_body_size(request, &response)?
                        } else {
                            0
                        };

                        let commands = vec![
                            Box::new(ReceiveHttp {
                                event: Box::new(ResponseHeaders {
                                    stream_id: self.stream_id.unwrap(),
                                    response,
                                    end_stream: expected_body_size == 0,
                                }),
                            }) as Box<dyn Command>
                        ];

                        self.state = Http1ClientState::ReadBody;
                        return Ok(commands);
                    }
                    Err(e) => {
                        return Ok(vec![
                            Box::new(CloseConnection {
                                connection: self.context.server_conn.clone(),
                            }),
                            Box::new(ReceiveHttp {
                                event: Box::new(ResponseProtocolError {
                                    stream_id: self.stream_id.unwrap(),
                                    message: format!("Cannot parse HTTP response: {}", e),
                                    code: ErrorCode::GenericServerError,
                                }),
                            })
                        ]);
                    }
                }
            }
        } else if let Some(_connection_closed) = event.downcast_ref::<ConnectionClosed>() {
            if self.context.server_conn.state != ConnectionState::Closed {
                return Ok(vec![
                    Box::new(CloseConnection {
                        connection: self.context.server_conn.clone(),
                    })
                ]);
            }

            if let Some(stream_id) = self.stream_id {
                if !self.receive_buffer.is_empty() {
                    return Ok(vec![
                        Box::new(ReceiveHttp {
                            event: Box::new(ResponseProtocolError {
                                stream_id,
                                message: format!("Unexpected server response: {:?}",
                                    String::from_utf8_lossy(&self.receive_buffer.buf)),
                                code: ErrorCode::GenericServerError,
                            }),
                        })
                    ]);
                } else {
                    return Ok(vec![
                        Box::new(ReceiveHttp {
                            event: Box::new(ResponseProtocolError {
                                stream_id,
                                message: "Server closed connection".to_string(),
                                code: ErrorCode::GenericServerError,
                            }),
                        })
                    ]);
                }
            }
        }

        Ok(vec![])
    }

    /// Read HTTP response body, matching Python's read_body method
    pub async fn read_body(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if let Some(data_received) = event.downcast_ref::<DataReceived>() {
            if let (Some(ref request), Some(ref response)) = (&self.request, &self.response) {
                self.receive_buffer.extend(&data_received.data);

                let expected_body_size = self.calculate_expected_response_body_size(request, response)?;

                // Handle different body reading strategies
                if expected_body_size == 0 {
                    // No body expected, mark response as done
                    return self.mark_done(false, true).await;
                } else if expected_body_size == usize::MAX {
                    // Chunked encoding - process chunks
                    return self.read_chunked_response_body().await;
                } else if expected_body_size == usize::MAX - 1 {
                    // Read-until-EOF semantics
                    return self.read_until_eof_body().await;
                } else {
                    // Content-Length specified
                    return self.read_content_length_response_body(expected_body_size).await;
                }
            }
        } else if let Some(_connection_closed) = event.downcast_ref::<ConnectionClosed>() {
            // Handle connection closed during response body reading
            if let (Some(ref request), Some(ref response)) = (&self.request, &self.response) {
                let expected_body_size = self.calculate_expected_response_body_size(request, response)?;
                if expected_body_size == usize::MAX - 1 {
                    // Read-until-EOF semantics - send remaining data and finish
                    let remaining_data = self.receive_buffer.buf.clone();
                    let mut commands = Vec::new();

                    if !remaining_data.is_empty() {
                        commands.push(Box::new(ReceiveHttp {
                            event: Box::new(ResponseData {
                                stream_id: self.stream_id.unwrap(),
                                data: remaining_data.into(),
                            }),
                        }) as Box<dyn Command>);
                        self.receive_buffer.clear();
                    }

                    commands.push(Box::new(ReceiveHttp {
                        event: Box::new(ResponseEndOfMessage {
                            stream_id: self.stream_id.unwrap(),
                        }),
                    }) as Box<dyn Command>);

                    commands.extend(self.mark_done(false, true).await?);
                    return Ok(commands);
                }
            }
        }

        Ok(vec![])
    }

    /// Read chunked response body
    async fn read_chunked_response_body(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let mut commands = Vec::new();

        loop {
            // Try to read chunk size line
            if let Some(line_end) = self.find_line_end() {
                let chunk_size_line = self.receive_buffer.buf.drain(..line_end + 2).collect::<Vec<u8>>();
                let chunk_size_str = String::from_utf8_lossy(&chunk_size_line[..chunk_size_line.len() - 2]);

                // Parse chunk size (hex)
                let chunk_size = match usize::from_str_radix(chunk_size_str.trim(), 16) {
                    Ok(size) => size,
                    Err(_) => {
                        return Ok(vec![
                            Box::new(CloseConnection {
                                connection: self.context.server_conn.clone(),
                            }),
                            Box::new(ReceiveHttp {
                                event: Box::new(ResponseProtocolError {
                                    stream_id: self.stream_id.unwrap(),
                                    message: "HTTP/1 protocol error: Invalid chunk size".to_string(),
                                    code: ErrorCode::GenericServerError,
                                }),
                            })
                        ]);
                    }
                };

                if chunk_size == 0 {
                    // Last chunk, read trailers (if any) and finish
                    if let Some(trailer_end) = self.find_double_crlf() {
                        self.receive_buffer.buf.drain(..trailer_end + 4);
                    }
                    commands.push(Box::new(ReceiveHttp {
                        event: Box::new(ResponseEndOfMessage {
                            stream_id: self.stream_id.unwrap(),
                        }),
                    }) as Box<dyn Command>);
                    commands.extend(self.mark_done(false, true).await?);
                    return Ok(commands);
                }

                // Check if we have the full chunk + CRLF
                if self.receive_buffer.len() >= chunk_size + 2 {
                    let chunk_data = self.receive_buffer.buf.drain(..chunk_size).collect::<Vec<u8>>();
                    self.receive_buffer.buf.drain(..2); // Remove trailing CRLF

                    if !chunk_data.is_empty() {
                        commands.push(Box::new(ReceiveHttp {
                            event: Box::new(ResponseData {
                                stream_id: self.stream_id.unwrap(),
                                data: chunk_data.into(),
                            }),
                        }) as Box<dyn Command>);
                    }
                } else {
                    // Need more data
                    break;
                }
            } else {
                // Need more data for chunk size line
                break;
            }
        }

        Ok(commands)
    }

    /// Read response body until EOF
    async fn read_until_eof_body(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // In read-until-EOF mode, we consume all data until connection closes
        if !self.receive_buffer.is_empty() {
            let data = self.receive_buffer.buf.clone();
            self.receive_buffer.clear();

            Ok(vec![
                Box::new(ReceiveHttp {
                    event: Box::new(ResponseData {
                        stream_id: self.stream_id.unwrap(),
                        data: data.into(),
                    }),
                }) as Box<dyn Command>
            ])
        } else {
            Ok(vec![])
        }
    }

    /// Read content-length response body
    async fn read_content_length_response_body(&mut self, expected_size: usize) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if self.receive_buffer.len() >= expected_size {
            let body_data = self.receive_buffer.buf.drain(..expected_size).collect::<Vec<u8>>();

            let mut commands = vec![
                Box::new(ReceiveHttp {
                    event: Box::new(ResponseData {
                        stream_id: self.stream_id.unwrap(),
                        data: body_data.into(),
                    }),
                }) as Box<dyn Command>,
                Box::new(ReceiveHttp {
                    event: Box::new(ResponseEndOfMessage {
                        stream_id: self.stream_id.unwrap(),
                    }),
                }) as Box<dyn Command>
            ];

            commands.extend(self.mark_done(false, true).await?);
            Ok(commands)
        } else {
            // Need more data
            Ok(vec![])
        }
    }

    /// Parse HTTP response head, matching Python's read_response_head
    fn parse_response_head(&self, lines: &[Vec<u8>]) -> Result<HTTPResponse, String> {
        if lines.is_empty() {
            return Err("Empty response".to_string());
        }

        // Parse status line
        let status_line = String::from_utf8_lossy(&lines[0]);
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err("Invalid status line".to_string());
        }

        let version = parts[0].to_string();
        let status_code: u16 = parts[1].parse()
            .map_err(|_| "Invalid status code".to_string())?;
        let reason = if parts.len() > 2 {
            parts[2..].join(" ")
        } else {
            String::new()
        };

        // Parse headers
        let mut headers = std::collections::HashMap::new();
        for line in &lines[1..] {
            if line.is_empty() {
                break;
            }

            let header_line = String::from_utf8_lossy(line);
            if let Some(colon_pos) = header_line.find(':') {
                let name = header_line[..colon_pos].trim().to_lowercase();
                let value = header_line[colon_pos + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }

        Ok(HTTPResponse {
            version,
            status_code,
            reason,
            headers,
            content: Vec::new(),
            timestamp_start: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            timestamp_end: None,
        })
    }

    /// Calculate expected response body size
    fn calculate_expected_response_body_size(&self, request: &HTTPRequest, response: &HTTPResponse) -> Result<usize, ProxyError> {
        // HEAD responses never have bodies
        if request.method.to_uppercase() == "HEAD" {
            return Ok(0);
        }

        // 1xx, 204, 304 responses never have bodies
        if response.status_code < 200 || response.status_code == 204 || response.status_code == 304 {
            return Ok(0);
        }

        // CONNECT with 200 never has a body
        if request.method.to_uppercase() == "CONNECT" && response.status_code == 200 {
            return Ok(0);
        }

        // Check Transfer-Encoding first
        if let Some(te) = response.headers.get("transfer-encoding") {
            if te.to_lowercase().contains("chunked") {
                return Ok(usize::MAX); // Chunked encoding
            }
        }

        // Check Content-Length
        if let Some(content_length) = response.headers.get("content-length") {
            return content_length.parse()
                .map_err(|_| ProxyError::Protocol("Invalid Content-Length header".to_string()));
        }

        // HTTP/1.0 without Content-Length means read until EOF
        if response.version == "HTTP/1.0" {
            Ok(usize::MAX - 1) // Read-until-EOF semantics
        } else {
            Ok(0) // No body
        }
    }

    /// Assemble HTTP request head
    fn assemble_request_head(&self, request: &HTTPRequest) -> Result<Vec<u8>, ProxyError> {
        let mut result = format!("{} {} {}\r\n",
            request.method, request.url.path(), request.version);

        for (name, value) in &request.headers {
            result.push_str(&format!("{}: {}\r\n", name, value));
        }
        result.push_str("\r\n");

        Ok(result.into_bytes())
    }

    fn is_chunked_encoding_request(&self, request: &HTTPRequest) -> bool {
        request.headers.get("transfer-encoding")
            .map(|te| te.to_lowercase().contains("chunked"))
            .unwrap_or(false)
    }

    fn encode_chunk(&self, data: &[u8]) -> Vec<u8> {
        format!("{:x}\r\n", data.len()).into_bytes()
            .into_iter()
            .chain(data.iter().cloned())
            .chain(b"\r\n".iter().cloned())
            .collect()
    }

    fn find_line_end(&self) -> Option<usize> {
        self.receive_buffer.buf.windows(2)
            .position(|window| window == b"\r\n")
    }

    fn find_double_crlf(&self) -> Option<usize> {
        self.receive_buffer.buf.windows(4)
            .position(|window| window == b"\r\n\r\n")
    }

    /// Mark request or response as done, matching Python's mark_done method
    async fn mark_done(&mut self, request: bool, response: bool) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if request {
            self.request_done = true;
        }
        if response {
            self.response_done = true;
        }

        if self.request_done && self.response_done {
            if let (Some(ref request), Some(ref response)) = (&self.request, &self.response) {
                if self.should_make_pipe(request, response) {
                    return self.make_pipe().await;
                }

                // Check if connection should be closed
                let read_until_eof_semantics = self.calculate_expected_response_body_size(request, response)
                    .map(|size| size == usize::MAX - 1)
                    .unwrap_or(false);

                let connection_done = read_until_eof_semantics ||
                    self.should_close_connection(request, response) ||
                    // If we proxy HTTP/2 to HTTP/1, we only use upstream connections for one request
                    ((request.version == "HTTP/2" || request.version == "HTTP/3"));

                if connection_done {
                    self.state = Http1ClientState::Done;
                    return Ok(vec![
                        Box::new(CloseConnection {
                            connection: self.context.server_conn.clone(),
                        })
                    ]);
                }
            }

            // Reset for next request
            self.request_done = false;
            self.response_done = false;
            self.request = None;
            self.response = None;
            self.stream_id = None;
            self.state = Http1ClientState::ReadHeaders;

            // Process any buffered data
            if !self.receive_buffer.is_empty() {
                return self.read_headers(Box::new(DataReceived {
                    connection: self.context.server_conn.clone(),
                    data: vec![],
                })).await;
            }
        }

        if self.request_done && !self.response_done {
            self.state = Http1ClientState::Wait;
        }

        Ok(vec![])
    }

    fn should_make_pipe(&self, request: &HTTPRequest, response: &HTTPResponse) -> bool {
        response.status_code == 101 ||
        (response.status_code == 200 && request.method.to_uppercase() == "CONNECT")
    }

    fn should_close_connection(&self, request: &HTTPRequest, response: &HTTPResponse) -> bool {
        // Check for Connection: close header
        request.headers.get("connection")
            .map(|conn| conn.to_lowercase().contains("close"))
            .unwrap_or(false) ||
        response.headers.get("connection")
            .map(|conn| conn.to_lowercase().contains("close"))
            .unwrap_or(false) ||
        // HTTP/1.0 defaults to close unless Keep-Alive is specified
        (request.version == "HTTP/1.0" &&
         !request.headers.get("connection")
            .map(|conn| conn.to_lowercase().contains("keep-alive"))
            .unwrap_or(false))
    }

    async fn make_pipe(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        self.state = Http1ClientState::Passthrough;

        // Handle any buffered data
        if !self.receive_buffer.is_empty() {
            let already_received = self.receive_buffer.buf.clone();
            self.receive_buffer.clear();

            // Some servers send superfluous newlines after responses, eat those
            let already_received = if already_received.starts_with(b"\r\n") {
                &already_received[2..]
            } else {
                &already_received
            };

            if !already_received.is_empty() {
                let passthrough_event = Box::new(DataReceived {
                    connection: self.context.server_conn.clone(),
                    data: already_received.to_vec(),
                });
                return self.handle_passthrough(passthrough_event).await;
            }
        }

        Ok(vec![])
    }

    async fn handle_passthrough(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if let Some(data_received) = event.downcast_ref::<DataReceived>() {
            Ok(vec![
                Box::new(ReceiveHttp {
                    event: Box::new(ResponseData {
                        stream_id: self.stream_id.unwrap_or(1),
                        data: data_received.data.clone().into(),
                    }),
                }) as Box<dyn Command>
            ])
        } else if let Some(_connection_closed) = event.downcast_ref::<ConnectionClosed>() {
            Ok(vec![
                Box::new(ReceiveHttp {
                    event: Box::new(ResponseEndOfMessage {
                        stream_id: self.stream_id.unwrap_or(1),
                    }),
                }) as Box<dyn Command>
            ])
        } else {
            Ok(vec![])
        }
    }
}

impl Http1Connection for Http1Client {
    fn stream_id(&self) -> Option<StreamId> {
        self.stream_id
    }

    fn request(&self) -> Option<&HTTPRequest> {
        self.request.as_ref()
    }

    fn response(&self) -> Option<&HTTPResponse> {
        self.response.as_ref()
    }

    fn request_done(&self) -> bool {
        self.request_done
    }

    fn response_done(&self) -> bool {
        self.response_done
    }
}

impl Layer for Http1Client {
    async fn handle_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        debug!("Http1Client handling event in state {:?}: {:?}",
               self.state, std::any::type_name_of_val(&*event));

        match self.state {
            Http1ClientState::Start => {
                if event.downcast_ref::<Start>().is_some() {
                    self.state = Http1ClientState::ReadHeaders;
                    Ok(vec![])
                } else {
                    Err(ProxyError::Protocol("Expected Start event".to_string()))
                }
            }
            Http1ClientState::ReadHeaders => {
                self.read_headers(event).await
            }
            Http1ClientState::ReadBody => {
                self.read_body(event).await
            }
            Http1ClientState::Wait => {
                // Wait for next request
                if let Some(http_event) = self.try_extract_http_event(&event) {
                    self.send_event(http_event).await
                } else {
                    Ok(vec![])
                }
            }
            Http1ClientState::Done => {
                Ok(vec![])
            }
            Http1ClientState::Passthrough => {
                self.handle_passthrough(event).await
            }
            Http1ClientState::Errored => {
                // Silently consume events in error state
                Ok(vec![])
            }
        }
    }

}

/// Command to receive HTTP event, matching Python's ReceiveHttp
#[derive(Debug)]
pub struct ReceiveHttp {
    pub event: Box<dyn HttpEvent>,
}

impl Command for ReceiveHttp {
    fn is_blocking(&self) -> bool {
        false
    }
}

// ===== HTTP/2 IMPLEMENTATION =====

/// HTTP/2 stream state enumeration, matching Python's StreamState
#[derive(Debug, Clone, PartialEq)]
pub enum Http2StreamState {
    ExpectingHeaders,
    HeadersReceived,
}

/// HTTP/2 trailers event, matching Python's RequestTrailers/ResponseTrailers
#[derive(Debug, Clone)]
pub struct RequestTrailers {
    pub stream_id: StreamId,
    pub trailers: http::HeaderMap,
}

impl Event for RequestTrailers {
    fn event_name(&self) -> &'static str {
        "RequestTrailers"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for RequestTrailers {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

#[derive(Debug, Clone)]
pub struct ResponseTrailers {
    pub stream_id: StreamId,
    pub trailers: http::HeaderMap,
}

impl Event for ResponseTrailers {
    fn event_name(&self) -> &'static str {
        "ResponseTrailers"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl HttpEvent for ResponseTrailers {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

/// HTTP/2 connection configuration, matching Python's h2_conf_defaults
#[derive(Debug, Clone)]
pub struct Http2Config {
    pub header_encoding: Option<String>,
    pub validate_outbound_headers: bool,
    pub validate_inbound_headers: bool,
    pub normalize_inbound_headers: bool,
    pub normalize_outbound_headers: bool,
}

impl Default for Http2Config {
    fn default() -> Self {
        Self {
            header_encoding: None,
            validate_outbound_headers: false,
            validate_inbound_headers: false,
            normalize_inbound_headers: false,
            normalize_outbound_headers: false,
        }
    }
}

/// Buffered HTTP/2 connection wrapper, matching Python's BufferedH2Connection
/// This wraps h2 server/client connection and adds internal send buffers
#[derive(Debug)]
pub struct BufferedH2Connection {
    // We'll store connection state and handle it manually to match Python's approach
    stream_buffers: HashMap<u32, VecDeque<SendH2Data>>,
    stream_trailers: HashMap<u32, Vec<(Bytes, Bytes)>>,
    max_frame_size: u32,
    initial_window_size: u32,
}

/// Data to be sent on an HTTP/2 stream
#[derive(Debug, Clone)]
pub struct SendH2Data {
    pub data: Bytes,
    pub end_stream: bool,
}

impl BufferedH2Connection {
    pub fn new() -> Self {
        Self {
            stream_buffers: HashMap::new(),
            stream_trailers: HashMap::new(),
            max_frame_size: 2_u32.pow(17), // 128KB, matching Python
            initial_window_size: 2_u32.pow(31) - 1, // Max window size, matching Python
        }
    }

    /// Receive data and return events, matching Python's receive_data method
    /// This converts raw bytes to H2Event enum, avoiding h2::frame usage
    pub fn receive_data(&mut self, data: &[u8]) -> Result<Vec<H2Event>, ProxyError> {
        // For now, return a placeholder event indicating we need to implement
        // proper HTTP/2 frame parsing using the h2 library's non-frame API
        let mut h2_events = Vec::new();

        if !data.is_empty() {
            h2_events.push(H2Event::ProtocolError {
                message: "BufferedH2Connection.receive_data not fully implemented - needs h2 integration".to_string(),
            });
        }

        Ok(h2_events)
    }

    /// Send data on a stream, with buffering like Python implementation
    pub fn send_data(&mut self, stream_id: u32, data: Bytes, end_stream: bool) -> Result<(), ProxyError> {
        let frame_size = data.len();

        // Check frame size limit
        if frame_size > self.max_frame_size as usize {
            // Split large frames
            let max_size = self.max_frame_size as usize;
            for chunk in data.chunks(max_size) {
                let is_last_chunk = chunk.as_ptr() == data[data.len() - chunk.len()..].as_ptr();
                self.send_data(stream_id, Bytes::copy_from_slice(chunk), end_stream && is_last_chunk)?;
            }
            return Ok(());
        }

        // Check if we have buffered data for this stream
        if self.stream_buffers.contains_key(&stream_id) {
            // Append to buffer
            self.stream_buffers
                .entry(stream_id)
                .or_insert_with(VecDeque::new)
                .push_back(SendH2Data { data, end_stream });
        } else {
            // For now, always buffer the data until we implement flow control
            let mut buffer = VecDeque::new();
            buffer.push_back(SendH2Data { data, end_stream });
            self.stream_buffers.insert(stream_id, buffer);
        }

        Ok(())
    }

    /// Get data to send to the network
    pub fn data_to_send(&mut self) -> Option<Bytes> {
        // TODO: Implement proper data serialization from buffered streams
        None
    }

    /// Check if stream has buffered data
    pub fn has_buffered_data(&self, stream_id: u32) -> bool {
        self.stream_buffers.get(&stream_id).map_or(false, |buf| !buf.is_empty())
    }

    /// Process buffered data for a stream when window updates occur
    pub fn stream_window_updated(&mut self, stream_id: u32) -> bool {
        // TODO: Implement window update processing like Python version
        false
    }
}

/// HTTP/2 connection base class, matching Python's Http2Connection
#[derive(Debug)]
pub struct Http2Connection {
    pub context: Context,
    pub conn: Arc<Connection>,
    pub h2_conn: BufferedH2Connection,
    pub streams: HashMap<StreamId, Http2StreamState>,
    pub debug: bool,
    pub config: Http2Config,
}

impl Http2Connection {
    pub fn new(context: Context, conn: Arc<Connection>, config: Http2Config) -> Self {
        // Create the buffered H2 connection wrapper
        let h2_conn = BufferedH2Connection::new();

        Self {
            context,
            conn,
            h2_conn,
            streams: HashMap::new(),
            debug: false, // TODO: Get from context options
            config,
        }
    }

    /// Check if a stream is closed, matching Python's is_closed method
    pub fn is_closed(&self, stream_id: StreamId) -> bool {
        // TODO: Implement proper stream state checking with h2 library
        !self.streams.contains_key(&stream_id)
    }

    /// Check if we can write to a stream, matching Python's is_open_for_us method
    pub fn is_open_for_us(&self, stream_id: StreamId) -> bool {
        // TODO: Implement proper stream state checking with h2 library
        self.streams.contains_key(&stream_id) &&
        self.streams[&stream_id] == Http2StreamState::HeadersReceived
    }

    /// Handle HTTP/2 events, matching Python's handle_h2_event method
    /// Returns CommandGenerator<bool> where true means stop further processing
    pub fn handle_h2_event(&mut self, event: H2Event) -> Box<dyn crate::proxy::layer::CommandGenerator<bool>> {
        match event {
            H2Event::DataReceived { stream_id, data, end_stream } => {
                self.handle_data_received(stream_id, data, end_stream)
            }
            H2Event::HeadersReceived { stream_id, headers, end_stream } => {
                self.handle_headers_received(stream_id, headers, end_stream)
            }
            H2Event::StreamReset { stream_id, error_code } => {
                self.handle_stream_reset(stream_id, error_code)
            }
            H2Event::SettingsChanged => {
                self.handle_settings_changed()
            }
            H2Event::GoAway { error_code, last_stream_id } => {
                self.handle_go_away(error_code, last_stream_id)
            }
            H2Event::WindowUpdate { stream_id } => {
                self.handle_window_update(stream_id)
            }
            H2Event::Ping { ack, data } => {
                self.handle_ping(ack, data)
            }
            H2Event::ProtocolError { message } => {
                self.handle_protocol_error_event(message)
            }
            H2Event::ConnectionTerminated { error_code, last_stream_id } => {
                self.handle_connection_terminated(error_code, last_stream_id)
            }
        }
    }

    fn handle_data_received(&mut self, stream_id: u32, data: Bytes, end_stream: bool) -> Box<dyn crate::proxy::layer::CommandGenerator<bool>> {
        let stream_id = stream_id as StreamId;

        // Check if stream exists
        let state = self.streams.get(&stream_id);
        if state.is_none() {
            return self.protocol_error_generator(
                format!("Received data frame for unknown stream {}", stream_id)
            );
        }

        // Check if we're expecting headers instead of data
        if *state.unwrap() == Http2StreamState::ExpectingHeaders {
            return self.protocol_error_generator(
                "Received HTTP/2 data frame, expected headers.".to_string()
            );
        }

        // Handle empty end-of-stream data frames (just flow control)
        let is_empty_eos_data_frame = end_stream && data.is_empty();
        if is_empty_eos_data_frame {
            // TODO: Acknowledge received data for flow control
            return Box::new(crate::proxy::layer::BooleanCommandGenerator::with_result(false));
        }

        // Send data to stream
        let commands = vec![
            Box::new(ReceiveHttp {
                event: Box::new(RequestData {
                    stream_id,
                    data,
                }),
            }) as Box<dyn Command>
        ];

        Box::new(crate::proxy::layer::BooleanCommandGenerator::new(commands, false))
    }

    fn handle_headers_received(&mut self, stream_id: u32, headers: Vec<(Bytes, Bytes)>, end_stream: bool) -> Box<dyn crate::proxy::layer::CommandGenerator<bool>> {
        let stream_id = stream_id as StreamId;

        // Parse headers into pseudo-headers and regular headers
        let result = self.parse_h2_headers_from_vec(headers);
        let (regular_headers, pseudo_headers) = match result {
            Ok(parsed) => parsed,
            Err(e) => {
                return self.protocol_error_generator(format!("Failed to parse headers: {}", e));
            }
        };

        // Create HTTP request from headers
        let request = match self.create_request_from_headers(pseudo_headers, regular_headers) {
            Ok(req) => req,
            Err(e) => {
                return self.protocol_error_generator(format!("Failed to create request: {}", e));
            }
        };

        self.streams.insert(stream_id, Http2StreamState::HeadersReceived);

        let commands = vec![
            Box::new(ReceiveHttp {
                event: Box::new(RequestHeaders {
                    stream_id,
                    request,
                    end_stream,
                    replay_flow: None,
                }),
            }) as Box<dyn Command>
        ];

        Box::new(crate::proxy::layer::BooleanCommandGenerator::new(commands, false))
    }

    fn handle_stream_reset(&mut self, stream_id: u32, error_code: u32) -> Box<dyn CommandGenerator<()>> {
        let stream_id = stream_id as StreamId;

        if !self.streams.contains_key(&stream_id) {
            // We don't track priority frames which could be followed by a stream reset
            return Box::new(SimpleCommandGenerator::new(vec![]));
        }

        self.streams.remove(&stream_id);

        let err_code = match error_code {
            0x8 => ErrorCode::Cancel, // CANCEL
            0xD => ErrorCode::Http11Required, // HTTP_1_1_REQUIRED
            _ => ErrorCode::GenericClientError,
        };

        let err_str = format!("0x{:x}", error_code);

        let commands = vec![
            Box::new(ReceiveHttp {
                event: Box::new(RequestProtocolError {
                    stream_id,
                    message: format!("stream reset by client ({})", err_str),
                    code: err_code,
                }),
            }) as Box<dyn Command>
        ];
        Box::new(SimpleCommandGenerator::new(commands))
    }

    fn handle_settings_changed(&mut self) -> Box<dyn CommandGenerator<()>> {
        // Settings frames are handled automatically by the h2 library
        Box::new(SimpleCommandGenerator::new(vec![]))
    }

    fn handle_go_away(&mut self, error_code: u32, last_stream_id: u32) -> Box<dyn CommandGenerator<()>> {
        // Close all streams >= last_stream_id
        let streams_to_close: Vec<StreamId> = self.streams.keys()
            .filter(|&&id| id >= last_stream_id as StreamId)
            .cloned()
            .collect();

        let mut commands = Vec::new();
        for stream_id in streams_to_close {
            self.streams.remove(&stream_id);
            commands.push(Box::new(ReceiveHttp {
                event: Box::new(RequestProtocolError {
                    stream_id,
                    message: format!("HTTP/2 connection closed: 0x{:x}", error_code),
                    code: ErrorCode::GenericClientError,
                }),
            }) as Box<dyn Command>);
        }

        commands.push(Box::new(CloseConnection {
            connection: self.conn.clone(),
        }) as Box<dyn Command>);

        Box::new(SimpleCommandGenerator::new(commands))
    }

    fn handle_window_update(&mut self, stream_id: u32) -> Box<dyn CommandGenerator<()>> {
        // Window update frames are handled automatically by the h2 library
        Box::new(SimpleCommandGenerator::new(vec![]))
    }

    fn handle_ping(&mut self, ack: bool, data: [u8; 8]) -> Box<dyn CommandGenerator<()>> {
        // Ping frames are handled automatically by the h2 library
        Box::new(SimpleCommandGenerator::new(vec![]))
    }


    fn parse_h2_headers_from_vec(&self, headers: Vec<(Bytes, Bytes)>) -> Result<(http::HeaderMap, HashMap<String, String>), ProxyError> {
        let mut header_map = http::HeaderMap::new();
        let mut pseudo_headers = HashMap::new();

        for (name_bytes, value_bytes) in headers {
            let name_str = std::str::from_utf8(&name_bytes)
                .map_err(|_| ProxyError::Protocol("Invalid header name encoding".to_string()))?;

            if name_str.starts_with(':') {
                if pseudo_headers.contains_key(name_str) {
                    return Err(ProxyError::Protocol(format!("Duplicate HTTP/2 pseudo header: {}", name_str)));
                }
                let value_str = std::str::from_utf8(&value_bytes)
                    .map_err(|_| ProxyError::Protocol("Invalid pseudo header value encoding".to_string()))?;
                pseudo_headers.insert(name_str.to_string(), value_str.to_string());
            } else {
                let name = http::HeaderName::from_bytes(&name_bytes)
                    .map_err(|_| ProxyError::Protocol(format!("Invalid header name: {}", name_str)))?;
                let value = http::HeaderValue::from_bytes(&value_bytes)
                    .map_err(|_| ProxyError::Protocol("Invalid header value".to_string()))?;
                header_map.insert(name, value);
            }
        }

        Ok((header_map, pseudo_headers))
    }

    fn create_request_from_headers(&self, pseudo_headers: HashMap<String, String>, headers: http::HeaderMap) -> Result<HTTPRequest, ProxyError> {
        let method = pseudo_headers.get(":method")
            .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :method".to_string()))?;
        let scheme = pseudo_headers.get(":scheme")
            .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :scheme".to_string()))?;
        let path = pseudo_headers.get(":path")
            .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :path".to_string()))?;
        let authority = pseudo_headers.get(":authority");

        if !pseudo_headers.is_empty() {
            return Err(ProxyError::Protocol(format!("Unknown pseudo headers: {:?}", pseudo_headers.keys())));
        }

        let (host, port) = if let Some(auth) = authority {
            parse_authority(auth, true)
                .map_err(|e| ProxyError::Protocol(format!("Invalid authority: {}", e)))?
        } else {
            ("".to_string(), 0)
        };

        // Build URL
        let url_str = if !host.is_empty() && port != 0 {
            format!("{}://{}:{}{}", scheme, host, port, path)
        } else if !host.is_empty() {
            format!("{}://{}{}", scheme, host, path)
        } else {
            format!("{}://{}", scheme, path)
        };

        let url = url::Url::parse(&url_str)
            .map_err(|e| ProxyError::Protocol(format!("Invalid URL: {}", e)))?;

        // Convert headers
        let mut header_map = std::collections::HashMap::new();
        for (name, value) in headers.iter() {
            if !name.as_str().starts_with(':') {
                header_map.insert(
                    name.as_str().to_lowercase(),
                    value.to_str().map_err(|_| ProxyError::Protocol("Invalid header value".to_string()))?.to_string()
                );
            }
        }

        Ok(HTTPRequest {
            method: method.clone(),
            url,
            version: "HTTP/2.0".to_string(),
            headers: header_map,
            content: Vec::new(),
            timestamp_start: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            timestamp_end: None,
        })
    }

    /// Send HTTP/2 frame data, matching Python's data_to_send method
    pub fn data_to_send(&mut self) -> Option<Bytes> {
        // TODO: Implement proper data sending with h2 library
        None
    }

    /// Close connection with error, matching Python's protocol_error method
    pub async fn protocol_error(&mut self, message: String, error_code: Option<h2::Reason>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        warn!("HTTP/2 protocol error: {}", message);

        // Send GOAWAY frame
        // TODO: Implement GOAWAY sending with h2 library

        Ok(vec![
            Box::new(Log {
                message: format!("HTTP/2 protocol error: {}", message),
                level: LogLevel::Error,
            }),
            Box::new(CloseConnection {
                connection: self.conn.clone(),
            }),
        ])
    }

    /// Protocol error handler that returns a CommandGenerator like Python
    pub fn protocol_error_generator(&mut self, message: String) -> Box<dyn crate::proxy::layer::CommandGenerator<bool>> {
        warn!("HTTP/2 protocol error: {}", message);

        let commands = vec![
            Box::new(Log {
                message: format!("HTTP/2 protocol error: {}", message),
                level: LogLevel::Error,
            }) as Box<dyn Command>,
            Box::new(CloseConnection {
                connection: self.conn.clone(),
            }) as Box<dyn Command>,
        ];

        // Return true to indicate processing should stop
        Box::new(crate::proxy::layer::BooleanCommandGenerator::new(commands, true))
    }

    /// Close connection, matching Python's close_connection method
    pub async fn close_connection(&mut self, msg: String) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let mut commands = vec![
            Box::new(CloseConnection {
                connection: self.conn.clone(),
            })
        ];

        // Send protocol errors for all active streams
        for stream_id in self.streams.keys().cloned().collect::<Vec<_>>() {
            commands.push(Box::new(ReceiveHttp {
                event: Box::new(RequestProtocolError {
                    stream_id,
                    message: msg.clone(),
                    code: ErrorCode::GenericClientError,
                }),
            }) as Box<dyn Command>);
        }

        self.streams.clear();
        Ok(commands)
    }
}

/// HTTP/2 Server implementation, matching Python's Http2Server
#[derive(Debug)]
pub struct Http2Server {
    pub base: Http2Connection,
    pub receive_protocol_error: fn(StreamId, String, ErrorCode) -> Box<dyn HttpEvent>,
    pub receive_data: fn(StreamId, Bytes) -> Box<dyn HttpEvent>,
    pub receive_trailers: fn(StreamId, http::HeaderMap) -> Box<dyn HttpEvent>,
    pub receive_end_of_message: fn(StreamId) -> Box<dyn HttpEvent>,
}

impl Http2Server {
    pub fn new(context: Context) -> Self {
        let config = Http2Config::default();
        let base = Http2Connection::new(context, Arc::new(Connection::default()), config);

        Self {
            base,
            receive_protocol_error: |stream_id, message, code| Box::new(RequestProtocolError { stream_id, message, code }),
            receive_data: |stream_id, data| Box::new(RequestData { stream_id, data }),
            receive_trailers: |stream_id, trailers| Box::new(RequestTrailers { stream_id, trailers }),
            receive_end_of_message: |stream_id| Box::new(RequestEndOfMessage { stream_id }),
        }
    }

    /// Handle HTTP/2 request received event, matching Python's handle_h2_event for RequestReceived
    pub async fn handle_request_received(&mut self, headers: Vec<(Bytes, Bytes)>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let (host, port, method, scheme, authority, path, headers) = parse_h2_request_headers(headers)?;

        let request = http::Request {
            host,
            port,
            method: method.to_string(),
            scheme: scheme.to_string(),
            authority: authority.to_string(),
            path: path.to_string(),
            http_version: b"HTTP/2.0".to_vec(),
            headers,
            content: None,
            trailers: None,
            timestamp_start: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
            timestamp_end: None,
        };

        // TODO: Get next available stream ID
        let stream_id = 1;
        self.base.streams.insert(stream_id, Http2StreamState::HeadersReceived);

        Ok(vec![
            Box::new(ReceiveHttp {
                event: Box::new(RequestHeaders {
                    stream_id,
                    request,
                    end_stream: false, // TODO: Determine from headers
                    replay_flow: None,
                }),
            }) as Box<dyn Command>
        ])
    }

    /// Handle HTTP/2 informational response, matching Python's handle_h2_event for InformationalResponseReceived
    pub async fn handle_informational_response(&mut self, headers: Vec<(Bytes, Bytes)>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // HTTP/2 informational responses are swallowed (not forwarded)
        let pseudo_headers = split_pseudo_headers(headers)?;
        let status = pseudo_headers.get(":status")
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        let reason = match status {
            100 => "Continue",
            101 => "Switching Protocols",
            102 => "Processing",
            103 => "Early Hints",
            _ => "Unknown",
        };

        Ok(vec![
            Box::new(Log {
                message: format!("Swallowing HTTP/2 informational response: {} {}", status, reason),
                level: LogLevel::Info,
            })
        ])
    }

    /// Handle HTTP/2 request from server (protocol error), matching Python's handle_h2_event for RequestReceived on client
    pub async fn handle_request_from_server(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        self.base.protocol_error(
            "HTTP/2 protocol error: received request from server".to_string(),
            Some(h2::Reason::PROTOCOL_ERROR)
        ).await
    }
}

impl Layer for Http2Server {
    async fn handle_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // TODO: Implement event handling matching Python's _handle_event
        match event.as_ref() {
            _ if event.downcast_ref::<Start>().is_some() => {
                // Initiate HTTP/2 connection
                if let Some(data) = self.base.data_to_send() {
                    Ok(vec![
                        Box::new(SendData {
                            connection: self.base.conn.clone(),
                            data,
                        })
                    ])
                } else {
                    Ok(vec![])
                }
            }
            _ if event.downcast_ref::<DataReceived>().is_some() => {
                // TODO: Parse HTTP/2 frames and handle events
                Ok(vec![])
            }
            _ => {
                // Handle HTTP events for sending responses
                if let Some(http_event) = event.downcast_ref::<ResponseHeaders>() {
                    self.handle_response_headers(http_event.clone()).await
                } else if let Some(http_event) = event.downcast_ref::<ResponseData>() {
                    self.handle_response_data(http_event.clone()).await
                } else if let Some(http_event) = event.downcast_ref::<ResponseEndOfMessage>() {
                    self.handle_response_end(http_event.clone()).await
                } else if let Some(http_event) = event.downcast_ref::<ResponseProtocolError>() {
                    self.handle_response_error(http_event.clone()).await
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

impl Http2Server {
    async fn handle_response_headers(&mut self, event: ResponseHeaders) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if !self.base.is_open_for_us(event.stream_id) {
            return Ok(vec![]);
        }

        let headers = format_h2_response_headers(&self.base.context, &event)?;
        // TODO: Send headers using h2 library

        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }

    async fn handle_response_data(&mut self, event: ResponseData) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if !self.base.is_open_for_us(event.stream_id) {
            return Ok(vec![]);
        }

        // TODO: Send data using h2 library
        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }

    async fn handle_response_end(&mut self, event: ResponseEndOfMessage) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if !self.base.is_open_for_us(event.stream_id) {
            return Ok(vec![]);
        }

        // TODO: End stream using h2 library
        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }

    async fn handle_response_error(&mut self, event: ResponseProtocolError) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if self.base.is_closed(event.stream_id) {
            return Ok(vec![]);
        }

        let status = event.code.http_status_code();
        if self.base.is_open_for_us(event.stream_id) && status.is_some() && !self.base.streams[&event.stream_id] == Http2StreamState::ExpectingHeaders {
            // Send error response headers
            // TODO: Send error headers using h2 library
        } else {
            let error_code = match event.code {
                ErrorCode::Cancel | ErrorCode::ClientDisconnected => h2::Reason::CANCEL,
                ErrorCode::Kill => h2::Reason::INTERNAL_ERROR,
                ErrorCode::Http11Required => h2::Reason::HTTP_1_1_REQUIRED,
                ErrorCode::PassthroughClose => h2::Reason::CANCEL,
                ErrorCode::GenericClientError | ErrorCode::GenericServerError |
                ErrorCode::RequestTooLarge | ErrorCode::ResponseTooLarge |
                ErrorCode::ConnectFailed | ErrorCode::DestinationUnknown |
                ErrorCode::RequestValidationFailed | ErrorCode::ResponseValidationFailed => h2::Reason::INTERNAL_ERROR,
                _ => h2::Reason::INTERNAL_ERROR,
            };

            // TODO: Reset stream using h2 library
        }

        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }
}

/// HTTP/2 Client implementation, matching Python's Http2Client
#[derive(Debug)]
pub struct Http2Client {
    pub base: Http2Connection,
    pub our_stream_id: HashMap<StreamId, u32>,
    pub their_stream_id: HashMap<u32, StreamId>,
    pub stream_queue: HashMap<StreamId, Vec<Box<dyn Event>>>,
    pub provisional_max_concurrency: Option<u32>,
    pub last_activity: f64,
    pub receive_protocol_error: fn(StreamId, String, ErrorCode) -> Box<dyn HttpEvent>,
    pub receive_data: fn(StreamId, Bytes) -> Box<dyn HttpEvent>,
    pub receive_trailers: fn(StreamId, http::HeaderMap) -> Box<dyn HttpEvent>,
    pub receive_end_of_message: fn(StreamId) -> Box<dyn HttpEvent>,
}

impl Http2Client {
    pub fn new(context: Context) -> Self {
        let config = Http2Config::default();
        let mut base = Http2Connection::new(context, Arc::new(Connection::default()), config);

        // Disable HTTP/2 push
        // TODO: Configure h2 connection to disable push

        Self {
            base,
            our_stream_id: HashMap::new(),
            their_stream_id: HashMap::new(),
            stream_queue: HashMap::new(),
            provisional_max_concurrency: Some(10),
            last_activity: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
            receive_protocol_error: |stream_id, message, code| Box::new(ResponseProtocolError { stream_id, message, code }),
            receive_data: |stream_id, data| Box::new(ResponseData { stream_id, data }),
            receive_trailers: |stream_id, trailers| Box::new(ResponseTrailers { stream_id, trailers }),
            receive_end_of_message: |stream_id| Box::new(ResponseEndOfMessage { stream_id }),
        }
    }

    /// Handle HTTP/2 response received event, matching Python's handle_h2_event for ResponseReceived
    pub async fn handle_response_received(&mut self, headers: Vec<(Bytes, Bytes)>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        let (status_code, headers) = parse_h2_response_headers(headers)?;

        let response = http::Response {
            http_version: b"HTTP/2.0".to_vec(),
            status_code,
            reason: b"".to_vec(),
            headers,
            content: None,
            trailers: None,
            timestamp_start: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
            timestamp_end: None,
        };

        // TODO: Get stream ID from h2 event
        let stream_id = 1;
        if self.base.streams.get(&stream_id) != Some(&Http2StreamState::ExpectingHeaders) {
            return self.base.protocol_error("Received unexpected HTTP/2 response.".to_string(), Some(h2::Reason::PROTOCOL_ERROR)).await;
        }

        self.base.streams.insert(stream_id, Http2StreamState::HeadersReceived);

        Ok(vec![
            Box::new(ReceiveHttp {
                event: Box::new(ResponseHeaders {
                    stream_id,
                    response,
                    end_stream: false, // TODO: Determine from headers
                }),
            }) as Box<dyn Command>
        ])
    }

    /// Handle HTTP/2 informational response, matching Python's handle_h2_event for InformationalResponseReceived
    pub async fn handle_informational_response(&mut self, headers: Vec<(Bytes, Bytes)>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // HTTP/2 informational responses are swallowed (not forwarded)
        let pseudo_headers = split_pseudo_headers(headers)?;
        let status = pseudo_headers.get(":status")
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        let reason = match status {
            100 => "Continue",
            101 => "Switching Protocols",
            102 => "Processing",
            103 => "Early Hints",
            _ => "Unknown",
        };

        Ok(vec![
            Box::new(Log {
                message: format!("Swallowing HTTP/2 informational response: {} {}", status, reason),
                level: LogLevel::Info,
            })
        ])
    }

    /// Handle HTTP/2 request from server (protocol error), matching Python's handle_h2_event for RequestReceived on client
    pub async fn handle_request_from_server(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        self.base.protocol_error(
            "HTTP/2 protocol error: received request from server".to_string(),
            Some(h2::Reason::PROTOCOL_ERROR)
        ).await
    }

    /// Handle remote settings changed, matching Python's handle_h2_event for RemoteSettingsChanged
    pub async fn handle_remote_settings_changed(&mut self) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // We have received at least one settings from now, can rely on max concurrency in remote_settings
        self.provisional_max_concurrency = None;
        Ok(vec![])
    }
}

impl Layer for Http2Client {
    async fn handle_event(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // TODO: Implement event handling matching Python's _handle_event and _handle_event2
        match event.as_ref() {
            _ if event.downcast_ref::<Start>().is_some() => {
                // TODO: Handle ping keepalive setup
                if let Some(data) = self.base.data_to_send() {
                    Ok(vec![
                        Box::new(SendData {
                            connection: self.base.conn.clone(),
                            data,
                        })
                    ])
                } else {
                    Ok(vec![])
                }
            }
            _ if event.downcast_ref::<Wakeup>().is_some() => {
                // TODO: Handle ping keepalive
                Ok(vec![])
            }
            _ if event.downcast_ref::<DataReceived>().is_some() => {
                // TODO: Parse HTTP/2 frames and handle events
                Ok(vec![])
            }
            _ => {
                // Handle HTTP events for sending requests
                if let Some(http_event) = event.downcast_ref::<RequestHeaders>() {
                    self.handle_request_headers(http_event.clone()).await
                } else if let Some(http_event) = event.downcast_ref::<RequestData>() {
                    self.handle_request_data(http_event.clone()).await
                } else if let Some(http_event) = event.downcast_ref::<RequestEndOfMessage>() {
                    self.handle_request_end(http_event.clone()).await
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

impl Http2Client {
    async fn handle_request_headers(&mut self, event: RequestHeaders) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        // Map stream IDs
        let ours = if let Some(ours) = self.our_stream_id.get(&event.stream_id) {
            *ours
        } else {
            let no_free_streams = self.base.h2_conn.open_outbound_streams >=
                self.provisional_max_concurrency.unwrap_or(self.base.h2_conn.remote_settings().max_concurrent_streams as u32);

            if no_free_streams {
                self.stream_queue.entry(event.stream_id).or_insert_with(Vec::new).push(Box::new(event));
                return Ok(vec![]);
            }

            let ours = self.base.h2_conn.get_next_available_stream_id();
            self.our_stream_id.insert(event.stream_id, ours);
            self.their_stream_id.insert(ours, event.stream_id);
            ours
        };

        let headers = format_h2_request_headers(&self.base.context, &event)?;
        // TODO: Send headers using h2 library
        self.base.streams.insert(ours as StreamId, Http2StreamState::ExpectingHeaders);

        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }

    async fn handle_request_data(&mut self, event: RequestData) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if !self.base.is_open_for_us(event.stream_id) {
            return Ok(vec![]);
        }

        // TODO: Send data using h2 library
        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }

    async fn handle_request_end(&mut self, event: RequestEndOfMessage) -> Result<Vec<Box<dyn Command>>, ProxyError> {
        if !self.base.is_open_for_us(event.stream_id) {
            return Ok(vec![]);
        }

        // TODO: End stream using h2 library
        Ok(vec![
            Box::new(SendData {
                connection: self.base.conn.clone(),
                data: self.base.data_to_send().unwrap_or_default(),
            })
        ])
    }
}

/// Utility functions for HTTP/2 header parsing and formatting, matching Python implementations

/// Normalize HTTP/1.1 headers for HTTP/2, matching Python's normalize_h1_headers
pub fn normalize_h1_headers(headers: Vec<(Bytes, Bytes)>, is_client: bool) -> Result<Vec<(Bytes, Bytes)>, ProxyError> {
    // HTTP/1 servers commonly send capitalized headers, which isn't valid HTTP/2
    let mut normalized = Vec::new();

    for (name, value) in headers {
        let name_str = String::from_utf8_lossy(&name);
        if name_str.chars().any(|c| !c.is_ascii()) {
            return Err(ProxyError::Protocol("Header name contains non-ASCII characters".to_string()));
        }

        // Convert to lowercase for HTTP/2
        let normalized_name = name_str.to_lowercase().into_bytes();
        normalized.push((Bytes::from(normalized_name), value));
    }

    Ok(normalized)
}

/// Normalize HTTP/2 headers, matching Python's normalize_h2_headers
pub fn normalize_h2_headers(headers: &mut Vec<(Bytes, Bytes)>) -> Result<(), ProxyError> {
    for (name, _) in headers.iter_mut() {
        let name_str = String::from_utf8_lossy(name);
        if !name_str.is_ascii() || !name_str.chars().next().unwrap_or(' ').is_lowercase() {
            *name = Bytes::from(name_str.to_lowercase());
        }
    }
    Ok(())
}

/// Format HTTP/2 request headers, matching Python's format_h2_request_headers
pub fn format_h2_request_headers(context: &Context, event: &RequestHeaders) -> Result<Vec<(Bytes, Bytes)>, ProxyError> {
    let mut pseudo_headers = Vec::new();
    pseudo_headers.push((Bytes::from(":method"), Bytes::from(event.request.method.clone())));
    pseudo_headers.push((Bytes::from(":scheme"), Bytes::from(event.request.scheme.clone())));
    pseudo_headers.push((Bytes::from(":path"), Bytes::from(event.request.path.clone())));

    // Create authority from host and port
    let authority = if event.request.port == 80 && event.request.scheme == "http" ||
                       event.request.port == 443 && event.request.scheme == "https" {
        event.request.host.clone()
    } else {
        format!("{}:{}", event.request.host, event.request.port)
    };
    pseudo_headers.push((Bytes::from(":authority"), Bytes::from(authority)));

    let mut headers = if event.request.http_version == "HTTP/2.0" || event.request.http_version == "HTTP/3.0" {
        let mut hdrs = event.request.headers.iter()
            .map(|(k, v)| (Bytes::from(k.clone()), Bytes::from(v.clone())))
            .collect::<Vec<_>>();
        if context.options.normalize_outbound_headers {
            normalize_h2_headers(&mut hdrs)?;
        }
        hdrs
    } else {
        // Host header should already be present in HTTP/1.1 requests
        normalize_h1_headers(
            event.request.headers.iter()
                .map(|(k, v)| (Bytes::from(k.clone()), Bytes::from(v.clone())))
                .collect(),
            true
        )?
    };

    Ok([pseudo_headers, headers].concat())
}

/// Format HTTP/2 response headers, matching Python's format_h2_response_headers
pub fn format_h2_response_headers(context: &Context, event: &ResponseHeaders) -> Result<Vec<(Bytes, Bytes)>, ProxyError> {
    let mut headers = vec![
        (Bytes::from(":status"), Bytes::from(format!("{}", event.response.status_code))),
    ];

    let mut header_fields = event.response.headers.iter()
        .map(|(k, v)| (Bytes::from(k.clone()), Bytes::from(v.clone())))
        .collect::<Vec<_>>();
    if event.response.http_version == "HTTP/2.0" || event.response.http_version == "HTTP/3.0" {
        if context.options.normalize_outbound_headers {
            normalize_h2_headers(&mut header_fields)?;
        }
    } else {
        header_fields = normalize_h1_headers(header_fields, false)?;
    }

    headers.extend(header_fields);
    Ok(headers)
}

/// Parse HTTP/2 request headers, matching Python's parse_h2_request_headers
pub fn parse_h2_request_headers(h2_headers: Vec<(Bytes, Bytes)>) -> Result<(String, u16, Bytes, Bytes, Bytes, Bytes, http::HeaderMap), ProxyError> {
    let (pseudo_headers, headers) = split_pseudo_headers(h2_headers)?;

    let method = pseudo_headers.get(":method")
        .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :method".to_string()))?;
    let scheme = pseudo_headers.get(":scheme")
        .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :scheme".to_string()))?;
    let path = pseudo_headers.get(":path")
        .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :path".to_string()))?;
    let authority = pseudo_headers.get(":authority")
        .map(|s| s.clone())
        .unwrap_or_else(|| Bytes::new());

    if !pseudo_headers.is_empty() {
        return Err(ProxyError::Protocol(format!("Unknown pseudo headers: {:?}", pseudo_headers.keys())));
    }

    let (host, port) = if !authority.is_empty() {
        parse_authority(&String::from_utf8_lossy(&authority), true)
            .map_err(|e| ProxyError::Protocol(format!("Invalid authority: {}", e)))?
    } else {
        ("".to_string(), 0)
    };

    Ok((host, port, Bytes::from(method.clone()), Bytes::from(scheme.clone()), authority, Bytes::from(path.clone()), headers))
}

/// Parse HTTP/2 response headers, matching Python's parse_h2_response_headers
pub fn parse_h2_response_headers(h2_headers: Vec<(Bytes, Bytes)>) -> Result<(u16, http::HeaderMap), ProxyError> {
    let (pseudo_headers, headers) = split_pseudo_headers(h2_headers)?;

    let status_code = pseudo_headers.get(":status")
        .ok_or_else(|| ProxyError::Protocol("Required pseudo header is missing: :status".to_string()))?
        .parse::<u16>()
        .map_err(|_| ProxyError::Protocol("Invalid status code".to_string()))?;

    if !pseudo_headers.is_empty() {
        return Err(ProxyError::Protocol(format!("Unknown pseudo headers: {:?}", pseudo_headers.keys())));
    }

    Ok((status_code, headers))
}

/// Split HTTP/2 pseudo-headers from actual headers, matching Python's split_pseudo_headers
pub fn split_pseudo_headers(h2_headers: Vec<(Bytes, Bytes)>) -> Result<(HashMap<String, Bytes>, http::HeaderMap), ProxyError> {
    let mut pseudo_headers = HashMap::new();
    let mut headers = http::HeaderMap::new();

    for (name, value) in h2_headers {
        let name_str = String::from_utf8_lossy(&name);
        if name_str.starts_with(':') {
            if pseudo_headers.contains_key(&name_str) {
                return Err(ProxyError::Protocol(format!("Duplicate HTTP/2 pseudo header: {}", name_str)));
            }
            pseudo_headers.insert(name_str, value);
        } else {
            headers.insert(
                name_str.parse::<http::HeaderName>()
                    .map_err(|_| ProxyError::Protocol("Invalid header name".to_string()))?,
                value.to_vec().as_slice()
            );
        }
    }

    Ok((pseudo_headers, headers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_status() {
        assert_eq!(ErrorCode::GenericClientError.http_status_code(), Some(400));
        assert_eq!(ErrorCode::ConnectFailed.http_status_code(), Some(502));
        assert_eq!(ErrorCode::Kill.http_status_code(), None);
    }

    #[test]
    fn test_receive_buffer() {
        let mut buf = ReceiveBuffer::new();
        buf.extend(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");

        let lines = buf.maybe_extract_lines().unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], b"GET / HTTP/1.1");
        assert_eq!(lines[1], b"Host: example.com");
    }

    #[test]
    fn test_format_error() {
        let error_html = format_error(404, "Page not found");
        let error_str = String::from_utf8(error_html).unwrap();
        assert!(error_str.contains("404 Not Found"));
        assert!(error_str.contains("Page not found"));
    }

    #[test]
    fn test_http1_server_creation() {
        let context = Context::default();
        let server = Http1Server::new(context);
        assert_eq!(server.stream_id, 1);
        assert_eq!(server.state, Http1ServerState::Start);
        assert!(!server.request_done);
        assert!(!server.response_done);
    }

    #[test]
    fn test_request_parsing() {
        let context = Context::default();
        let server = Http1Server::new(context);

        let lines = vec![
            b"GET /path HTTP/1.1".to_vec(),
            b"Host: example.com".to_vec(),
            b"User-Agent: test".to_vec(),
        ];

        let request = server.parse_request_head(&lines).unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.headers.get("host"), Some(&"example.com".to_string()));
        assert_eq!(request.headers.get("user-agent"), Some(&"test".to_string()));
    }
}
