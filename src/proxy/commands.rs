//! Commands that layers can emit to communicate with higher layers

use crate::connection::{Connection, Server};
use openssl::ssl::SslStream;
use std::fmt::Debug;

/// Base trait for all commands
pub trait Command: Debug + Send + Sync {
    fn command_name(&self) -> &'static str;
    fn is_blocking(&self) -> bool {
        false
    }
}

/// Request a wakeup event after the specified delay
#[derive(Debug, Clone)]
pub struct RequestWakeup {
    pub delay: f64,
}

impl Command for RequestWakeup {
    fn command_name(&self) -> &'static str {
        "RequestWakeup"
    }
}

/// Commands involving a specific connection
pub trait ConnectionCommand: Command {
    fn connection(&self) -> &Connection;
}

/// Send data to a remote peer
#[derive(Debug, Clone)]
pub struct SendData {
    pub connection: Connection,
    pub data: Vec<u8>,
}

impl Command for SendData {
    fn command_name(&self) -> &'static str {
        "SendData"
    }
}

impl ConnectionCommand for SendData {
    fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Open a new connection
#[derive(Debug, Clone)]
pub struct OpenConnection {
    pub connection: Server,
}

impl Command for OpenConnection {
    fn command_name(&self) -> &'static str {
        "OpenConnection"
    }

    fn is_blocking(&self) -> bool {
        true
    }
}

impl ConnectionCommand for OpenConnection {
    fn connection(&self) -> &Connection {
        &self.connection.connection
    }
}

/// Close a connection
#[derive(Debug, Clone)]
pub struct CloseConnection {
    pub connection: Connection,
}

impl Command for CloseConnection {
    fn command_name(&self) -> &'static str {
        "CloseConnection"
    }
}

impl ConnectionCommand for CloseConnection {
    fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Close a TCP connection (with half-close option)
#[derive(Debug, Clone)]
pub struct CloseTcpConnection {
    pub connection: Connection,
    pub half_close: bool,
}

impl Command for CloseTcpConnection {
    fn command_name(&self) -> &'static str {
        "CloseTcpConnection"
    }
}

impl ConnectionCommand for CloseTcpConnection {
    fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Base trait for hook commands
pub trait StartHook: Command {
    fn hook_name(&self) -> &'static str;
    fn is_blocking_hook(&self) -> bool {
        false
    }
}

/// Log a message
#[derive(Debug, Clone)]
pub struct Log {
    pub message: String,
    pub level: LogLevel,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl Command for Log {
    fn command_name(&self) -> &'static str {
        "Log"
    }
}

// TLS-related data structures
/// TLS client hello data
#[derive(Debug, Clone)]
pub struct ClientHelloData {
    pub sni: Option<String>,
    pub alpn_protocols: Vec<String>,
    pub ignore_connection: bool,
    pub establish_server_tls_first: bool,
}

/// TLS connection data
#[derive(Debug)]
pub struct TlsData {
    pub connection: Connection,
    pub is_dtls: bool,
}

// TLS Hook Commands
/// TLS ClientHello hook
#[derive(Debug)]
pub struct TlsClienthelloHook {
    pub data: ClientHelloData,
}

impl Command for TlsClienthelloHook {
    fn command_name(&self) -> &'static str {
        "TlsClienthelloHook"
    }
}

impl StartHook for TlsClienthelloHook {
    fn hook_name(&self) -> &'static str {
        "tls_clienthello"
    }
}

/// TLS start client hook
#[derive(Debug)]
pub struct TlsStartClientHook {
    pub data: TlsData,
}

impl Command for TlsStartClientHook {
    fn command_name(&self) -> &'static str {
        "TlsStartClientHook"
    }
}

impl StartHook for TlsStartClientHook {
    fn hook_name(&self) -> &'static str {
        "tls_start_client"
    }
}

/// TLS start server hook
#[derive(Debug)]
pub struct TlsStartServerHook {
    pub data: TlsData,
}

impl Command for TlsStartServerHook {
    fn command_name(&self) -> &'static str {
        "TlsStartServerHook"
    }
}

impl StartHook for TlsStartServerHook {
    fn hook_name(&self) -> &'static str {
        "tls_start_server"
    }
}

/// TLS established client hook
#[derive(Debug)]
pub struct TlsEstablishedClientHook {
    pub data: TlsData,
}

impl Command for TlsEstablishedClientHook {
    fn command_name(&self) -> &'static str {
        "TlsEstablishedClientHook"
    }
}

impl StartHook for TlsEstablishedClientHook {
    fn hook_name(&self) -> &'static str {
        "tls_established_client"
    }
}

/// TLS established server hook
#[derive(Debug)]
pub struct TlsEstablishedServerHook {
    pub data: TlsData,
}

impl Command for TlsEstablishedServerHook {
    fn command_name(&self) -> &'static str {
        "TlsEstablishedServerHook"
    }
}

impl StartHook for TlsEstablishedServerHook {
    fn hook_name(&self) -> &'static str {
        "tls_established_server"
    }
}

/// TLS failed client hook
#[derive(Debug)]
pub struct TlsFailedClientHook {
    pub data: TlsData,
}

impl Command for TlsFailedClientHook {
    fn command_name(&self) -> &'static str {
        "TlsFailedClientHook"
    }
}

impl StartHook for TlsFailedClientHook {
    fn hook_name(&self) -> &'static str {
        "tls_failed_client"
    }
}

/// TLS failed server hook
#[derive(Debug)]
pub struct TlsFailedServerHook {
    pub data: TlsData,
}

impl Command for TlsFailedServerHook {
    fn command_name(&self) -> &'static str {
        "TlsFailedServerHook"
    }
}

impl StartHook for TlsFailedServerHook {
    fn hook_name(&self) -> &'static str {
        "tls_failed_server"
    }
}

// WebSocket Hook Commands
/// WebSocket connection start hook
#[derive(Debug)]
pub struct WebsocketStartHook {
    pub flow: crate::flow::Flow,
}

impl Command for WebsocketStartHook {
    fn command_name(&self) -> &'static str {
        "WebsocketStartHook"
    }
}

impl StartHook for WebsocketStartHook {
    fn hook_name(&self) -> &'static str {
        "websocket_start"
    }
}

/// WebSocket message hook
#[derive(Debug)]
pub struct WebsocketMessageHook {
    pub flow: crate::flow::Flow,
}

impl Command for WebsocketMessageHook {
    fn command_name(&self) -> &'static str {
        "WebsocketMessageHook"
    }
}

impl StartHook for WebsocketMessageHook {
    fn hook_name(&self) -> &'static str {
        "websocket_message"
    }
}

/// WebSocket connection end hook
#[derive(Debug)]
pub struct WebsocketEndHook {
    pub flow: crate::flow::Flow,
}

impl Command for WebsocketEndHook {
    fn command_name(&self) -> &'static str {
        "WebsocketEndHook"
    }
}

impl StartHook for WebsocketEndHook {
    fn hook_name(&self) -> &'static str {
        "websocket_end"
    }
}
