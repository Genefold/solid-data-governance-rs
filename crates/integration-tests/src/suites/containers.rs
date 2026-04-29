//! LDP Container suite.
//!
//! Mirrors:
//!   test/integration/LdpHandlerWithoutAuth.test.ts  (container sections)
//!   test/integration/ContainerManager.test.ts

use std::sync::Arc;

use anyhow::Result;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{
    client::SolidClient,
    runner::{case, Suite},
};

fn container_path() -> String {
    format!("/test-{}/", Uuid::new_v4())
}

pub fn suite(client: Arc<SolidClient>) -> Suite {
    Suite {
        name: "containers".into(),
        cases: vec![
            // ── GET on root returns 200 or 401 (it is a container) ────────
            // it('root / is a container')
            {
                let c = Arc::clone(&client);
                case("GET / responds with 200 or 401 (root is a container)", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let resp = c.get("/").await?;
                        let s = resp.status();
                        if s != StatusCode::OK && s != StatusCode::UNAUTHORIZED {
                            anyhow::bail!("expected 200|401 from GET /, got {s}");
                        }
                        Ok(())
                    }
                })
            },

            // ── PUT to a container URL creates the container (201) ────────
            // it('should be able to create a container via PUT')
            {
                let c = Arc::clone(&client);
                case("PUT to container URL creates container (201)", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = container_path();
                        let resp = c
                            .put(&path, "", "text/turtle")
                            .await?;
                        SolidClient::assert_status(&resp, StatusCode::CREATED)?;
                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },

            // ── POST creates a child resource ─────────────────────────────
            // it('should be able to create a resource via POST')
            {
                let c = Arc::clone(&client);
                case("POST to container creates child resource (201)", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let container = container_path();
                        // Create the container first.
                        c.put(&container, "", "text/turtle").await?;

                        // POST a child into it.
                        let resp = c
                            .post(&container, "child content", "text/plain")
                            .await?;
                        SolidClient::assert_status(&resp, StatusCode::CREATED)?;

                        // Location header must be present.
                        if resp.headers().get("location").is_none() {
                            anyhow::bail!("POST response missing Location header");
                        }

                        let _ = c.delete(&container).await;
                        Ok(())
                    }
                })
            },

            // ── GET on container returns Link: rel="type"; ldp:Container ──
            // it('container GET should include ldp:Container Link header')
            {
                let c = Arc::clone(&client);
                case("GET on container includes ldp:Container Link header", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = container_path();
                        c.put(&path, "", "text/turtle").await?;

                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::OK)?;
                        SolidClient::assert_link_rel(
                            &resp,
                            "http://www.w3.org/ns/ldp#Container",
                        )?;

                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },

            // ── DELETE empty container succeeds (204) ─────────────────────
            // it('should delete an empty container')
            {
                let c = Arc::clone(&client);
                case("DELETE on empty container returns 204", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = container_path();
                        c.put(&path, "", "text/turtle").await?;

                        let resp = c.delete(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::NO_CONTENT)
                    }
                })
            },
        ],
    }
}
