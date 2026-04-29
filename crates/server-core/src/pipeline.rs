//! Builds the Axum router that represents the full request pipeline.
//!
//! Wires together middleware, route handlers, static assets, and error
//! handling. The top-level router is composed in two layers:
//!
//! 1. **Governance plane** — `/catalog/...` and `/datasets/...` routes
//!    backed by the [`GovernanceState`] (catalog, chunk store, token
//!    issuer).
//! 2. **LDP fallback** — the upstream Solid LDP/WAC handler, registered
//!    as a fallback router so it serves every other path (root,
//!    `/profile/...`, `/pods/...`, etc.) without colliding with the
//!    specific governance routes.

use std::{path::PathBuf, sync::Arc};

use axum::{Router, middleware};
use governance::{Catalog, TokenIssuer};
use zarr_storage::MmapChunkStore;

use crate::{
    governance::{GovernanceState, governance_router},
    middleware::cors_middleware,
    routing::ldp_router,
    store::LdpStore,
};

/// Owns the assembled request pipeline.
pub struct RequestPipeline {
    store: Arc<LdpStore>,
    governance: Arc<GovernanceState>,
}

/// Configuration for the data-plane components.
#[derive(Clone, Debug)]
pub struct DataPlaneConfig {
    /// Filesystem root where catalog metadata is stored.
    pub catalog_root: PathBuf,
    /// Filesystem root where Zarr chunk blobs are stored.
    pub chunks_root: PathBuf,
    /// Optional pre-shared HMAC secret for capability tokens.
    /// When `None`, a fresh random secret is generated at startup
    /// (tokens won't survive a restart).
    pub token_secret: Option<Vec<u8>>,
}

impl DataPlaneConfig {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            catalog_root: root.join("catalog"),
            chunks_root: root.join("chunks"),
            token_secret: None,
        }
    }
}

impl RequestPipeline {
    pub fn new(config: DataPlaneConfig) -> anyhow::Result<Self> {
        let catalog = Catalog::open(&config.catalog_root)?;
        let chunks = MmapChunkStore::open(&config.chunks_root)?;
        let tokens = match config.token_secret {
            Some(s) => TokenIssuer::new(s),
            None => TokenIssuer::random(),
        };
        let governance = GovernanceState::new(catalog, chunks, tokens);
        Ok(Self {
            store: LdpStore::new(),
            governance,
        })
    }

    /// Convenience constructor: sandbox everything under one directory.
    pub fn under(root: impl Into<PathBuf>) -> anyhow::Result<Self> {
        Self::new(DataPlaneConfig::under(root))
    }

    /// Borrow the shared governance state (catalog + chunks + tokens).
    pub fn governance_state(&self) -> &Arc<GovernanceState> {
        &self.governance
    }

    /// Construct the full Axum `Router`.
    pub fn build_router(&self) -> Router {
        // Governance routes are specific; LDP is a wildcard fallback.
        let governance = governance_router(Arc::clone(&self.governance));
        let ldp = ldp_router(Arc::clone(&self.store));
        governance
            .fallback_service(ldp)
            .layer(middleware::from_fn(cors_middleware))
    }
}
