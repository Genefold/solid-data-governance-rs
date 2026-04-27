//! Representation metadata.
//!
//! [`RepresentationMetadata`] carries all the HTTP header information
//! associated with a resource representation: content type, ETag,
//! Last-Modified, Link headers, and the conditional-request headers
//! used for optimistic concurrency control.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── LinkHeader ────────────────────────────────────────────────────────────

/// A typed `Link` header entry.
///
/// Encodes a single `<target>; rel="relation"` pair plus optional params.
///
/// # Example
/// ```
/// # use http_types::LinkHeader;
/// let lh = LinkHeader::new("http://www.w3.org/ns/ldp#Resource", "type");
/// assert_eq!(lh.to_string(), "<http://www.w3.org/ns/ldp#Resource>; rel=\"type\"");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkHeader {
    /// Target URI.
    pub target: String,
    /// Link relation (e.g. `"type"`, `"acl"`, `"describedby"`).
    pub rel: String,
    /// Additional link params as raw key=value pairs (e.g. `"anchor=\"…\""`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
}

impl LinkHeader {
    pub fn new(target: impl Into<String>, rel: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            rel: rel.into(),
            params: Vec::new(),
        }
    }

    /// Append an extra param string.
    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.params.push(param.into());
        self
    }
}

impl std::fmt::Display for LinkHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}>; rel=\"{}\"", self.target, self.rel)?;
        for p in &self.params {
            write!(f, "; {p}")?;
        }
        Ok(())
    }
}

// ── ConditionalHeaders ────────────────────────────────────────────────────

/// Conditional request headers that guard write operations.
///
/// All fields are `None` when the corresponding header was absent from the
/// incoming request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConditionalHeaders {
    /// `If-Match` — the request succeeds only if the ETag matches.
    pub if_match: Option<Vec<String>>,
    /// `If-None-Match` — the request succeeds only if the ETag does NOT match.
    pub if_none_match: Option<Vec<String>>,
    /// `If-Modified-Since` — the request succeeds only if modified after this
    /// date (HTTP-date string).
    pub if_modified_since: Option<String>,
    /// `If-Unmodified-Since` — the request succeeds only if NOT modified after
    /// this date.
    pub if_unmodified_since: Option<String>,
}

impl ConditionalHeaders {
    pub fn is_empty(&self) -> bool {
        self.if_match.is_none()
            && self.if_none_match.is_none()
            && self.if_modified_since.is_none()
            && self.if_unmodified_since.is_none()
    }
}

// ── RepresentationMetadata ────────────────────────────────────────────────

/// All metadata associated with a resource representation.
///
/// This is the Rust equivalent of `RepresentationMetadata` in the TypeScript
/// server.  It is serialisable so it can be persisted as a sidecar file.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RepresentationMetadata {
    // ── Content description ──────────────────────────────────────────────
    /// `Content-Type` header value, e.g. `"text/turtle; charset=utf-8"`.
    pub content_type: Option<String>,
    /// `Content-Length` in bytes.
    pub content_length: Option<u64>,
    /// `Content-Language` value, e.g. `"en-GB"`.
    pub content_language: Option<String>,

    // ── Caching & validators ─────────────────────────────────────────────
    /// `ETag` header value (with quotes), e.g. `"\"abc123\""`.
    pub etag: Option<String>,
    /// `Last-Modified` HTTP-date string.
    pub last_modified: Option<String>,

    // ── LDP / Solid link relations ───────────────────────────────────────
    /// Typed `Link` header entries.
    pub link_headers: Vec<LinkHeader>,

    // ── Conditional request guards ───────────────────────────────────────
    /// Conditional request headers (only populated on incoming requests).
    #[serde(default, skip_serializing_if = "ConditionalHeaders::is_empty")]
    pub conditionals: ConditionalHeaders,

    // ── Arbitrary extension metadata ─────────────────────────────────────
    /// Catch-all for any additional metadata key-value pairs.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

impl RepresentationMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Builder helpers ──────────────────────────────────────────────────

    pub fn with_content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }

    pub fn with_content_length(mut self, len: u64) -> Self {
        self.content_length = Some(len);
        self
    }

    pub fn with_content_language(mut self, lang: impl Into<String>) -> Self {
        self.content_language = Some(lang.into());
        self
    }

    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }

    pub fn with_last_modified(mut self, date: impl Into<String>) -> Self {
        self.last_modified = Some(date.into());
        self
    }

    pub fn with_link(mut self, link: LinkHeader) -> Self {
        self.link_headers.push(link);
        self
    }

    pub fn with_conditionals(mut self, cond: ConditionalHeaders) -> Self {
        self.conditionals = cond;
        self
    }

    // ── LDP convenience ──────────────────────────────────────────────────

    /// Add an `ldp:Resource` type link (all Solid resources carry this).
    pub fn add_ldp_resource_link(self) -> Self {
        self.with_link(LinkHeader::new(
            "http://www.w3.org/ns/ldp#Resource",
            "type",
        ))
    }

    /// Add an `ldp:BasicContainer` type link.
    pub fn add_ldp_container_link(self) -> Self {
        self.with_link(LinkHeader::new(
            "http://www.w3.org/ns/ldp#BasicContainer",
            "type",
        ))
    }

    /// Set the ACL resource link (`rel="acl"`).
    pub fn with_acl(mut self, acl_url: impl Into<String>) -> Self {
        self.link_headers
            .push(LinkHeader::new(acl_url, "acl"));
        self
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_header_display() {
        let lh = LinkHeader::new("http://www.w3.org/ns/ldp#Resource", "type");
        assert_eq!(
            lh.to_string(),
            "<http://www.w3.org/ns/ldp#Resource>; rel=\"type\""
        );
    }

    #[test]
    fn metadata_builder_roundtrip() {
        let m = RepresentationMetadata::new()
            .with_content_type("text/turtle")
            .with_etag("\"abc123\"")
            .add_ldp_resource_link();

        assert_eq!(m.content_type.as_deref(), Some("text/turtle"));
        assert_eq!(m.link_headers.len(), 1);
    }

    #[test]
    fn metadata_serde_roundtrip() {
        let m = RepresentationMetadata::new()
            .with_content_type("text/turtle")
            .with_content_length(42)
            .with_etag("\"xyz\"");
        let json = serde_json::to_string(&m).unwrap();
        let back: RepresentationMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content_type, m.content_type);
        assert_eq!(back.content_length, Some(42));
    }
}
