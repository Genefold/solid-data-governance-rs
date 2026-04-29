//! Local Zarr v3 chunk storage.
//!
//! This crate owns both chunk layout and metadata address resolution for
//! Phase 0. Chunks for a dataset live in a single mmap-backed binary file
//! (`<dataset>.bin`); a sibling manifest (`<dataset>.manifest.json`)
//! records the byte offset, length, and (optional) checksum for each
//! Zarr chunk key, plus the array's shape, chunk shape, and dtype.
//!
//! The design intentionally mirrors the Phase 0 plan:
//!
//! - `ChunkDescriptor` records key/offset/length/checksum.
//! - `ArrayManifest` records the dataset id, shape, chunk shape, dtype,
//!   and the descriptor list.
//! - `MmapChunkStore` exposes random-access reads of arbitrary byte
//!   slices (`read_chunk` for whole chunks, `read_range` for HTTP Range
//!   serving) plus an admin write path (`write_chunk`) for ingestion.
//!
//! Phase 2 will extend the manifest model for sharding and indexing;
//! this crate is structured so that addition is additive.

pub mod manifest;
pub mod store;
pub mod zarr_meta;

pub use manifest::{ArrayManifest, ChunkDescriptor};
pub use store::{ChunkStoreError, MmapChunkStore};
pub use zarr_meta::ZarrV3ArrayMeta;
