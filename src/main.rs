use clap::Parser;
use mitmproxy_rs::{config::Config, server::MitmproxyServer, Result};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "mitmproxy-rs")]
#[command(about = "A Rust implementation of mitmproxy's HTTP intercepting proxy")]
struct Cli {
    #[arg(short, long, default_value = "127.0.0.1")]
    listen_host: String,

    #[arg(short, long, default_value = "8080")]
    listen_port: u16,

    #[arg(short, long, default_value = "8081")]
    web_port: u16,

    #[arg(long)]
    web_host: Option<String>,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(if cli.verbose { Level::DEBUG } else { Level::INFO })
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting mitmproxy-rs");

    // Load configuration
    let config = if let Some(config_path) = cli.config {
        Config::from_file(&config_path)?
    } else {
        Config::default()
    };

    let mut server_config = config;
    server_config.proxy_host = cli.listen_host;
    server_config.proxy_port = cli.listen_port;
    server_config.web_port = cli.web_port;
    if let Some(web_host) = cli.web_host {
        server_config.web_host = web_host;
    }

    // Create and start the server
    let server = MitmproxyServer::new(server_config).await?;
    server.run().await?;

    Ok(())
}