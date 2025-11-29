use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TLS error: {0}")]
    Tls(#[from] openssl::error::ErrorStack),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Certificate error: {0}")]
    Certificate(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Filter error: {0}")]
    Filter(String),

    #[error("Flow not found: {0}")]
    FlowNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Proxy error: {0}")]
    Proxy(String),

    #[error("{0}")]
    Other(String),
}

/// Alias for proxy-specific errors
pub type ProxyError = Error;

impl Error {
    pub fn certificate<T: fmt::Display>(msg: T) -> Self {
        Error::Certificate(msg.to_string())
    }

    pub fn auth<T: fmt::Display>(msg: T) -> Self {
        Error::Auth(msg.to_string())
    }

    pub fn filter<T: fmt::Display>(msg: T) -> Self {
        Error::Filter(msg.to_string())
    }

    pub fn flow_not_found<T: fmt::Display>(id: T) -> Self {
        Error::FlowNotFound(id.to_string())
    }

    pub fn invalid_request<T: fmt::Display>(msg: T) -> Self {
        Error::InvalidRequest(msg.to_string())
    }

    pub fn internal<T: fmt::Display>(msg: T) -> Self {
        Error::Internal(msg.to_string())
    }
}