//! Middleware stack: CORS, conditional requests, WAC-Allow headers, static redirects.

use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Request, Response},
    middleware::Next,
};
use tracing::debug;

/// Adds CORS headers to every response.
///
/// Mirrors the TypeScript `CorsHandler`.
pub async fn cors_middleware(
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let origin = req
        .headers()
        .get("origin")
        .cloned();
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    if let Some(o) = origin {
        headers.insert(
            HeaderName::from_static("access-control-allow-origin"),
            o,
        );
    } else {
        headers.insert(
            HeaderName::from_static("access-control-allow-origin"),
            HeaderValue::from_static("*"),
        );
    }
    headers.insert(
        HeaderName::from_static("access-control-allow-headers"),
        HeaderValue::from_static("Authorization, Content-Type, If-Match, If-None-Match, If-Modified-Since, If-Unmodified-Since"),
    );
    headers.insert(
        HeaderName::from_static("access-control-expose-headers"),
        HeaderValue::from_static("ETag, Last-Modified, Location, Link, WAC-Allow"),
    );
    res
}
