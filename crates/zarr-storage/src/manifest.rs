//! Manifest types describing a stored Zarr array.

use serde::{Deserialize, Serialize};

/// Describes a single chunk's location inside the dataset blob file.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkDescriptor {
    /// Logical Zarr chunk key (e.g. `"c/0/0"` for Zarr v3, or `"0.0"` for v2).
    pub key: String,
    /// Byte offset within the dataset blob.
    pub offset: u64,
    /// Length of the chunk in bytes.
    pub length: u64,
    /// Optional content checksum (hex-encoded SHA-256).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// Metadata describing a stored array.
///
/// One manifest exists per dataset and is persisted alongside the
/// mmap blob. Phase 2 will extend this with sharding metadata.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArrayManifest {
    pub dataset_id: String,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
    pub dtype: String,
    #[serde(default)]
    pub descriptors: Vec<ChunkDescriptor>,
}

impl ArrayManifest {
    /// Build an empty manifest for a new dataset.
    pub fn new(dataset_id: impl Into<String>, shape: Vec<u64>, chunk_shape: Vec<u64>, dtype: impl Into<String>) -> Self {
        Self {
            dataset_id: dataset_id.into(),
            shape,
            chunk_shape,
            dtype: dtype.into(),
            descriptors: Vec::new(),
        }
    }

    /// Locate a chunk descriptor by key.
    pub fn descriptor(&self, key: &str) -> Option<&ChunkDescriptor> {
        self.descriptors.iter().find(|d| d.key == key)
    }
}
