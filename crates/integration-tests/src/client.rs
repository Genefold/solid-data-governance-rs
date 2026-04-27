//! Thin wrapper around `reqwest::Client` that carries the server's base URL
//! and provides helpers that mirror the patterns used in the TypeScript
//! integration tests (SolidClient, fetch helpers, etc.).

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Method, Response, StatusCode,
};
use url::Url;

/// A configured HTTP client bound to one Solid server.
#[derive(Clone)]
pub struct SolidClient {
    inner:    reqwest::Client,
    base_url: Url,
    verbose:  bool,
}

impl SolidClient {
    /// Build a new client.
    ///
    /// `timeout_ms` applies to each individual request.
    pub fn new(base_url: &str, timeout_ms: u64, verbose: bool) -> Result<Self> {
        let inner = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .use_rustls_tls()
            .build()
            .context("building reqwest client")?;
        let base_url = Url::parse(base_url)
            .with_context(|| format!("parsing base URL '{base_url}'"))?;
        Ok(Self { inner, base_url, verbose })
    }

    /// Resolve a relative path against the server base URL.
    pub fn url(&self, path: &str) -> Url {
        self.base_url
            .join(path)
            .unwrap_or_else(|_| self.base_url.clone())
    }

    // ── low-level helpers ─────────────────────────────────────────────────

    pub async fn get(&self, path: &str) -> Result<Response> {
        self.request(Method::GET, path, None, None).await
    }

    pub async fn get_accept(&self, path: &str, accept: &str) -> Result<Response> {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_str(accept).unwrap(),
        );
        self.request_with_headers(Method::GET, path, None, None, headers)
            .await
    }

    pub async fn head(&self, path: &str) -> Result<Response> {
        self.request(Method::HEAD, path, None, None).await
    }

    pub async fn put(
        &self,
        path: &str,
        body: impl Into<reqwest::Body>,
        content_type: &str,
    ) -> Result<Response> {
        self.request(
            Method::PUT,
            path,
            Some(body.into()),
            Some(content_type.to_owned()),
        )
        .await
    }

    pub async fn post(
        &self,
        path: &str,
        body: impl Into<reqwest::Body>,
        content_type: &str,
    ) -> Result<Response> {
        self.request(
            Method::POST,
            path,
            Some(body.into()),
            Some(content_type.to_owned()),
        )
        .await
    }

    pub async fn patch(
        &self,
        path: &str,
        body: impl Into<reqwest::Body>,
        content_type: &str,
    ) -> Result<Response> {
        self.request(
            Method::PATCH,
            path,
            Some(body.into()),
            Some(content_type.to_owned()),
        )
        .await
    }

    pub async fn delete(&self, path: &str) -> Result<Response> {
        self.request(Method::DELETE, path, None, None).await
    }

    pub async fn options(&self, path: &str) -> Result<Response> {
        self.request(Method::OPTIONS, path, None, None).await
    }

    // ── core dispatcher ───────────────────────────────────────────────────

    async fn request(
        &self,
        method:       Method,
        path:         &str,
        body:         Option<reqwest::Body>,
        content_type: Option<String>,
    ) -> Result<Response> {
        let headers = if let Some(ct) = content_type {
            let mut h = HeaderMap::new();
            h.insert(CONTENT_TYPE, HeaderValue::from_str(&ct).unwrap());
            h
        } else {
            HeaderMap::new()
        };
        self.request_with_headers(method, path, body, None, headers)
            .await
    }

    async fn request_with_headers(
        &self,
        method:  Method,
        path:    &str,
        body:    Option<reqwest::Body>,
        _slug:   Option<&str>,
        headers: HeaderMap,
    ) -> Result<Response> {
        let url = self.url(path);

        if self.verbose {
            println!("  → {method} {url}");
        }

        let mut builder = self.inner.request(method, url.as_str());
        builder = builder.headers(headers);
        if let Some(b) = body {
            builder = builder.body(b);
        }

        let response = builder.send().await.context("sending HTTP request")?;

        if self.verbose {
            println!("  ← {}", response.status());
        }

        Ok(response)
    }

    // ── assertion helpers ─────────────────────────────────────────────────

    /// Assert the response carries a specific status code.
    pub fn assert_status(resp: &Response, expected: StatusCode) -> Result<()> {
        let actual = resp.status();
        if actual != expected {
            anyhow::bail!(
                "expected HTTP {expected}, got {actual}"
            );
        }
        Ok(())
    }

    /// Assert the `Content-Type` header starts with `expected_prefix`.
    pub fn assert_content_type(resp: &Response, expected_prefix: &str) -> Result<()> {
        let ct = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !ct.starts_with(expected_prefix) {
            anyhow::bail!(
                "expected Content-Type starting with '{expected_prefix}', got '{ct}'"
            );
        }
        Ok(())
    }

    /// Assert the response has the `Link` header and it contains `rel_type`.
    pub fn assert_link_rel(resp: &Response, rel_type: &str) -> Result<()> {
        let link = resp
            .headers()
            .get(HeaderName::from_static("link"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !link.contains(rel_type) {
            anyhow::bail!("expected Link header to contain '{rel_type}', got '{link}'");
        }
        Ok(())
    }

    /// Assert the response body (text) contains `needle`.
    pub fn assert_body_contains(body: &str, needle: &str) -> Result<()> {
        if !body.contains(needle) {
            anyhow::bail!("expected body to contain '{needle}', got:\n{body}");
        }
        Ok(())
    }
}
