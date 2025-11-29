//! Proxy server implementation
//! This mirrors the Python proxy server in mitmproxy/proxy/server.py

use crate::proxy::{Context, Layer, AnyEvent, Command};
use crate::connection::{Client, Server, Connection, ConnectionState};
use crate::config::Config;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use tracing::{debug, info, error};

/// Main proxy server that handles incoming connections
#[derive(Debug)]
pub struct ProxyServer {
    config: Arc<Config>,
    connections: HashMap<String, Box<dyn Layer>>,
}

impl ProxyServer {
    /// Create a new proxy server
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            connections: HashMap::new(),
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
        // Create client connection
        let client = Client {
            connection: Connection {
                id: uuid::Uuid::new_v4().to_string(),
                peername: Some((addr.ip().to_string(), addr.port())),
                sockname: None,
                address: Some((addr.ip().to_string(), addr.port())),
                state: ConnectionState::Open,
                timestamp_start: Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()),
                timestamp_end: None,
                tls_established: false,
                cert: None,
                sni: None,
                cipher: None,
                alpn: None,
                tls_version: None,
                timestamp_tcp_setup: None,
                timestamp_tls_setup: None,
                error: None,
                sockname_str: None,
                peername_str: None,
            },
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