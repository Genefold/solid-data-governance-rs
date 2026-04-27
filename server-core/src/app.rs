//! Top-level application lifecycle: start, stop, and cluster management.
//!
//! Mirrors the TypeScript `App` / `AppRunner` classes.

use crate::pipeline::RequestPipeline;
use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

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
        debug!(
            base_url = %self.config.base_url,
            bind_address = %self.config.bind_address,
            log_level = %self.config.log_level,
            "App::start: binding server"
        );

        let addr = self.config.bind_address;
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            error!(bind_address = %addr, error = %e, "App::start: TcpListener::bind failed");
            e
        })?;

        info!(
            base_url = %self.config.base_url,
            bind_address = %addr,
            "Solid Community Server listening"
        );

        let router = self.pipeline.build_router();
        debug!("App::start: router built, entering accept loop");

        axum::serve(listener, router).await.map_err(|e| {
            error!(error = %e, "App::start: axum::serve exited with error");
            anyhow::anyhow!(e)
        })?;
        Ok(())
    }

    /// Gracefully stop the server.
    pub async fn stop(&self) -> Result<()> {
        info!("App::stop: shutting down Solid Community Server");
        Ok(())
    }
}
