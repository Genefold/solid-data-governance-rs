//! Error-response suite.
//!
//! Verifies that the server returns well-formed HTTP errors for common
//! invalid requests.
//!
//! Mirrors:
//!   test/integration/LdpHandlerWithoutAuth.test.ts  (error sections)
//!   test/unit/util/errors/HttpError.test.ts          (error shape)

use std::sync::Arc;

use anyhow::Result;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{
    client::SolidClient,
    runner::{case, Suite},
};

pub fn suite(client: Arc<SolidClient>) -> Suite {
    Suite {
        name: "error-responses".into(),
        cases: vec![
            // ── 404 for unknown resource ──────────────────────────────────
            // it('should return 404 for unknown resource')
            {
                let c = Arc::clone(&client);
                case("GET unknown path returns 404", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = format!("/does-not-exist-{}", Uuid::new_v4());
                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::NOT_FOUND)
                    }
                })
            },

            // ── 405 for unsupported method ────────────────────────────────
            // it('should return 405 for unsupported HTTP method on document')
            {
                let c = Arc::clone(&client);
                case(
                    "PATCH without supported patch Content-Type returns 415 or 405",
                    move || {
                        let c = Arc::clone(&c);
                        async move {
                            // First create a resource.
                            let path =
                                format!("/patch-target-{}.txt", Uuid::new_v4());
                            c.put(&path, "original", "text/plain").await?;

                            // PATCH with an unsupported patch media type.
                            let resp = c
                                .patch(
                                    &path,
                                    "garbage patch body",
                                    "application/x-not-a-patch-type",
                                )
                                .await?;

                            let status = resp.status();
                            if status != StatusCode::UNSUPPORTED_MEDIA_TYPE
                                && status != StatusCode::METHOD_NOT_ALLOWED
                                && status != StatusCode::BAD_REQUEST
                            {
                                anyhow::bail!(
                                    "expected 415|405|400 for bad PATCH, got {status}"
                                );
                            }

                            let _ = c.delete(&path).await;
                            Ok(())
                        }
                    },
                )
            },

            // ── Error response is JSON or text with status info ───────────
            // it('error response body should describe the error')
            {
                let c = Arc::clone(&client);
                case("404 response body describes the error", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = format!("/nope-{}", Uuid::new_v4());
                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::NOT_FOUND)?;

                        // Body should be non-empty (the server ought to describe the error).
                        let body = resp.text().await?;
                        if body.trim().is_empty() {
                            anyhow::bail!("expected non-empty error body for 404");
                        }
                        Ok(())
                    }
                })
            },

            // ── PUT with non-existent parent container → 404 or 201 ───────
            // CSS creates missing parent containers automatically (201);
            // other servers may return 404/409. Both are acceptable here.
            {
                let c = Arc::clone(&client);
                case(
                    "PUT to path with missing parent returns 201 (auto-create) or 404/409",
                    move || {
                        let c = Arc::clone(&c);
                        async move {
                            let path = format!(
                                "/no-such-container-{}/file.txt",
                                Uuid::new_v4()
                            );
                            let resp = c.put(&path, "body", "text/plain").await?;
                            let status = resp.status();
                            let ok = status == StatusCode::CREATED
                                || status == StatusCode::NOT_FOUND
                                || status == StatusCode::CONFLICT;
                            if !ok {
                                anyhow::bail!(
                                    "expected 201|404|409 for PUT to deep path, got {status}"
                                );
                            }
                            // Cleanup best-effort.
                            let _ = c.delete(&path).await;
                            Ok(())
                        }
                    },
                )
            },
        ],
    }
}
