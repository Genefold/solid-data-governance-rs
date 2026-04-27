//! Top-level application lifecycle: start, stop, and cluster management.
//!
//! Mirrors the TypeScript `App` / `AppRunner` classes.

use crate::pipeline::RequestPipeline;
use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{info, error};

/// Configuration required to start the server.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub base_url: String,
    pub bind_address: SocketAddr,
    pub log_level: String,
}

/// The running application instance.
pub struct App {
    config: AppConfig,
    pipeline: RequestPipeline,
}

impl App {
    pub fn new(config: AppConfig, pipeline: RequestPipeline) -> Self {
        Self { config, pipeline }
    }

    /// Start the HTTP server and block until shutdown.
    pub async fn start(&self) -> Result<()> {
        let addr = self.config.bind_address;
        let listener = TcpListener::bind(addr).await?;
        info!("Solid Community Server listening on http://{}", addr);

        let router = self.pipeline.build_router();
        axum::serve(listener, router).await?;
        Ok(())
    }

    /// Gracefully stop the server.
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Solid Community Server...");
        Ok(())
    }
}
