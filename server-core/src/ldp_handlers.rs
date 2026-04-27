//! Axum request handlers implementing a minimal LDP server.
//!
//! Every handler takes `State<Arc<LdpStore>>` plus an optional `Path<String>`
//! for the wildcard segment.  Root (`/`) requests have no path extractor.
//!
//! Covers all cases required by the five integration-test suites:
//!   health, resource-crud, containers, content-negotiation, error-responses.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{ALLOW, CONTENT_TYPE, LOCATION},
        HeaderValue, Request, StatusCode,
    },
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::store::LdpStore;

// ── helpers ───────────────────────────────────────────────────────────────

/// Resolve the request path to a store key.
/// `wildcard` is `Some(seg)` for `/*path` matches, `None` for `/`.
fn resolve_path(wildcard: Option<&str>) -> String {
    match wildcard {
        None | Some("") => "/".to_owned(),
        Some(p) => {
            if p.starts_with('/') {
                p.to_owned()
            } else {
                format!("/{p}")
            }
        }
    }
}

/// Build a 404 response with a plain-text body (satisfies test 22).
fn not_found(path: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("404 Not Found: {path}"),
    )
        .into_response()
}

/// Build the `Link` header value for an LDP Container.
fn container_link() -> HeaderValue {
    HeaderValue::from_static(
        "<http://www.w3.org/ns/ldp#Container>; rel=\"type\", \
         <http://www.w3.org/ns/ldp#Resource>; rel=\"type\"",
    )
}

/// Build the `Link` header value for an LDP Resource (document).
fn resource_link() -> HeaderValue {
    HeaderValue::from_static(
        "<http://www.w3.org/ns/ldp#Resource>; rel=\"type\"",
    )
}

// ── GET ───────────────────────────────────────────────────────────────────

pub async fn handle_get(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    match store.get(&key) {
        None => not_found(&key),
        Some(entry) => {
            let mut resp = Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, &entry.content_type);
            if entry.is_container {
                resp = resp.header("Link", container_link());
            } else {
                resp = resp.header("Link", resource_link());
            }
            resp.body(Body::from(entry.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    }
}

// ── HEAD ──────────────────────────────────────────────────────────────────

pub async fn handle_head(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    match store.get(&key) {
        None => not_found(&key),
        Some(entry) => {
            let mut resp = Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, &entry.content_type);
            if entry.is_container {
                resp = resp.header("Link", container_link());
            }
            // HEAD: empty body.
            resp.body(Body::empty())
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    }
}

// ── PUT ───────────────────────────────────────────────────────────────────

pub async fn handle_put(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
    req: Request<Body>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));

    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b.to_vec(),
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let created = store.put(&key, body_bytes, content_type);

    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::NO_CONTENT
    };

    let mut builder = Response::builder().status(status);
    if store.is_container(&key) {
        builder = builder.header("Link", container_link());
    }
    builder
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── POST ──────────────────────────────────────────────────────────────────

pub async fn handle_post(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
    req: Request<Body>,
) -> Response {
    let container_key = resolve_path(path.as_deref().map(|p| p.as_str()));

    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b.to_vec(),
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // Generate a UUID slug for the child resource.
    let slug = Uuid::new_v4().to_string();
    let container_base = container_key.trim_end_matches('/');
    let child_key = format!("{container_base}/{slug}");

    store.put(&child_key, body_bytes, content_type);

    Response::builder()
        .status(StatusCode::CREATED)
        .header(LOCATION, &child_key)
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── DELETE ────────────────────────────────────────────────────────────────

pub async fn handle_delete(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    if store.delete(&key) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        not_found(&key)
    }
}

// ── OPTIONS ───────────────────────────────────────────────────────────────

pub async fn handle_options(
    State(_store): State<Arc<LdpStore>>,
    _path: Option<Path<String>>,
) -> Response {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(
            ALLOW,
            "GET, HEAD, PUT, POST, DELETE, OPTIONS, PATCH",
        )
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── PATCH ─────────────────────────────────────────────────────────────────

pub async fn handle_patch(
    State(_store): State<Arc<LdpStore>>,
    _path: Option<Path<String>>,
    _req: Request<Body>,
) -> Response {
    // No patch format supported yet → 415 Unsupported Media Type.
    (
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        "415 Unsupported patch Content-Type",
    )
        .into_response()
}
