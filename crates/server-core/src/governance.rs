//! HTTP surface for the Phase 0 governance plane.
//!
//! Exposes:
//!
//! - `PUT  /catalog/:org/:dataset`         — register a new dataset.
//! - `GET  /catalog`                       — list all registered datasets.
//! - `GET  /catalog/:org/:dataset`         — fetch dataset metadata + policy.
//! - `POST /catalog/:org/:dataset/tokens`  — mint a DPoP-bound capability token.
//! - `GET  /catalog/:org/:dataset/audit`   — return the audit event stream as NDJSON.
//! - `GET  /datasets/:org/:dataset/zarr.json` — Zarr v3 array metadata.
//! - `GET  /datasets/:org/:dataset/c/*key` — Zarr chunk bytes (HTTP Range supported).
//!
//! All handlers share a [`GovernanceState`] owning the [`Catalog`],
//! [`MmapChunkStore`], and [`TokenIssuer`].

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE},
    },
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use governance::{
    AccessPolicy, AuditEvent, Catalog, CapabilityClaims, Tier, TokenIssuer,
    catalog::CatalogEntry,
};
use serde::Deserialize;
use tracing::{debug, info, warn};
use zarr_storage::{ArrayManifest, MmapChunkStore, ZarrV3ArrayMeta, store::ChunkStoreError};

/// Shared state for all governance/dataset handlers.
pub struct GovernanceState {
    pub catalog: Catalog,
    pub chunks: MmapChunkStore,
    pub tokens: TokenIssuer,
}

impl GovernanceState {
    pub fn new(catalog: Catalog, chunks: MmapChunkStore, tokens: TokenIssuer) -> Arc<Self> {
        Arc::new(Self {
            catalog,
            chunks,
            tokens,
        })
    }
}

/// Build the governance + dataset routes.
pub fn governance_router(state: Arc<GovernanceState>) -> Router {
    Router::new()
        .route("/catalog", get(list_datasets))
        .route(
            "/catalog/{org}/{dataset}",
            put(create_dataset).get(get_dataset),
        )
        .route("/catalog/{org}/{dataset}/policy", put(put_policy))
        .route("/catalog/{org}/{dataset}/tokens", post(issue_token))
        .route("/catalog/{org}/{dataset}/audit", get(get_audit_stream))
        .route(
            "/datasets/{org}/{dataset}/zarr.json",
            get(get_zarr_metadata),
        )
        .route(
            "/datasets/{org}/{dataset}/c/{*chunk_path}",
            get(get_zarr_chunk),
        )
        .with_state(state)
}

// ── helpers ───────────────────────────────────────────────────────────────

fn dataset_id(org: &str, dataset: &str) -> String {
    format!("{org}/{dataset}")
}

fn json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap()
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    json_response(status, &serde_json::json!({ "error": message.into() }))
}

// ── PUT /catalog/:org/:dataset ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateDatasetBody {
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Zarr array shape. Defaults to a tiny 1-D placeholder.
    #[serde(default = "default_shape")]
    pub shape: Vec<u64>,
    /// Chunk shape. Defaults to the array shape.
    #[serde(default)]
    pub chunk_shape: Option<Vec<u64>>,
    /// Zarr v3 dtype identifier (e.g. `"float32"`).
    #[serde(default = "default_dtype")]
    pub dtype: String,
}
fn default_shape() -> Vec<u64> {
    vec![0]
}
fn default_dtype() -> String {
    "float32".to_owned()
}

pub async fn create_dataset(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset)): Path<(String, String)>,
    Json(body): Json<CreateDatasetBody>,
) -> Response {
    let id = dataset_id(&org, &dataset);
    let chunk_shape = body.chunk_shape.unwrap_or_else(|| body.shape.clone());

    match state.catalog.register(&id, &body.title, &body.description) {
        Ok(entry) => {
            // Create a backing zarr blob alongside the catalog entry.
            let manifest = ArrayManifest::new(
                &id,
                body.shape.clone(),
                chunk_shape.clone(),
                body.dtype.clone(),
            );
            if let Err(e) = state.chunks.create_dataset(manifest) {
                warn!(error = %e, "create_dataset: chunk_store create failed");
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("chunk store error: {e}"),
                );
            }
            info!(method = "PUT", path = %format!("/catalog/{id}"), status = 201, "dataset registered");
            json_response(StatusCode::CREATED, &entry)
        }
        Err(governance::CatalogError::AlreadyExists(_)) => {
            error_response(StatusCode::CONFLICT, format!("dataset exists: {id}"))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    }
}

