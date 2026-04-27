//! Health-check suite — verifies the server is reachable and responds
//! sensibly before the heavier suites run.
//!
//! Mirrors the smoke-test expectations from the CSS integration suite:
//!   test/integration/ — server startup checks.

use std::sync::Arc;

use anyhow::Result;
use reqwest::StatusCode;

use crate::{
    client::SolidClient,
    runner::{case, Suite},
};

pub fn suite(client: Arc<SolidClient>) -> Suite {
    let c = Arc::clone(&client);
    Suite {
        name: "health".into(),
        cases: vec![
            // it('server responds to GET /')
            case("GET / returns 200 or 401", move || {
                let c = Arc::clone(&c);
                async move {
                    let resp = c.get("/").await?;
                    let status = resp.status();
                    // Root may require auth (401) or return the container (200).
                    if status != StatusCode::OK && status != StatusCode::UNAUTHORIZED {
                        anyhow::bail!(
                            "expected 200 or 401 from GET /, got {status}"
                        );
                    }
                    Ok(())
                }
            }),

            // it('server responds to OPTIONS /')
            {
                let c = Arc::clone(&client);
                case("OPTIONS / returns 204 or 200 with Allow header", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let resp = c.options("/").await?;
                        let status = resp.status();
                        if status != StatusCode::NO_CONTENT && status != StatusCode::OK {
                            anyhow::bail!(
                                "expected 200 or 204 from OPTIONS /, got {status}"
                            );
                        }
                        Ok(())
                    }
                })
            },

            // it('HEAD / returns the same status as GET')
            {
                let c = Arc::clone(&client);
                case("HEAD / does not return a body", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let resp = c.head("/").await?;
                        let status = resp.status();
                        if !status.is_success() && status != StatusCode::UNAUTHORIZED {
                            anyhow::bail!("HEAD / returned unexpected {status}");
                        }
                        // HEAD must never return a body.
                        let body = resp.bytes().await?;
                        if !body.is_empty() {
                            anyhow::bail!("HEAD / returned a non-empty body");
                        }
                        Ok(())
                    }
                })
            },
        ],
    }
}
