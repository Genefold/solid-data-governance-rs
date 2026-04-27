//! `solid-test` binary — HTTP integration test runner.
//!
//! Connects to a **running** Solid server and exercises its HTTP API.
//! Designed to work against both this Rust server and the original
//! TypeScript CSS, so it is a useful regression guard after any refactor.
//!
//! # Usage
//! ```
//! # start the server first, e.g.:
//! #   solid-server --port 3001 &
//!
//! solid-test [OPTIONS]
//!
//! Options:
//!   -b, --base-url   <URL>   Server base URL [env: CSS_BASE_URL]
//!                            [default: http://localhost:3000/]
//!       --filter     <GLOB>  Only run suites whose name contains FILTER
//!   -v, --verbose            Print each request/response on success too
//!   -h, --help               Print help
//! ```
//!
//! The runner exits with code **0** on full pass, **1** if any test fails.

use anyhow::Result;
use clap::Parser;
use integration_tests::{RunConfig, TestSuite};
use tracing_subscriber::EnvFilter;

/// Run HTTP integration tests against a live Solid server.
#[derive(Parser, Debug)]
#[command(
    name    = "solid-test",
    author,
    version,
    about   = "HTTP integration tests against a running Solid Community Server"
)]
pub struct TestCli {
    /// Base URL of the server under test.
    #[arg(
        short = 'b',
        long,
        default_value = "http://localhost:3000/",
        env = "CSS_BASE_URL"
    )]
    pub base_url: String,

    /// Only run suites whose name contains this substring (case-insensitive).
    #[arg(long)]
    pub filter: Option<String>,

    /// Print each request/response line even for passing tests.
    #[arg(short, long)]
    pub verbose: bool,

    /// Timeout per individual HTTP request in milliseconds.
    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = TestCli::parse();

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();

    let cfg = RunConfig {
        base_url:   cli.base_url,
        filter:     cli.filter,
        verbose:    cli.verbose,
        timeout_ms: cli.timeout_ms,
    };

    let suite = TestSuite::new(cfg);
    let passed = suite.run().await;

    if passed {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
