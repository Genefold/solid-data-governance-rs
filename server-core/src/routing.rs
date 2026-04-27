//! LDP route table.
//!
//! Maps HTTP methods and path patterns onto operation handlers.

use axum::{
    Router,
    routing::{delete, get, head, options, patch, post, put},
    http::StatusCode,
};

/// Stub handler — replaced per-method once operation handlers are wired up.
async fn not_implemented() -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

/// Build the LDP resource route table.
pub fn ldp_router() -> Router {
    Router::new()
        // Wildcard catch-all — routes every path through the operation pipeline.
        .route("/{*path}",
            get(not_implemented)
            .head(not_implemented)
            .put(not_implemented)
            .post(not_implemented)
            .patch(not_implemented)
            .delete(not_implemented)
            .options(not_implemented)
        )
        // Root resource.
        .route("/",
            get(not_implemented)
            .head(not_implemented)
            .post(not_implemented)
            .options(not_implemented)
        )
}
