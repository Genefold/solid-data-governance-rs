//! Builds the Axum router that represents the full request pipeline.
//!
//! Wires together middleware, route handlers, static assets, and error handling.

use axum::{
    Router,
    routing::any,
    middleware,
};
use crate::middleware::cors_middleware;
use crate::routing::ldp_router;

/// Owns the assembled request pipeline.
pub struct RequestPipeline {
    // Future: inject storage, authz, identity, and static-assets handles here.
}

impl RequestPipeline {
    pub fn new() -> Self {
        Self {}
    }

    /// Construct the full Axum `Router`.
    pub fn build_router(&self) -> Router {
        ldp_router()
            .layer(middleware::from_fn(cors_middleware))
    }
}

impl Default for RequestPipeline {
    fn default() -> Self {
        Self::new()
    }
}
