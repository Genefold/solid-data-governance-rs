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
//! - [`ChunkDescriptor`] records key/offset/length/checksum.
//! - [`ArrayManifest`] records the dataset id, shape, chunk shape, dtype,
//!   and the descriptor list.
//! - [`MmapChunkStore`] exposes random-access reads of arbitrary byte
//!   slices ([`MmapChunkStore::read_chunk`] for whole chunks,
//!   [`MmapChunkStore::read_chunk_range`] for HTTP Range serving) plus an
//!   admin write path ([`MmapChunkStore::write_chunk`]) for ingestion.
//! - [`ZarrV3ArrayMeta`] renders a v3-compliant `zarr.json` from the
//!   manifest at request time.
//!
//! # Architectural decisions (top-level)
//!
//! - **Two layers, one source of truth.** [`ArrayManifest`] is the
//!   only persisted description of the array. [`ZarrV3ArrayMeta`] is
//!   derived from it on each request, which keeps the on-disk format
//!   independent of the wire format and avoids dual-write bugs.
//! - **Mmap blob + sidecar manifest.** A pair of files per dataset
//!   (`.bin` + `.manifest.json`) makes Phase 0 datasets trivially
//!   backed up, copied, and inspected, while the mmap path keeps
//!   read latency dominated by page-cache hits.
//! - **Append-only writes, last-write-wins descriptors.** Crash
//!   safety is bought with disk space rather than a write-ahead log;
//!   a future `compact()` will reclaim stranded bytes.
//! - **Phase-2 extensibility is additive.** Sharding metadata, codec
//!   pipelines, and per-chunk attributes can land as new fields with
//!   `#[serde(default)]` without migrating existing datasets.
//!
//! See each module's docstring for the per-type reasoning.
//!
//! Phase 2 will extend the manifest model for sharding and indexing;
//! this crate is structured so that addition is additive.

pub mod manifest;
pub mod store;
pub mod zarr_meta;

pub use manifest::{ArrayManifest, ChunkDescriptor};
pub use store::{ChunkStoreError, MmapChunkStore};
pub use zarr_meta::ZarrV3ArrayMeta;
