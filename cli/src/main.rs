//! Solid Community Server — Rust Edition
//!
//! Entry point: parse CLI arguments, configure tracing, and boot the server.

use anyhow::{Context, Result};
use clap::Parser;
use server_core::{
    app::{App, AppConfig},
    pipeline::RequestPipeline,
};
use std::net::ToSocketAddrs;
use tracing_subscriber::EnvFilter;

/// Solid Community Server — Rust port.
#[derive(Parser, Debug)]
#[command(name = "solid-community-rs", author, version, about, long_about = None)]
pub struct Cli {
    /// Base URL of the server (e.g. http://localhost:3000/)
    #[arg(
        short = 'b',
        long,
        default_value = "http://localhost:3500/",
        env = "CSS_BASE_URL"
    )]
    pub base_url: String,

    /// TCP port to listen on.
    #[arg(short, long, default_value_t = 3000, env = "CSS_PORT")]
    pub port: u16,

    /// Hostname or IP address to bind to.
    ///
    /// Accepts both hostnames ("localhost") and IP literals ("0.0.0.0",
    /// "127.0.0.1", "::1"). The value is resolved via the OS resolver so
    /// DNS names work too.
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
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();

    // ── Resolve host → SocketAddr ──────────────────────────────────────────
    //
    // `SocketAddr::parse()` only accepts numeric IP literals; hostnames such
    // as "localhost" cause `invalid socket address syntax`.
    //
    // `ToSocketAddrs` performs a blocking OS-level lookup (getaddrinfo) that
    // handles both hostnames and IP literals uniformly.  The lookup is
    // intentionally synchronous: it happens once at startup before the async
    // runtime is under any load.
    let addr = format!("{}:{}", cli.host, cli.port)
        .to_socket_addrs()
        .with_context(|| {
            format!(
                "Could not resolve bind address `{}:{}`. \
                 Pass a hostname resolvable on this machine or an IP literal \
                 such as `127.0.0.1` or `0.0.0.0`.",
                cli.host, cli.port
            )
        })?
        .next()
        .with_context(|| format!("Host `{}` resolved to zero addresses.", cli.host))?;

    tracing::info!("Binding to {addr}");

    let config = AppConfig {
        base_url: cli.base_url.clone(),
        bind_address: addr,
        log_level: cli.log_level.clone(),
    };

    let pipeline = RequestPipeline::new();
    let app = App::new(config, pipeline);

    app.start().await
}
