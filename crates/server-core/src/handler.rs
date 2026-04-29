//! Handler re-exports for `server-core`.
//!
//! The canonical trait definitions live in `http-types` (behind the `axum`
//! feature flag) to keep them usable without depending on the full
//! server-core crate.  This module simply re-exports them so that code which
//! already depends on `server-core` can import from a single place.

pub use http_types::handler::{HttpHandler, OperationHttpHandler, WaterfallHandler};
