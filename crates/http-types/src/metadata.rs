//! Representation metadata (content-type, ETag, last-modified, link headers).

use std::collections::HashMap;

/// Metadata attached to a resource representation.
///
/// Analogous to `RepresentationMetadata` in the TypeScript server.
#[derive(Debug, Default, Clone)]
pub struct RepresentationMetadata {
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub link_headers: Vec<String>,
    pub extra: HashMap<String, String>,
}

impl RepresentationMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }

    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }
}