// ── GET /catalog/:org/:dataset ────────────────────────────────────────────

pub async fn get_dataset(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset)): Path<(String, String)>,
) -> Response {
    let id = dataset_id(&org, &dataset);
    match state.catalog.get(&id) {
        Ok(entry) => json_response(StatusCode::OK, &entry),
        Err(governance::CatalogError::NotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, format!("dataset not found: {id}"))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    }
}

// ── GET /catalog ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct CatalogList {
    datasets: Vec<CatalogEntry>,
}

pub async fn list_datasets(State(state): State<Arc<GovernanceState>>) -> Response {
    let ids = match state.catalog.list() {
        Ok(ids) => ids,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };
    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(entry) = state.catalog.get(&id) {
            entries.push(entry);
        }
    }
    json_response(StatusCode::OK, &CatalogList { datasets: entries })
}

// ── PUT /catalog/:org/:dataset/policy ─────────────────────────────────────

pub async fn put_policy(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset)): Path<(String, String)>,
    Json(policy): Json<AccessPolicy>,
) -> Response {
    let id = dataset_id(&org, &dataset);
    if policy.dataset_id != id {
        return error_response(
            StatusCode::BAD_REQUEST,
            "policy.dataset_id does not match URL path",
        );
    }
    match state.catalog.put_policy(&id, &policy) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(governance::CatalogError::NotFound(_)) => {
            error_response(StatusCode::NOT_FOUND, format!("dataset not found: {id}"))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    }
}

// ── POST /catalog/:org/:dataset/tokens ────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IssueTokenBody {
    pub webid: String,
    pub tier: Tier,
    /// TTL in seconds. Defaults to 1 hour.
    #[serde(default = "default_ttl")]
    pub ttl_seconds: u64,
    /// Optional byte cap (only meaningful for the evaluation tier).
    #[serde(default)]
    pub byte_cap: Option<u64>,
    /// JWK thumbprint of the DPoP key the client will use.
    pub dpop_jkt: String,
}
fn default_ttl() -> u64 {
    3600
}

#[derive(Debug, serde::Serialize)]
struct IssueTokenResponse {
    pub token: String,
    pub claims: CapabilityClaims,
}

pub async fn issue_token(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset)): Path<(String, String)>,
    Json(body): Json<IssueTokenBody>,
) -> Response {
    let id = dataset_id(&org, &dataset);
    if state.catalog.get(&id).is_err() {
        return error_response(StatusCode::NOT_FOUND, format!("dataset not found: {id}"));
    }
    let claims = CapabilityClaims::new(
        body.webid.clone(),
        &id,
        body.tier,
        body.ttl_seconds,
        body.byte_cap,
        body.dpop_jkt,
    );
    let token = match state.tokens.sign(&claims) {
        Ok(t) => t,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };

    if let Ok(log) = state.catalog.audit_log(&id) {
        let _ = log.append(
            &AuditEvent::new("token.issue", &id)
                .with_principal(body.webid)
                .with_tier(body.tier.as_str())
                .with_status(201),
        );
    }

    json_response(
        StatusCode::CREATED,
        &IssueTokenResponse { token, claims },
    )
}

// ── GET /catalog/:org/:dataset/audit ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn get_audit_stream(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset)): Path<(String, String)>,
    Query(q): Query<AuditQuery>,
) -> Response {
    let id = dataset_id(&org, &dataset);
    let log = match state.catalog.audit_log(&id) {
        Ok(l) => l,
        Err(governance::CatalogError::NotFound(_)) => {
            return error_response(StatusCode::NOT_FOUND, format!("dataset not found: {id}"));
        }
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };
    let events = match log.read_all(q.limit) {
        Ok(e) => e,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };
    let mut body = Vec::new();
    for ev in events {
        let line = serde_json::to_vec(&ev).unwrap_or_default();
        body.extend_from_slice(&line);
        body.push(b'\n');
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/x-ndjson")
        .body(Body::from(body))
        .unwrap()
}

// ── GET /datasets/:org/:dataset/zarr.json ─────────────────────────────────

pub async fn get_zarr_metadata(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset)): Path<(String, String)>,
) -> Response {
    let id = dataset_id(&org, &dataset);
    let manifest = match state.chunks.manifest(&id) {
        Ok(m) => m,
        Err(ChunkStoreError::DatasetNotFound(_)) => {
            return error_response(StatusCode::NOT_FOUND, format!("dataset not found: {id}"));
        }
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };
    let meta = ZarrV3ArrayMeta::from_manifest(&manifest);
    json_response(StatusCode::OK, &meta)
}

