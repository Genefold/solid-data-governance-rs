//! End-to-end integration test for the governance + zarr HTTP surface.
//!
//! Boots the full RequestPipeline against a fresh tempdir, registers a
//! dataset, mints a token, ingests one chunk, then exercises both
//! whole-chunk reads and HTTP Range reads.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;
use server_core::pipeline::{DataPlaneConfig, RequestPipeline};
use tower::ServiceExt;

fn tempdir() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("sdg-it-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[tokio::test]
async fn full_governance_zarr_flow() {
    let root = tempdir();
    let cfg = DataPlaneConfig {
        catalog_root: root.join("catalog"),
        chunks_root: root.join("chunks"),
        token_secret: Some(b"phase-0-test-secret".to_vec()),
    };
    let pipeline = RequestPipeline::new(cfg.clone()).unwrap();
    let governance = pipeline.governance_state().clone();
    let app = pipeline.build_router();

    // 1. Register a dataset (PUT /catalog/{org}/{dataset}).
    let body = serde_json::to_vec(&json!({
        "title": "Demo",
        "description": "Phase 0 smoke",
        "shape": [4u64],
        "chunk_shape": [4u64],
        "dtype": "float32",
    }))
    .unwrap();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/catalog/org-a/demo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2. Issue a capability token (POST /catalog/{org}/{dataset}/tokens).
    let token_body = serde_json::to_vec(&json!({
        "webid": "https://alice.example/#me",
        "tier": "training",
        "ttl_seconds": 3600,
        "dpop_jkt": "test-thumbprint"
    }))
    .unwrap();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/catalog/org-a/demo/tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(token_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let token_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(token_json["token"].as_str().unwrap().starts_with("ey")); // JWT prefix
    assert_eq!(token_json["claims"]["dataset_id"], "org-a/demo");

    // 3. Ingest one 16-byte chunk directly via the shared chunk store
    //    (Phase 0 doesn't yet expose an HTTP write path; the CLI/upload
    //    flow lands in Phase 1).
    let bytes: Vec<u8> = (0u8..16).collect();
    governance
        .chunks
        .write_chunk("org-a/demo", "c/0", &bytes)
        .unwrap();

    // 4. Whole-chunk GET (no Range header).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/datasets/org-a/demo/c/0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::ACCEPT_RANGES).unwrap(),
        "bytes"
    );
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body_bytes.as_ref(), bytes.as_slice());

    // 5. Range GET (bytes=4-7) → 4 bytes from offset 4.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/datasets/org-a/demo/c/0")
                .header(header::RANGE, "bytes=4-7")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(header::CONTENT_RANGE).unwrap(),
        "bytes 4-7/16"
    );
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body_bytes.as_ref(), &bytes[4..8]);

    // 6. Suffix Range GET (last 3 bytes).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/datasets/org-a/demo/c/0")
                .header(header::RANGE, "bytes=-3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body_bytes.as_ref(), &bytes[13..16]);

    // 7. Out-of-range Range GET → 416.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/datasets/org-a/demo/c/0")
                .header(header::RANGE, "bytes=100-200")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);

    // 8. zarr.json metadata is served.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/datasets/org-a/demo/zarr.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let meta: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(meta["zarr_format"], 3);
    assert_eq!(meta["data_type"], "float32");
    assert_eq!(meta["shape"][0], 4);

    // 9. Audit stream contains create + token.issue events.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/catalog/org-a/demo/audit")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = std::str::from_utf8(&body_bytes).unwrap();
    let actions: Vec<String> = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["action"].as_str().unwrap().to_owned())
        .collect();
    assert!(actions.contains(&"dataset.create".to_owned()));
    assert!(actions.contains(&"token.issue".to_owned()));

    // 10. LDP fallback still works: GET / returns the upstream root container.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers()
            .get(header::LINK)
            .map(|v| v.to_str().unwrap_or(""))
            .unwrap_or("")
            .contains("ldp#Container")
    );

    std::fs::remove_dir_all(&root).ok();
}
