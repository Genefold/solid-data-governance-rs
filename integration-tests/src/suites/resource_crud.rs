//! Resource CRUD suite.
//!
//! Mirrors the scenarios from:
//!   test/integration/LdpHandlerWithoutAuth.test.ts
//!   test/integration/RepresentationConversion.test.ts
//!
//! Every test is self-contained: it uses a UUID-keyed path so parallel runs
//! do not collide, and it cleans up after itself with a DELETE.

use std::sync::Arc;

use anyhow::Result;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{
    client::SolidClient,
    runner::{case, Suite},
};

/// Generate a unique resource path for this test run.
fn uniq(suffix: &str) -> String {
    format!("/test-{}/{}", Uuid::new_v4(), suffix)
}

pub fn suite(client: Arc<SolidClient>) -> Suite {
    Suite {
        name: "resource-crud".into(),
        cases: vec![
            // ── PUT creates a document (201) ─────────────────────────────
            // it('should be able to create a new resource')
            {
                let c = Arc::clone(&client);
                case("PUT creates a new document and returns 201", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("file.txt");
                        let resp = c.put(&path, "hello world", "text/plain").await?;
                        SolidClient::assert_status(&resp, StatusCode::CREATED)?;
                        // cleanup
                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },

            // ── GET retrieves stored content ──────────────────────────────
            // it('should be able to read a resource')
            {
                let c = Arc::clone(&client);
                case("GET returns the stored body after PUT", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("hello.txt");
                        c.put(&path, "stored body", "text/plain").await?;

                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::OK)?;
                        let body = resp.text().await?;
                        SolidClient::assert_body_contains(&body, "stored body")?;

                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },

            // ── GET on absent resource → 404 ─────────────────────────────
            // it('should return 404 for missing resources')
            {
                let c = Arc::clone(&client);
                case("GET on absent resource returns 404", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("does-not-exist.txt");
                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::NOT_FOUND)
                    }
                })
            },

            // ── PUT overwrites (200 / 204) ────────────────────────────────
            // it('should be able to overwrite a resource')
            {
                let c = Arc::clone(&client);
                case("PUT on existing resource overwrites and returns 200 or 204", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("overwrite.txt");
                        c.put(&path, "original", "text/plain").await?;

                        let resp = c.put(&path, "updated", "text/plain").await?;
                        let status = resp.status();
                        if status != StatusCode::OK && status != StatusCode::NO_CONTENT {
                            anyhow::bail!("expected 200 or 204 for overwrite PUT, got {status}");
                        }

                        let body = c.get(&path).await?.text().await?;
                        SolidClient::assert_body_contains(&body, "updated")?;

                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },

            // ── DELETE removes the resource (204) ─────────────────────────
            // it('should be able to delete a resource')
            {
                let c = Arc::clone(&client);
                case("DELETE returns 204 and removes the resource", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("todelete.txt");
                        c.put(&path, "bye", "text/plain").await?;

                        let del = c.delete(&path).await?;
                        SolidClient::assert_status(&del, StatusCode::NO_CONTENT)?;

                        // Confirm gone
                        let get = c.get(&path).await?;
                        SolidClient::assert_status(&get, StatusCode::NOT_FOUND)
                    }
                })
            },

            // ── DELETE on absent resource → 404 ──────────────────────────
            {
                let c = Arc::clone(&client);
                case("DELETE on absent resource returns 404", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("phantom.txt");
                        let resp = c.delete(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::NOT_FOUND)
                    }
                })
            },

            // ── Content-Type is reflected in GET ─────────────────────────
            // it('should return the correct Content-Type')
            {
                let c = Arc::clone(&client);
                case("GET returns matching Content-Type", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = uniq("data.ttl");
                        let ttl = "@prefix : <http://example.org/> .\n:me a :Person .";
                        c.put(&path, ttl, "text/turtle").await?;

                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::OK)?;
                        SolidClient::assert_content_type(&resp, "text/turtle")?;

                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },
        ],
    }
}
