//! LDP route table.
//!
//! Maps HTTP methods and path patterns onto operation handlers.

use axum::{
    Router,
    routing::{delete, get, head, options, patch, post, put},
};
use std::sync::Arc;

use crate::{
    ldp_handlers::{
        handle_delete, handle_get, handle_head, handle_options, handle_patch, handle_post,
        handle_put,
    },
    store::LdpStore,
};

/// Build the LDP resource route table.
pub fn ldp_router(store: Arc<LdpStore>) -> Router {
    Router::new()
        // Wildcard catch-all — every other path.
        .route(
            "/{*path}",
            get(handle_get)
                .head(handle_head)
                .put(handle_put)
                .post(handle_post)
                .delete(handle_delete)
                .options(handle_options)
                .patch(handle_patch),
        )
        // Root resource — containers suite hits GET / and PUT /.
        .route(
            "/",
            get(handle_get)
                .head(handle_head)
                .put(handle_put)
                .post(handle_post)
                .delete(handle_delete)
                .options(handle_options)
                .patch(handle_patch),
        )
        .with_state(store)
}
