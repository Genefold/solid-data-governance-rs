//! Handler traits: `HttpHandler` and `OperationHttpHandler`.
//!
//! These traits form the spine of the request processing pipeline,
//! mirroring the TypeScript `HttpHandler` / `OperationHttpHandler` interfaces.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, Response},
    response::IntoResponse,
};
use http_types::{Operation, SolidError};

/// Processes a raw HTTP request and produces a response.
///
/// Mirrors the TypeScript `HttpHandler`.
#[async_trait]
pub trait HttpHandler: Send + Sync {
    async fn handle(&self, req: Request<Body>) -> Response<Body>;
}

/// Processes a parsed `Operation` and returns a `Response`.
///
/// Mirrors the TypeScript `OperationHttpHandler`.
#[async_trait]
pub trait OperationHttpHandler: Send + Sync {
    async fn handle_operation(&self, op: Operation) -> Result<Response<Body>, SolidError>;
}
