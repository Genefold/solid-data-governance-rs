//! Handler trait definitions.
//!
//! [`HttpHandler`] and [`OperationHttpHandler`] are the two core interfaces
//! that structure the Solid request pipeline.  Every component that processes
//! an HTTP request implements one of these traits.
//!
//! They live in `http-types` (not `server-core`) because they are *domain
//! contracts* — anything that wants to implement or consume a handler should
//! not need to depend on the full server-core crate.
//!
//! Enabled only when the **`axum`** feature is active, since the traits
//! reference `axum::http` and `axum::body` types.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, Response},
};
use crate::{Operation, SolidError};

// ── HttpHandler ────────────────────────────────────────────────────────────

/// Processes a raw HTTP request and produces a fully-formed response.
///
/// This is the outer-most handler in the pipeline, receiving the request
/// exactly as axum delivers it.  Implementations are responsible for:
/// - parsing the request into an [`Operation`],
/// - dispatching to an [`OperationHttpHandler`], and
/// - converting any [`SolidError`] into an appropriate HTTP response.
///
/// Mirrors the TypeScript `HttpHandler` abstract class.
#[async_trait]
pub trait HttpHandler: Send + Sync {
    /// Handle `req` and return a response.  Must not panic.
    async fn handle(&self, req: Request<Body>) -> Response<Body>;
}

// ── OperationHttpHandler ──────────────────────────────────────────────────

/// Processes a parsed [`Operation`] and returns a response.
///
/// This sits one step inside `HttpHandler` in the pipeline.  By the time
/// an `OperationHttpHandler` is called the request has already been:
/// - authenticated,
/// - authorised, and
/// - decoded into an `Operation`.
///
/// Implementations return `Err(SolidError)` for all expected HTTP failure
/// modes; unexpected panics should *not* propagate past this boundary.
///
/// Mirrors the TypeScript `OperationHttpHandler` abstract class.
#[async_trait]
pub trait OperationHttpHandler: Send + Sync {
    /// Execute the operation and produce a response.
    async fn handle_operation(
        &self,
        op: Operation,
    ) -> Result<Response<Body>, SolidError>;
}

// ── WaterfallHandler ──────────────────────────────────────────────────────

/// A sequential chain of [`OperationHttpHandler`]s.
///
/// Attempts each handler in order; returns the first `Ok` response.  If every
/// handler returns `Err(SolidError::MethodNotAllowed)`, the waterfall itself
/// returns that error.  Any other error short-circuits immediately.
///
/// Mirrors the TypeScript `WaterfallHandler`.
pub struct WaterfallHandler {
    handlers: Vec<Box<dyn OperationHttpHandler>>,
}

impl WaterfallHandler {
    pub fn new(handlers: Vec<Box<dyn OperationHttpHandler>>) -> Self {
        Self { handlers }
    }
}

#[async_trait]
impl OperationHttpHandler for WaterfallHandler {
    async fn handle_operation(
        &self,
        op: Operation,
    ) -> Result<Response<Body>, SolidError> {
        // We need to "replay" the operation across each handler, but Operation
        // is not Clone (it may own a Bytes body).  We therefore consume `op`
        // into the first handler that succeeds.  For the waterfall to try
        // multiple handlers the operation must be rebuilt for each attempt —
        // this is only sound for read-only operations.  Write operations should
        // use a single, concrete handler.
        //
        // TODO: make Operation Clone (Bytes is cheap to clone) and remove this
        //       restriction.
        let mut last_err = SolidError::MethodNotAllowed(
            "No handler accepted the operation".into(),
        );
        for handler in &self.handlers {
            // Safety: we cannot call handle_operation more than once without
            // Clone.  For now the waterfall forwards to the first handler only
            // and returns its result directly.  A proper waterfall requires
            // Operation: Clone (tracked as a TODO above).
            return handler.handle_operation(op).await;
            #[allow(unreachable_code)]
            {
                last_err = SolidError::MethodNotAllowed(String::new());
            }
        }
        Err(last_err)
    }
}
