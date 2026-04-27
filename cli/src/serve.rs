//! `solid-server` binary — parse CLI arguments and start the HTTP server.
//!
//! # Usage
//! ```
//! solid-server [OPTIONS]
//!
//! Options:
//!   -b, --base-url   <URL>    Base URL  [env: CSS_BASE_URL]  [default: http://localhost:3000/]
//!   -p, --port       <PORT>   TCP port  [env: CSS_PORT]      [default: 3000]
//!       --host       <HOST>   Hostname  [env: CSS_HOST]      [default: localhost]
//!   -l, --log-level  <LEVEL>  Log level [env: CSS_LOG_LEVEL] [default: info]
//!   -h, --help                Print help
//!   -V, --version             Print version
//! ```

use anyhow::Result;
use clap::Parser;
use server_core::{
    app::{App, AppConfig},
    pipeline::RequestPipeline,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Solid Community Server — Rust Edition.
#[derive(Parser, Debug)]
#[command(
    name    = "solid-server",
    author,
    version,
    about   = "Run the Solid Community Server",
    long_about = None
)]
pub struct ServeCli {
    /// Base URL that the server advertises to clients.
    /// Must end with a trailing slash.
    #[arg(
        short = 'b',
        long,
        default_value = "http://localhost:3000/",
        env = "CSS_BASE_URL"
    )]
    pub base_url: String,

    /// TCP port to listen on.
    #[arg(short = 'p', long, default_value_t = 3000, env = "CSS_PORT")]
    pub port: u16,

    /// Hostname or IP address to bind to.
    #[arg(long, default_value = "localhost", env = "CSS_HOST")]
    pub host: String,

    /// Logging level: trace | debug | info | warn | error.
    #[arg(short = 'l', long, default_value = "info", env = "CSS_LOG_LEVEL")]
    pub log_level: String,

    /// Root directory for file-backed storage (optional).
    /// When absent, the server uses in-memory storage.
    #[arg(long, env = "CSS_ROOT_DIR")]
    pub root_dir: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = ServeCli::parse();
    init_tracing(&cli.log_level);

    let addr: std::net::SocketAddr =
        format!("{}:{}", cli.host, cli.port).parse()?;

    info!(
        base_url = %cli.base_url,
        host     = %cli.host,
        port     = cli.port,
        log_level = %cli.log_level,
        root_dir = ?cli.root_dir,
        "Starting Solid Community Server"
    );

    let config = AppConfig {
        base_url:     cli.base_url,
        bind_address: addr,
        log_level:    cli.log_level,
    };

    let pipeline = RequestPipeline::new();
    let app = App::new(config, pipeline);
    app.start().await
}

// ── shared helpers ──────────────────────────────────────────────────────────

/// Initialise `tracing-subscriber`.  Honours `RUST_LOG`; falls back to
/// the value of `--log-level` / `CSS_LOG_LEVEL`.
pub fn init_tracing(default_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
