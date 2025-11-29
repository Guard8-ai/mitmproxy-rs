//! Proxy server implementation
//! This mirrors the Python proxy server in mitmproxy/proxy/server.py

use crate::proxy::{Context, Layer, AnyEvent, Command};
use crate::connection::{Client, Server, Connection, ConnectionState, TransportProtocol};
use crate::config::Config;
use crate::flow::HTTPFlow;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, info, error};

/// Main proxy server that handles incoming connections
#[derive(Debug)]
pub struct ProxyServer {
    config: Arc<Config>,
    connections: HashMap<String, Box<dyn Layer>>,
    /// Flow storage for API access
    flows: RwLock<HashMap<String, HTTPFlow>>,
}

impl ProxyServer {
    /// Create a new proxy server
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            connections: HashMap::new(),
            flows: RwLock::new(HashMap::new()),
        }
    }

    /// Get all flows
    pub async fn get_flows(&self) -> Vec<HTTPFlow> {
        let flows = self.flows.read().await;
        flows.values().cloned().collect()
    }

    /// Get a specific flow by ID
    pub async fn get_flow(&self, id: &str) -> Option<HTTPFlow> {
        let flows = self.flows.read().await;
        flows.get(id).cloned()
    }

    /// Update a flow
    pub async fn update_flow(&self, flow: HTTPFlow) -> bool {
        let mut flows = self.flows.write().await;
        let id = flow.flow.id.clone();
        if flows.contains_key(&id) {
            flows.insert(id, flow);
            true
        } else {
            false
        }
    }

    /// Add a new flow
    pub async fn add_flow(&self, flow: HTTPFlow) {
        let mut flows = self.flows.write().await;
        flows.insert(flow.flow.id.clone(), flow);
    }

    /// Remove a flow by ID
    pub async fn remove_flow(&self, id: &str) -> bool {
        let mut flows = self.flows.write().await;
        flows.remove(id).is_some()
    }

    /// Clear all flows
    pub async fn clear_flows(&self) {
        let mut flows = self.flows.write().await;
        flows.clear();
    }

    /// Run the proxy server (alternative entry point)
    pub async fn run(&self) -> crate::Result<()> {
        let addr = format!("{}:{}", self.config.proxy_host, self.config.proxy_port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Proxy server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New connection from {}", addr);
                    let config = self.config.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, addr.into(), config).await {
                            error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    /// Start the proxy server
    pub async fn start(&mut self) -> crate::Result<()> {
        let addr = format!("{}:{}", self.config.proxy_host, self.config.proxy_port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Proxy server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New connection from {}", addr);
                    // Handle connection in a separate task
                    let config = self.config.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, addr.into(), config).await {
                            error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    /// Handle a single connection
    async fn handle_connection(
        stream: TcpStream,
        addr: std::net::SocketAddr,
        config: Arc<Config>,
    ) -> crate::Result<()> {
        // Create client connection using the connection module's types
        let mut connection = Connection::new(TransportProtocol::Tcp);
        connection.peername = Some(addr);
        connection.timestamp_start = Some(std::time::SystemTime::now());
        connection.timestamp_tcp_setup = Some(std::time::SystemTime::now());

        let client = Client {
            connection,
            proxy_mode: None,
        };

        // Create context
        let context = Context::new(client, config);

        // Create root layer (NextLayer)
        let mut root_layer = crate::proxy::NextLayer::new(context);

        // Start processing
        let start_event = AnyEvent::Start(crate::proxy::events::Start);
        let mut generator = root_layer.handle_event(start_event);

        // Process commands from the generator
        while let Some(command) = generator.next_command() {
            debug!("Processing command: {:?}", command.command_name());
            // TODO: Implement command processing
        }

        Ok(())
    }
}