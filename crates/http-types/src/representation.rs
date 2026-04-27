//! Resource representation: data stream + metadata.

use crate::metadata::RepresentationMetadata;
use bytes::Bytes;

/// A resource representation: its byte content and associated metadata.
///
/// Analogous to `BasicRepresentation` in the TypeScript server.
#[derive(Debug)]
pub struct Representation {
    pub data: Bytes,
    pub metadata: RepresentationMetadata,
}

impl Representation {
    pub fn new(data: impl Into<Bytes>, metadata: RepresentationMetadata) -> Self {
        Self {
            data: data.into(),
            metadata,
        }
    }

    pub fn empty(metadata: RepresentationMetadata) -> Self {
        Self {
            data: Bytes::new(),
            metadata,
        }
    }
}