// ── GET /datasets/:org/:dataset/c/*key  (with HTTP Range) ────────────────

pub async fn get_zarr_chunk(
    State(state): State<Arc<GovernanceState>>,
    Path((org, dataset, chunk_path)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let id = dataset_id(&org, &dataset);
    // Zarr v3 default chunk key is `c/<index>` with `/` separator. Our route
    // captures `chunk_path` as the trailing segments after `c/`, so the
    // canonical key becomes `c/<chunk_path>`.
    let chunk_key = format!("c/{chunk_path}");

    let length = match state.chunks.chunk_length(&id, &chunk_key) {
        Ok(l) => l,
        Err(ChunkStoreError::DatasetNotFound(_)) => {
            return error_response(StatusCode::NOT_FOUND, format!("dataset not found: {id}"));
        }
        Err(ChunkStoreError::ChunkNotFound(_)) => {
            return error_response(StatusCode::NOT_FOUND, format!("chunk not found: {chunk_key}"));
        }
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };

    let range_header = headers
        .get(RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let (status, start, end) = match range_header.as_deref() {
        Some(h) => match parse_range(h, length) {
            Some((s, e)) => (StatusCode::PARTIAL_CONTENT, s, e),
            None => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(CONTENT_RANGE, format!("bytes */{length}"))
                    .body(Body::empty())
                    .unwrap();
            }
        },
        None => (StatusCode::OK, 0u64, length),
    };

    let bytes = match state.chunks.read_chunk_range(&id, &chunk_key, start, end) {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
    };
    debug!(
        dataset_id = %id,
        chunk_key = %chunk_key,
        start, end,
        body_bytes = bytes.len(),
        status = status.as_u16(),
        "zarr chunk served"
    );

    let mut builder = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(ACCEPT_RANGES, HeaderValue::from_static("bytes"))
        .header(CONTENT_LENGTH, (end - start).to_string());
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end - 1, length),
        );
    }
    builder.body(Body::from(bytes)).unwrap()
}

/// Parse a single `bytes=start-end` Range header.
///
/// Returns `(start, end_exclusive)` on success or `None` on failure.
/// Multipart byte ranges are not supported (Phase 0 simplification).
fn parse_range(header: &str, length: u64) -> Option<(u64, u64)> {
    let raw = header.strip_prefix("bytes=")?;
    let (lhs, rhs) = raw.split_once('-')?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    if rhs.contains(',') {
        return None; // multipart ranges not supported
    }
    let (start, end_inclusive) = match (lhs.is_empty(), rhs.is_empty()) {
        (true, false) => {
            // Suffix range: last N bytes.
            let n: u64 = rhs.parse().ok()?;
            if n == 0 || n > length {
                (0u64, length.saturating_sub(1))
            } else {
                (length - n, length - 1)
            }
        }
        (false, true) => {
            let s: u64 = lhs.parse().ok()?;
            if s >= length {
                return None;
            }
            (s, length - 1)
        }
        (false, false) => {
            let s: u64 = lhs.parse().ok()?;
            let mut e: u64 = rhs.parse().ok()?;
            if e >= length {
                e = length - 1;
            }
            if s > e || s >= length {
                return None;
            }
            (s, e)
        }
        (true, true) => return None,
    };
    Some((start, end_inclusive + 1))
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn parse_bytes_ranges() {
        assert_eq!(parse_range("bytes=0-9", 10), Some((0, 10)));
        assert_eq!(parse_range("bytes=2-5", 10), Some((2, 6)));
        assert_eq!(parse_range("bytes=5-", 10), Some((5, 10)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 10)));
        assert_eq!(parse_range("bytes=2-100", 10), Some((2, 10)));
        assert_eq!(parse_range("bytes=100-200", 10), None);
        assert_eq!(parse_range("bytes=", 10), None);
        assert_eq!(parse_range("bytes=0-1,2-3", 10), None);
    }
}
