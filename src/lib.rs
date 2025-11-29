pub mod api;
pub mod auth;
pub mod certs;
pub mod config;
pub mod connection;
pub mod error;
pub mod filter;
pub mod flow;
pub mod proxy;
pub mod server;
pub mod websocket;

pub use error::{Error, Result};
pub use flow::{Flow, HTTPFlow};
pub use proxy::ProxyServer;
pub use server::MitmproxyServer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}