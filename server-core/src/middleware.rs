//! Middleware stack: CORS, conditional requests, WAC-Allow headers, static redirects.
//!
//! ## Logging
//!
//! `info!`  — every incoming request (method + URI) and its final status after
//!             CORS decoration.
//! `debug!` — origin header value used when setting `Access-Control-Allow-Origin`.

use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Request, Response},
    middleware::Next,
};
use tracing::{debug, info};

/// Adds CORS headers to every response.
///
/// Mirrors the TypeScript `CorsHandler`.
pub async fn cors_middleware(
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let method = req.method().clone();
    let uri    = req.uri().clone();

    let origin = req.headers().get("origin").cloned();
    debug!(
        method = %method,
        uri = %uri,
        origin = origin
            .as_ref()
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<none>"),
        "cors_middleware: incoming request"
    );

    let mut res = next.run(req).await;

    let status = res.status();
    info!(method = %method, uri = %uri, status = status.as_u16(),
          "cors_middleware: response ready");

    let headers = res.headers_mut();
    if let Some(o) = origin {
        debug!(origin = ?o, "cors_middleware: echoing Origin in ACAO header");
        headers.insert(
            HeaderName::from_static("access-control-allow-origin"),
            o,
        );
    } else {
        debug!("cors_middleware: no Origin header, using wildcard ACAO");
        headers.insert(
            HeaderName::from_static("access-control-allow-origin"),
            HeaderValue::from_static("*"),
        );
    }
    headers.insert(
        HeaderName::from_static("access-control-allow-headers"),
        HeaderValue::from_static(
            "Authorization, Content-Type, If-Match, If-None-Match, \
             If-Modified-Since, If-Unmodified-Since",
        ),
    );
    headers.insert(
        HeaderName::from_static("access-control-expose-headers"),
        HeaderValue::from_static(
            "ETag, Last-Modified, Location, Link, WAC-Allow",
        ),
    );
    res
}
