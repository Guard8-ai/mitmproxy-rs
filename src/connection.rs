//! Connection types and states matching mitmproxy's connection model

use std::net::SocketAddr;
use std::time::SystemTime;

/// Address type matching Python mitmproxy's Address
pub type Address = (String, u16);

/// Connection state flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionState {
    pub can_read: bool,
    pub can_write: bool,
}

impl ConnectionState {
    pub const OPEN: Self = Self {
        can_read: true,
        can_write: true,
    };

    pub const CLOSED: Self = Self {
        can_read: false,
        can_write: false,
    };

    pub const CAN_READ: Self = Self {
        can_read: true,
        can_write: false,
    };

    pub const CAN_WRITE: Self = Self {
        can_read: false,
        can_write: true,
    };
}

/// Transport protocol type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

/// TLS version
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsVersion {
    TLSv1,
    TLSv1_1,
    TLSv1_2,
    TLSv1_3,
}

/// Base connection type
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub transport_protocol: TransportProtocol,
    pub peername: Option<SocketAddr>,
    pub sockname: Option<SocketAddr>,
    pub state: ConnectionState,
    pub timestamp_start: Option<SystemTime>,
    pub timestamp_end: Option<SystemTime>,
    pub timestamp_tcp_setup: Option<SystemTime>,
    pub timestamp_tls_setup: Option<SystemTime>,
    pub error: Option<String>,
    pub tls: bool,
    pub tls_version: Option<TlsVersion>,
    pub cipher: Option<String>,
    pub sni: Option<String>,
    pub alpn: Option<String>,
}

impl Connection {
    pub fn new(transport_protocol: TransportProtocol) -> Self {
        Self {
            transport_protocol,
            peername: None,
            sockname: None,
            state: ConnectionState::OPEN,
            timestamp_start: Some(SystemTime::now()),
            timestamp_end: None,
            timestamp_tcp_setup: None,
            timestamp_tls_setup: None,
            error: None,
            tls: false,
            tls_version: None,
            cipher: None,
            sni: None,
            alpn: None,
        }
    }
}

impl Default for Connection {
    fn default() -> Self {
        Self::new(TransportProtocol::Tcp)
    }
}

/// Client connection with proxy mode
#[derive(Debug, Clone, PartialEq)]
pub struct Client {
    pub connection: Connection,
    pub proxy_mode: Option<String>,
}

impl Client {
    pub fn new(transport_protocol: TransportProtocol) -> Self {
        Self {
            connection: Connection::new(transport_protocol),
            proxy_mode: None,
        }
    }
}

/// Server connection with address
#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub connection: Connection,
    pub address: Option<SocketAddr>,
}

impl Server {
    pub fn new(transport_protocol: TransportProtocol) -> Self {
        Self {
            connection: Connection::new(transport_protocol),
            address: None,
        }
    }

    pub fn with_address(transport_protocol: TransportProtocol, address: SocketAddr) -> Self {
        Self {
            connection: Connection::new(transport_protocol),
            address: Some(address),
        }
    }
}