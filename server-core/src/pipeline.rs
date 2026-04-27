//! Builds the Axum router that represents the full request pipeline.
//!
//! Wires together middleware, route handlers, static assets, and error handling.

use axum::{Router, middleware};
use std::sync::Arc;

use crate::{
    middleware::cors_middleware,
    routing::ldp_router,
    store::LdpStore,
};

/// Owns the assembled request pipeline.
pub struct RequestPipeline {
    store: Arc<LdpStore>,
}

impl RequestPipeline {
    pub fn new() -> Self {
        Self {
            store: LdpStore::new(),
        }
    }

    /// Construct the full Axum `Router`.
    pub fn build_router(&self) -> Router {
        ldp_router(Arc::clone(&self.store))
            .layer(middleware::from_fn(cors_middleware))
    }
}

impl Default for RequestPipeline {
    fn default() -> Self {
        Self::new()
    }
}
