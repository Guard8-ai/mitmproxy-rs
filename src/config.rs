use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub proxy_host: String,
    pub proxy_port: u16,
    pub web_host: String,
    pub web_port: u16,
    pub auth_enabled: bool,
    pub auth_token: Option<String>,
    pub cert_store_path: String,
    pub flows_store_path: String,
    pub max_flows: usize,
    pub ssl_insecure: bool,
    pub upstream_cert: bool,
    pub anticache: bool,
    pub anticomp: bool,
    pub showhost: bool,
    pub no_server: bool,
    pub mode: ProxyMode,
    pub upstream_server: Option<String>,
    pub listen_host: Option<String>,
    pub listen_port: Option<u16>,
    pub certs_path: String,
    pub confdir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    Regular,
    Transparent,
    Socks5,
    Reverse,
    Upstream,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            proxy_host: "127.0.0.1".to_string(),
            proxy_port: 8080,
            web_host: "127.0.0.1".to_string(),
            web_port: 8081,
            auth_enabled: false,
            auth_token: None,
            cert_store_path: "~/.mitmproxy-rs/certs".to_string(),
            flows_store_path: "~/.mitmproxy-rs/flows".to_string(),
            max_flows: 10000,
            ssl_insecure: false,
            upstream_cert: false,
            anticache: false,
            anticomp: false,
            showhost: false,
            no_server: false,
            mode: ProxyMode::Regular,
            upstream_server: None,
            listen_host: None,
            listen_port: None,
            certs_path: "~/.mitmproxy-rs/certs".to_string(),
            confdir: "~/.mitmproxy-rs".to_string(),
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path.as_ref().to_str().unwrap()))
            .build()?;

        let config: Config = settings.try_deserialize()?;
        Ok(config)
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn proxy_addr(&self) -> String {
        format!("{}:{}", self.proxy_host, self.proxy_port)
    }

    pub fn web_addr(&self) -> String {
        format!("{}:{}", self.web_host, self.web_port)
    }

    pub fn expand_path(&self, path: &str) -> String {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                return path.replace('~', home.to_str().unwrap());
            }
        }
        path.to_string()
    }

    pub fn cert_store_path(&self) -> String {
        self.expand_path(&self.cert_store_path)
    }

    pub fn flows_store_path(&self) -> String {
        self.expand_path(&self.flows_store_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.proxy_host, "127.0.0.1");
        assert_eq!(config.proxy_port, 8080);
        assert_eq!(config.web_port, 8081);
        assert!(!config.auth_enabled);
    }

    #[test]
    fn test_proxy_addr() {
        let config = Config::default();
        assert_eq!(config.proxy_addr(), "127.0.0.1:8080");
    }

    #[test]
    fn test_web_addr() {
        let config = Config::default();
        assert_eq!(config.web_addr(), "127.0.0.1:8081");
    }
}