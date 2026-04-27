//! Resource representation: body bytes + metadata.
//!
//! A [`Representation`] is a snapshot of a resource's state at a specific
//! content type.  It pairs the raw bytes with the metadata that describes
//! them, mirroring the TypeScript `BasicRepresentation`.

use crate::metadata::RepresentationMetadata;
use bytes::Bytes;

/// A resource body together with its describing metadata.
#[derive(Debug, Clone)]
pub struct Representation {
    /// The raw body bytes.  Empty for HEAD responses or empty resources.
    pub data: Bytes,
    /// All metadata associated with this representation.
    pub metadata: RepresentationMetadata,
}

impl Representation {
    /// Construct a representation with `data` and `metadata`.
    pub fn new(data: impl Into<Bytes>, metadata: RepresentationMetadata) -> Self {
        Self {
            data: data.into(),
            metadata,
        }
    }

    /// Construct an empty representation (e.g. for 204 / HEAD responses).
    pub fn empty(metadata: RepresentationMetadata) -> Self {
        Self {
            data: Bytes::new(),
            metadata,
        }
    }

    /// Convenience: build from a UTF-8 string and a content-type.
    pub fn from_text(text: impl Into<String>, content_type: impl Into<String>) -> Self {
        let s: String = text.into();
        let len = s.len() as u64;
        Self::new(
            s.into_bytes(),
            RepresentationMetadata::new()
                .with_content_type(content_type)
                .with_content_length(len),
        )
    }

    /// Convenience: build from raw bytes and a content-type.
    pub fn from_bytes(data: impl Into<Bytes>, content_type: impl Into<String>) -> Self {
        let bytes: Bytes = data.into();
        let len = bytes.len() as u64;
        Self::new(
            bytes,
            RepresentationMetadata::new()
                .with_content_type(content_type)
                .with_content_length(len),
        )
    }

    /// Returns `true` if the body is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the content-type from metadata, if set.
    pub fn content_type(&self) -> Option<&str> {
        self.metadata.content_type.as_deref()
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_text_sets_content_length() {
        let r = Representation::from_text("hello", "text/plain");
        assert_eq!(r.metadata.content_length, Some(5));
        assert_eq!(r.content_type(), Some("text/plain"));
    }

    #[test]
    fn empty_representation() {
        let r = Representation::empty(RepresentationMetadata::new());
        assert!(r.is_empty());
    }
}
