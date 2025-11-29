//! Context that layers operate within
//! This mirrors the Python Context class in mitmproxy/proxy/context.py

use crate::config::Config;
use crate::connection::{Client, Server, Connection};
use std::sync::Arc;

/// Context provided to each layer containing connection and configuration state.
/// This mirrors the Python Context class behavior.
#[derive(Debug, Clone)]
pub struct Context {
    /// The client connection
    pub client: Client,
    /// The server connection (if established)
    pub server: Option<Server>,
    /// Configuration options
    pub options: ContextOptions,
    /// Stack of layers for debugging and context tracking
    pub layers: Vec<LayerRef>,
}

/// Options available to the context - mirrors Python options
#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// Enable proxy debug logging
    pub proxy_debug: bool,
    /// Maximum body size before limiting
    pub body_size_limit: Option<String>,
    /// Stream large bodies
    pub stream_large_bodies: Option<String>,
    /// Store streamed bodies
    pub store_streamed_bodies: bool,
    /// Validate inbound headers
    pub validate_inbound_headers: bool,
    /// Connection strategy (eager/lazy)
    pub connection_strategy: String,
    /// Keep host header in reverse proxy mode
    pub keep_host_header: bool,
    /// Enable WebSocket support
    pub websocket: bool,
    /// Enable raw TCP mode
    pub rawtcp: bool,
    /// Normalize outbound HTTP/2 headers
    pub normalize_outbound_headers: bool,
}

/// Reference to a layer in the stack
#[derive(Debug, Clone)]
pub struct LayerRef {
    pub name: String,
    pub id: usize,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            proxy_debug: false,
            body_size_limit: None,
            stream_large_bodies: None,
            store_streamed_bodies: true,
            validate_inbound_headers: true,
            connection_strategy: "eager".to_string(),
            keep_host_header: false,
            websocket: true,
            rawtcp: false,
            normalize_outbound_headers: false,
        }
    }
}

impl From<Arc<Config>> for ContextOptions {
    fn from(_config: Arc<Config>) -> Self {
        ContextOptions {
            proxy_debug: false, // TODO: read from config
            body_size_limit: None,
            stream_large_bodies: None,
            store_streamed_bodies: true,
            validate_inbound_headers: true,
            connection_strategy: "eager".to_string(),
            keep_host_header: false,
            websocket: true,
            rawtcp: false,
            normalize_outbound_headers: false,
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        use crate::connection::{Client, TransportProtocol};

        let default_client = Client::new(TransportProtocol::Tcp);

        Self {
            client: default_client,
            server: None,
            options: ContextOptions::default(),
            layers: Vec::new(),
        }
    }
}

impl Context {
    /// Create a new context with a client connection
    pub fn new(client: Client, options: Arc<Config>) -> Self {
        Self {
            client,
            server: None,
            options: options.into(),
            layers: Vec::new(),
        }
    }

    /// Set the server connection
    pub fn with_server(mut self, server: Server) -> Self {
        self.server = Some(server);
        self
    }

    /// Fork the context for a child layer
    pub fn fork(&self) -> Self {
        let forked = self.clone();
        // In Python mitmproxy, fork() creates a copy but maintains the same connections
        // The layers list is shared but can be modified independently
        forked
    }

    /// Add a layer reference to the context stack
    pub fn add_layer(&mut self, name: String) {
        let id = self.layers.len();
        self.layers.push(LayerRef { name, id });
    }

    /// Get the current layer depth
    pub fn layer_depth(&self) -> usize {
        self.layers.len()
    }

    /// Get the current layer name if any
    pub fn current_layer(&self) -> Option<&str> {
        self.layers.last().map(|l| l.name.as_str())
    }

    /// Get the server connection, panicking if not set
    pub fn server(&self) -> &Server {
        self.server.as_ref().expect("Server connection not set")
    }

    /// Get mutable server connection, panicking if not set
    pub fn server_mut(&mut self) -> &mut Server {
        self.server.as_mut().expect("Server connection not set")
    }

    /// Get the client connection for compatibility with code expecting client_conn field
    pub fn client_conn(&self) -> &Connection {
        &self.client.connection
    }

    /// Get the server connection for compatibility with code expecting server_conn field
    pub fn server_conn(&self) -> Option<&Connection> {
        self.server.as_ref().map(|s| &s.connection)
    }
}