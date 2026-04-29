//! Manifest types describing a stored Zarr array.
//!
//! # Architectural decisions
//!
//! **Why a sidecar manifest instead of relying on the Zarr v3 hierarchy?**
//! Zarr v3 stores each chunk as an independent key in the underlying
//! store, with metadata derived from `zarr.json` and chunk-grid math.
//! Phase 0 packs many chunks into a single blob file and uses an
//! external manifest to map *logical* chunk keys to *physical*
//! `(offset, length)` slices. This decouples on-disk layout from the
//! logical Zarr addressing model: chunks can be reordered, compacted,
//! or sharded later (Phase 2) without rewriting consumers, and the
//! HTTP layer can answer `Range` requests with a single mmap slice.
//!
//! **Why a flat `Vec<ChunkDescriptor>` instead of a `HashMap`?**
//! Phase 0 datasets are small (tens to low thousands of chunks) and
//! the manifest is rewritten on every chunk write. A flat vector
//! keeps the on-disk JSON stable and diff-friendly, preserves
//! insertion order for debugging, and round-trips cleanly through
//! `serde_json` without a custom map serializer. If lookup ever
//! becomes hot we will add an in-memory index alongside the vector
//! rather than changing the persisted shape.
//!
//! **Why no compression / codec metadata here?**
//! Codec configuration is a *Zarr-level* concern and lives in
//! [`crate::zarr_meta::ZarrV3ArrayMeta`]. The manifest only describes
//! how raw chunk bytes are laid out on disk; whether those bytes are
//! compressed is opaque to the store.

use serde::{Deserialize, Serialize};

/// Describes a single chunk's location inside the dataset blob file.
///
/// # Design notes
///
/// - **`key` is the logical Zarr chunk key**, not a filesystem path.
///   For Zarr v3 with the default key encoding this looks like
///   `"c/0/0"`; for v2 it would be `"0.0"`. Storing the encoded key
///   verbatim means the HTTP layer can compare what it received from
///   the URL against what is stored without re-deriving indices.
/// - **`offset`/`length` are `u64`** so individual chunks can exceed
///   4 GiB on 64-bit platforms. Phase 0 chunks are far smaller, but
///   sharded layouts in Phase 2 will reuse this descriptor type for
///   shard slices that easily cross that boundary.
/// - **`checksum` is optional** because some ingestion paths
///   (streaming uploads) may not have a checksum at the time the
///   descriptor is created. When present it is hex-encoded SHA-256
///   over the *raw* on-disk bytes, so callers can verify integrity
///   without re-decoding through any codec.
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
/// mmap blob (`<dataset>.manifest.json`). Phase 2 will extend this with
/// sharding metadata.
///
/// # Architectural decisions
///
/// - **Single source of truth for shape/dtype.** The manifest, not
///   `zarr.json`, is authoritative. `ZarrV3ArrayMeta` is *derived*
///   from this struct on read, which means callers can freely change
///   how `zarr.json` is rendered (e.g., to support v2 compatibility)
///   without migrating stored manifests.
/// - **Embedded `dataset_id`.** Storing the id inside the manifest
///   makes the file self-describing — useful when manifests are
///   copied between pods or recovered from backup, where the
///   filesystem name may have been rewritten by `sanitize`.
/// - **`Vec<u64>` for shape and chunk shape.** N-dimensional arrays
///   require a runtime-sized rank; we keep both vectors and let the
///   Zarr metadata layer enforce that they are equal length.
/// - **`#[serde(default)]` on descriptors.** Newly-created datasets
///   start with no chunks, so the field is omitted from disk for
///   freshly registered datasets and added on first write — cleaner
///   diffs and a smaller "empty dataset" on disk.
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
    ///
    /// Constructing through this helper (rather than struct-literal
    /// init) means future required fields can be added without
    /// breaking call sites — `register_dataset` paths in the
    /// governance crate go through here.
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
    ///
    /// Linear scan is intentional: see the module-level note on the
    /// `Vec` vs `HashMap` decision. For Phase 0 dataset sizes the
    /// scan is dominated by the surrounding I/O and lock acquisition.
    pub fn descriptor(&self, key: &str) -> Option<&ChunkDescriptor> {
        self.descriptors.iter().find(|d| d.key == key)
    }
}
