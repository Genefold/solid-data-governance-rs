//! Axum request handlers implementing a minimal LDP server.
//!
//! Every handler takes `State<Arc<LdpStore>>` plus an optional `Path<String>`
//! for the wildcard segment.  Root (`/`) requests have no path extractor.
//!
//! Covers all cases required by the five integration-test suites:
//!   health, resource-crud, containers, content-negotiation, error-responses.
//!
//! ## Logging
//!
//! Every handler emits:
//!   `info!`  — one summary line: method, resolved key, response status
//!   `debug!` — internal decisions (store hit/miss, content-type, body size,
//!               create-vs-overwrite, child slug, ancestor containers,
//!               Accept negotiation outcome)

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
use tracing::{debug, info};
use uuid::Uuid;

use crate::store::LdpStore;

// ── helpers ───────────────────────────────────────────────────────────────

/// Resolve the request path to a store key.
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
    debug!(key = %path, "store miss → 404");
    (
        StatusCode::NOT_FOUND,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("404 Not Found: {path}"),
    )
        .into_response()
}

/// Build a 406 response when the stored Content-Type cannot satisfy Accept.
fn not_acceptable(key: &str, accept: &str, stored_ct: &str) -> Response {
    debug!(
        key = %key,
        accept = %accept,
        stored_content_type = %stored_ct,
        "content negotiation failed → 406"
    );
    (
        StatusCode::NOT_ACCEPTABLE,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!(
            "406 Not Acceptable: cannot serve '{stored_ct}' as '{accept}'"
        ),
    )
        .into_response()
}

/// Returns `true` when the stored `content_type` satisfies the `accept` header.
///
/// Rules (simplified — no quality values, no subtype wildcards beyond `*/*`):
/// - Accept absent or `*/*` → always satisfied.
/// - Each comma-separated token is matched as a prefix against `content_type`
///   (so `text/*` matches `text/turtle`, `text/plain`, etc.).
/// - If any token matches, satisfied.
fn accept_satisfied(accept_header: &str, content_type: &str) -> bool {
    // Strip parameters from stored content-type for comparison.
    let ct_base = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();

    for token in accept_header.split(',') {
        let token = token
            .split(';') // strip quality params like ;q=0.9
            .next()
            .unwrap_or(token)
            .trim();

        if token == "*/*" || token.is_empty() {
            debug!(
                accept_token = %token,
                "accept_satisfied: wildcard or empty token → satisfied"
            );
            return true;
        }

        // Subtype wildcard: `text/*`
        if let Some(major) = token.strip_suffix("/*") {
            if ct_base.starts_with(&format!("{major}/")) {
                debug!(
                    accept_token = %token,
                    content_type = %ct_base,
                    "accept_satisfied: subtype wildcard match"
                );
                return true;
            }
            continue;
        }

        // Exact match.
        if ct_base == token {
            debug!(
                accept_token = %token,
                content_type = %ct_base,
                "accept_satisfied: exact match"
            );
            return true;
        }
    }

    debug!(
        accept = %accept_header,
        content_type = %ct_base,
        "accept_satisfied: no token matched → not satisfied"
    );
    false
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
    req: Request<Body>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    debug!(key = %key, "GET handler entered");

    let accept = req
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    if let Some(ref a) = accept {
        debug!(key = %key, accept = %a, "GET: Accept header present");
    } else {
        debug!(key = %key, "GET: no Accept header");
    }

    match store.get(&key) {
        None => {
            info!(method = "GET", key = %key, status = 404, "request complete");
            not_found(&key)
        }
        Some(entry) => {
            debug!(
                key = %key,
                content_type = %entry.content_type,
                body_bytes = entry.body.len(),
                is_container = entry.is_container,
                "store hit"
            );

            // Content negotiation: if Accept is present and the stored
            // Content-Type cannot satisfy it, return 406.
            if let Some(ref a) = accept {
                if !accept_satisfied(a, &entry.content_type) {
                    info!(
                        method = "GET",
                        key = %key,
                        accept = %a,
                        stored_content_type = %entry.content_type,
                        status = 406,
                        "request complete"
                    );
                    return not_acceptable(&key, a, &entry.content_type);
                }
            }

            let mut resp = Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, &entry.content_type);
            if entry.is_container {
                debug!(key = %key, "attaching ldp:Container Link header");
                resp = resp.header("Link", container_link());
            } else {
                resp = resp.header("Link", resource_link());
            }
            info!(
                method = "GET",
                key = %key,
                status = 200,
                content_type = %entry.content_type,
                "request complete"
            );
            resp.body(Body::from(entry.body))
                .unwrap_or_else(|e| {
                    info!(method = "GET", key = %key, error = %e, "response build failed");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                })
        }
    }
}

// ── HEAD ──────────────────────────────────────────────────────────────────

