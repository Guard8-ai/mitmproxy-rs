use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

use crate::api;
use crate::config::Config;
use crate::proxy::ProxyServer;
use crate::{Error, Result};

pub struct MitmproxyServer {
    config: Config,
    proxy: Arc<ProxyServer>,
}

impl MitmproxyServer {
    pub async fn new(config: Config) -> Result<Self> {
        let proxy = Arc::new(ProxyServer::new(Arc::new(config.clone())));

        Ok(Self { config, proxy })
    }

    pub async fn run(self) -> Result<()> {
        info!("Starting mitmproxy-rs server");
        info!("Proxy listening on: {}", self.config.proxy_addr());
        info!("Web API listening on: {}", self.config.web_addr());

        // Start proxy server
        let proxy_handle = {
            let proxy = Arc::clone(&self.proxy);
            tokio::spawn(async move {
                if let Err(e) = proxy.run().await {
                    error!("Proxy server error: {}", e);
                }
            })
        };

        // Start web API server
        let web_handle = {
            let proxy = Arc::clone(&self.proxy);
            let web_addr = self.config.web_addr();
            tokio::spawn(async move {
                let app = api::create_router(proxy);
                let listener = tokio::net::TcpListener::bind(&web_addr)
                    .await
                    .expect("Failed to bind web server");

                info!("Web API server starting on {}", web_addr);

                if let Err(e) = axum::serve(listener, app).await {
                    error!("Web server error: {}", e);
                }
            })
        };

        // Wait for shutdown signal
        let shutdown_handle = tokio::spawn(async {
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            info!("Received shutdown signal");
        });

        // Wait for any task to complete
        tokio::select! {
            _ = proxy_handle => {
                info!("Proxy server shut down");
            }
            _ = web_handle => {
                info!("Web server shut down");
            }
            _ = shutdown_handle => {
                info!("Shutting down gracefully");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = Config::default();
        let server = MitmproxyServer::new(config).await;
        assert!(server.is_ok());
    }
}