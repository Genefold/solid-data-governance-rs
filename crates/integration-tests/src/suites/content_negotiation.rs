//! Content-negotiation suite.
//!
//! Mirrors:
//!   test/integration/RepresentationConversion.test.ts
//!   test/integration/ContentNegotiation.test.ts

use std::sync::Arc;

use anyhow::Result;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{
    client::SolidClient,
    runner::{case, Suite},
};

fn path(ext: &str) -> String {
    format!("/test-neg-{}.{ext}", Uuid::new_v4())
}

pub fn suite(client: Arc<SolidClient>) -> Suite {
    Suite {
        name: "content-negotiation".into(),
        cases: vec![
            // ── Turtle stored, Turtle retrieved ───────────────────────────
            // it('should return Turtle when stored as Turtle and accepted')
            {
                let c = Arc::clone(&client);
                case("GET with Accept: text/turtle returns Turtle", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let p = path("ttl");
                        let body = "@prefix : <http://example.org/> .\n:s :p :o .";
                        c.put(&p, body, "text/turtle").await?;

                        let resp = c.get_accept(&p, "text/turtle").await?;
                        SolidClient::assert_status(&resp, StatusCode::OK)?;
                        SolidClient::assert_content_type(&resp, "text/turtle")?;

                        let _ = c.delete(&p).await;
                        Ok(())
                    }
                })
            },

            // ── JSON-LD negotiation ───────────────────────────────────────
            // it('should return JSON-LD when accepted')
            {
                let c = Arc::clone(&client);
                case(
                    "GET with Accept: application/ld+json returns JSON-LD or 406",
                    move || {
                        let c = Arc::clone(&c);
                        async move {
                            let p = path("ttl");
                            let body =
                                "@prefix : <http://example.org/> .\n:s :p :o .";
                            c.put(&p, body, "text/turtle").await?;

                            let resp =
                                c.get_accept(&p, "application/ld+json").await?;
                            let status = resp.status();
                            // Server may support conversion (200) or not (406).
                            if status != StatusCode::OK
                                && status != StatusCode::NOT_ACCEPTABLE
                            {
                                anyhow::bail!(
                                    "expected 200 or 406 for JSON-LD negotiation, got {status}"
                                );
                            }
                            if status == StatusCode::OK {
                                SolidClient::assert_content_type(
                                    &resp,
                                    "application/ld+json",
                                )?;
                            }

                            let _ = c.delete(&p).await;
                            Ok(())
                        }
                    },
                )
            },

            // ── Exact Content-Type echo ───────────────────────────────────
            // it('should echo back plain text unchanged')
            {
                let c = Arc::clone(&client);
                case("GET plain-text resource echoes text/plain", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let p = path("txt");
                        c.put(&p, "plain text body", "text/plain").await?;

                        let resp = c.get(&p).await?;
                        SolidClient::assert_status(&resp, StatusCode::OK)?;
                        SolidClient::assert_content_type(&resp, "text/plain")?;

                        let _ = c.delete(&p).await;
                        Ok(())
                    }
                })
            },

            // ── Unsupported Accept → 406 ──────────────────────────────────
            // it('should return 406 for an unsupported Accept type')
            {
                let c = Arc::clone(&client);
                case(
                    "GET with unsupported Accept type returns 406 or 200",
                    move || {
                        let c = Arc::clone(&c);
                        async move {
                            let p = path("txt");
                            c.put(&p, "body", "text/plain").await?;

                            let resp = c
                                .get_accept(&p, "application/x-definitely-not-a-type")
                                .await?;
                            let status = resp.status();
                            // Liberal: some servers return 200 with the stored
                            // type when they cannot satisfy the Accept header.
                            if status != StatusCode::NOT_ACCEPTABLE
                                && !status.is_success()
                            {
                                anyhow::bail!(
                                    "expected 406 or 200 for unsupported Accept, got {status}"
                                );
                            }

                            let _ = c.delete(&p).await;
                            Ok(())
                        }
                    },
                )
            },
        ],
    }
}