pub async fn handle_head(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
    req: Request<Body>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    debug!(key = %key, "HEAD handler entered");

    let accept = req
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    match store.get(&key) {
        None => {
            info!(method = "HEAD", key = %key, status = 404, "request complete");
            not_found(&key)
        }
        Some(entry) => {
            debug!(
                key = %key,
                content_type = %entry.content_type,
                is_container = entry.is_container,
                "store hit (no body returned for HEAD)"
            );

            // Apply the same Accept negotiation as GET.
            if let Some(ref a) = accept {
                if !accept_satisfied(a, &entry.content_type) {
                    info!(
                        method = "HEAD",
                        key = %key,
                        accept = %a,
                        stored_content_type = %entry.content_type,
                        status = 406,
                        "request complete"
                    );
                    return not_acceptable(&key, a, &entry.content_type);
                }
            }

            let mut resp = Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, &entry.content_type);
            if entry.is_container {
                resp = resp.header("Link", container_link());
            }
            info!(method = "HEAD", key = %key, status = 200, "request complete");
            resp.body(Body::empty())
                .unwrap_or_else(|e| {
                    info!(method = "HEAD", key = %key, error = %e, "response build failed");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                })
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
    debug!(key = %key, "PUT handler entered");

    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    debug!(key = %key, content_type = %content_type, "resolved Content-Type for PUT");

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => {
            debug!(key = %key, body_bytes = b.len(), "body read successfully");
            b.to_vec()
        }
        Err(e) => {
            info!(method = "PUT", key = %key, error = %e, status = 400, "body read error");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let created = store.put(&key, body_bytes, content_type.clone());
    let status = if created {
        debug!(key = %key, "PUT: new resource created → 201");
        StatusCode::CREATED
    } else {
        debug!(key = %key, "PUT: existing resource overwritten → 204");
        StatusCode::NO_CONTENT
    };

    let is_container = store.is_container(&key);
    let mut builder = Response::builder().status(status);
    if is_container {
        debug!(key = %key, "PUT: target is container, attaching Link header");
        builder = builder.header("Link", container_link());
    }

    info!(
        method = "PUT",
        key = %key,
        status = status.as_u16(),
        content_type = %content_type,
        is_container = is_container,
        created = created,
        "request complete"
    );

    builder
        .body(Body::empty())
        .unwrap_or_else(|e| {
            info!(method = "PUT", key = %key, error = %e, "response build failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

// ── POST ──────────────────────────────────────────────────────────────────

pub async fn handle_post(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
    req: Request<Body>,
) -> Response {
    let container_key = resolve_path(path.as_deref().map(|p| p.as_str()));
    debug!(container_key = %container_key, "POST handler entered");

    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    debug!(
        container_key = %container_key,
        content_type = %content_type,
        "resolved Content-Type for POST child"
    );

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => {
            debug!(container_key = %container_key, body_bytes = b.len(), "body read successfully");
            b.to_vec()
        }
        Err(e) => {
            info!(
                method = "POST",
                container_key = %container_key,
                error = %e,
                status = 400,
                "body read error"
            );
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let slug = Uuid::new_v4().to_string();
    let container_base = container_key.trim_end_matches('/');
    let child_key = format!("{container_base}/{slug}");
    debug!(
        container_key = %container_key,
        child_key = %child_key,
        "generated UUID slug for POST child"
    );

    store.put(&child_key, body_bytes, content_type.clone());

    info!(
        method = "POST",
        container_key = %container_key,
        child_key = %child_key,
        content_type = %content_type,
        status = 201,
        "request complete"
    );

    Response::builder()
        .status(StatusCode::CREATED)
        .header(LOCATION, &child_key)
        .body(Body::empty())
        .unwrap_or_else(|e| {
            info!(
                method = "POST",
                container_key = %container_key,
                error = %e,
                "response build failed"
            );
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

// ── DELETE ────────────────────────────────────────────────────────────────

pub async fn handle_delete(
    State(store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    debug!(key = %key, "DELETE handler entered");

    if store.delete(&key) {
        info!(method = "DELETE", key = %key, status = 204, "request complete");
        StatusCode::NO_CONTENT.into_response()
    } else {
        info!(method = "DELETE", key = %key, status = 404, "request complete");
        not_found(&key)
    }
}

// ── OPTIONS ───────────────────────────────────────────────────────────────

pub async fn handle_options(
    State(_store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    debug!(key = %key, "OPTIONS handler entered");
    info!(method = "OPTIONS", key = %key, status = 204, "request complete");

    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(ALLOW, "GET, HEAD, PUT, POST, DELETE, OPTIONS, PATCH")
        .body(Body::empty())
        .unwrap_or_else(|e| {
            info!(method = "OPTIONS", key = %key, error = %e, "response build failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

// ── PATCH ─────────────────────────────────────────────────────────────────

pub async fn handle_patch(
    State(_store): State<Arc<LdpStore>>,
    path: Option<Path<String>>,
    req: Request<Body>,
) -> Response {
    let key = resolve_path(path.as_deref().map(|p| p.as_str()));
    let patch_ct = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_owned();
    debug!(
        key = %key,
        patch_content_type = %patch_ct,
        "PATCH handler entered: no supported patch format"
    );
    info!(
        method = "PATCH",
        key = %key,
        patch_content_type = %patch_ct,
        status = 415,
        "unsupported patch Content-Type → 415"
    );
    (
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        "415 Unsupported patch Content-Type",
    )
        .into_response()
}
