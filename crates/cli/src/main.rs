//! Solid Community Server — Rust Edition
//!
//! Entry point: parse CLI arguments, configure tracing, and boot the server.

use anyhow::Result;
use clap::Parser;
use server_core::{
    app::{App, AppConfig},
    pipeline::RequestPipeline,
};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// Solid Community Server — Rust port.
#[derive(Parser, Debug)]
#[command(name = "solid-community-rs", author, version, about, long_about = None)]
pub struct Cli {
    /// Base URL of the server (e.g. http://localhost:3000/)
    #[arg(short = 'b', long, default_value = "http://localhost:3000/", env = "CSS_BASE_URL")]
    pub base_url: String,

    /// TCP port to listen on.
    #[arg(short, long, default_value_t = 3000, env = "CSS_PORT")]
    pub port: u16,

    /// Hostname or IP address to bind to.
    #[arg(long, default_value = "localhost", env = "CSS_HOST")]
    pub host: String,

    /// Logging level (trace | debug | info | warn | error).
    #[arg(short = 'l', long, default_value = "info", env = "CSS_LOG_LEVEL")]
    pub log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialise structured tracing.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();

    let addr: std::net::SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let config = AppConfig {
        base_url: cli.base_url.clone(),
        bind_address: addr,
        log_level: cli.log_level.clone(),
    };

    let pipeline = RequestPipeline::new();
    let app = App::new(config, pipeline);

    app.start().await
}
