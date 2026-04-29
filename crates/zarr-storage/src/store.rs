//! Mmap-backed binary chunk store.
//!
//! # Architectural decisions
//!
//! **Why mmap?** Zarr access patterns are read-heavy and random:
//! clients open an array and stream individual chunks (or chunk
//! sub-ranges via HTTP `Range`). Memory-mapping the dataset blob
//! lets the kernel page-cache do the heavy lifting — we hand the
//! OS a slice and it returns the bytes — with zero user-space
//! copy until the final `to_vec()` for the HTTP body. Compared to
//! per-request `pread`, mmap also amortises file-open cost and
//! plays nicely with hot chunks staying resident.
//!
//! **Why one blob per dataset?** Phase 0 prioritises a few large
//! sequential writes (`write_chunk` appends) over many small files
//! (the v3 "one chunk = one key" model). One blob means one mmap
//! per dataset, contiguous I/O during ingestion, and trivial backup
//! semantics (`<id>.bin` + `<id>.manifest.json` is the entire dataset).
//! When sharding lands in Phase 2 we will keep the same shape but
//! address shard slices through the same descriptor type.
//!
//! **Why an in-memory `RwLock<HashMap<id, DatasetState>>` cache?**
//! Opening a mmap and parsing the manifest is non-trivial; caching
//! the `DatasetState` keyed by id lets repeated chunk reads skip
//! that work. The `RwLock` is read-mostly: reads take the read
//! guard, writes (chunk append) take the write guard *and* re-mmap.
//! This means a `MmapChunkStore` is the single coordinator for a
//! root directory — do not open two stores against the same root
//! from different processes without external locking, and tests
//! that need to observe mutations made through one store should
//! reuse the same instance rather than constructing a second one.
//!
//! **Why re-mmap on every write?** Appending to the file invalidates
//! the previous mapping's view of the tail (mmap length is captured
//! at `Mmap::map` time). Phase 0 writes are administrative
//! (ingestion) and rare relative to reads, so the simplicity of
//! "write → fsync → re-map" beats a more complex remap-on-grow
//! scheme. Hot read paths never re-mmap.

use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::RwLock,
};

use memmap2::Mmap;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, info};

use crate::manifest::{ArrayManifest, ChunkDescriptor};

/// Errors returned by the chunk store.
///
/// # Design notes
///
/// Errors are split into *transport* (I/O, JSON) and *semantic*
/// (chunk-not-found, dataset-not-found, invalid-range) variants so
/// the HTTP layer can map them directly to status codes:
///
/// - [`Self::DatasetNotFound`] / [`Self::ChunkNotFound`] → `404`
/// - [`Self::InvalidRange`] → `416 Range Not Satisfiable`
/// - [`Self::Io`] / [`Self::Json`] → `500`
///
/// Keeping these distinct (rather than a single `String` error)
/// also makes the integration tests assert on the variant rather
/// than scraping error messages.
#[derive(Debug, Error)]
pub enum ChunkStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("chunk not found: {0}")]
    ChunkNotFound(String),
    #[error("invalid byte range {start}..{end} for blob of length {len}")]
    InvalidRange { start: u64, end: u64, len: u64 },
    #[error("dataset not found: {0}")]
    DatasetNotFound(String),
}

/// Names used for the per-dataset blob and manifest files.
fn blob_path(root: &Path, dataset_id: &str) -> PathBuf {
    root.join(format!("{}.bin", sanitize(dataset_id)))
}
fn manifest_path(root: &Path, dataset_id: &str) -> PathBuf {
    root.join(format!("{}.manifest.json", sanitize(dataset_id)))
}

/// Replace path-unsafe characters in dataset ids so they can become file
/// names. Datasets are typically `org/name`, so `/` becomes `__`.
///
/// # Why `__` (double underscore)?
///
/// We need an escape that:
///
/// 1. Survives every common filesystem (no slashes, colons, or backslashes).
/// 2. Is unlikely to appear in real-world `org/dataset` ids.
/// 3. Is reversible without ambiguity — [`MmapChunkStore::list_datasets`]
///    relies on `replace("__", "/")` to recover the logical id.
///
/// Double underscore satisfies all three. If we ever need to support
/// ids that legitimately contain `__`, we will move to a percent-style
/// encoding and bump the on-disk layout version.
fn sanitize(dataset_id: &str) -> String {
    dataset_id.replace(['/', '\\'], "__")
}

/// A single dataset blob mapped into memory.
///
/// # Architectural decisions
///
/// - **`Option<Mmap>` for the empty case.** macOS and Windows reject
///   `mmap` of a zero-length file; we therefore lazily create the
///   mapping and let `slice` return `InvalidRange` until the first
///   write grows the file. Holding `None` (rather than a 0-length
///   `Mmap`) keeps the type-level invariant that an existing mmap
///   always covers at least one byte.
/// - **`_file` retained for lifetime.** `Mmap` borrows the file's
///   address space; dropping the `File` early would unmap on some
///   platforms. The leading underscore signals "intentionally unused
///   field, kept for RAII".
/// - **No interior locking.** Concurrency is managed by the parent
///   `MmapChunkStore`'s `RwLock`; `MappedBlob` itself is purely a
///   value type for read-only slicing.
struct MappedBlob {
    /// Live mmap. `None` when the blob file is empty (mmap of length 0
    /// is not allowed on most platforms).
    mmap: Option<Mmap>,
    /// Underlying file kept open so the mmap stays valid.
    _file: File,
}

impl MappedBlob {
    fn open(path: &Path) -> Result<Self, ChunkStoreError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        let len = file.metadata()?.len();
        let mmap = if len == 0 {
            None
        } else {
            // SAFETY: file is opened for the lifetime of MappedBlob and
            // the platform guarantees that subsequent writes through
            // the same file handle will be visible after re-mapping.
            Some(unsafe { Mmap::map(&file)? })
        };
        Ok(Self { mmap, _file: file })
    }

    fn slice(&self, offset: u64, length: u64) -> Result<&[u8], ChunkStoreError> {
        let mmap = self.mmap.as_ref().ok_or(ChunkStoreError::InvalidRange {
            start: offset,
            end: offset + length,
            len: 0,
        })?;
        let start = offset as usize;
        let end = start
            .checked_add(length as usize)
            .ok_or(ChunkStoreError::InvalidRange {
                start: offset,
                end: offset.saturating_add(length),
                len: mmap.len() as u64,
            })?;
        if end > mmap.len() {
            return Err(ChunkStoreError::InvalidRange {
                start: offset,
                end: end as u64,
                len: mmap.len() as u64,
            });
        }
        Ok(&mmap[start..end])
    }

    fn len(&self) -> u64 {
        self.mmap.as_ref().map(|m| m.len() as u64).unwrap_or(0)
    }
}

/// One mapped dataset: blob + in-memory manifest.
///
/// Pairs the parsed [`ArrayManifest`] with the live [`MappedBlob`]
/// so a single map lookup hands the caller everything needed to
/// resolve a chunk key into a byte slice. The two are kept in sync
/// inside the `MmapChunkStore` write path: blob is appended first,
/// then re-mapped, then the manifest is mutated and persisted, so a
/// crash mid-write leaves an over-long blob (harmless, the manifest
/// never points at the trailing bytes) rather than a manifest that
/// references unwritten bytes.
struct DatasetState {
    manifest: ArrayManifest,
    blob: MappedBlob,
}

/// Mmap-backed chunk store rooted at a single directory.
///
/// Each dataset becomes two sibling files:
///   * `<sanitized_id>.bin` — the concatenated chunk blob.
///   * `<sanitized_id>.manifest.json` — chunk descriptors and array meta.
///
/// All reads go through mmap; writes append to the blob and rewrite the
/// manifest. The store is safe to share across threads.
///
/// # Architectural decisions
///
/// - **Single root directory.** A pod hosts one chunk store rooted
///   at `--data-dir` (default `./.pod-data`). Datasets are
///   namespaced by id, not directory — keeping the layout flat
///   means `list_datasets` is a single `read_dir` and avoids the
///   need to walk arbitrary depths or interpret directory names as
///   org boundaries.
/// - **Single-process ownership.** The in-memory cache is the
///   authoritative view for the lifetime of the process. Two
///   processes pointing at the same root will race on writes.
///   Phase 0 deployments run a single API replica; horizontal scale
///   is a Phase 3 concern that will introduce an external lock or
///   object-store backend.
/// - **No file-system locking.** We rely on the parent `RwLock` to
///   serialize writers within a process; we deliberately do *not*
///   take `flock` on the blob, both for portability (Windows lacks
///   POSIX `flock` semantics) and because cross-process safety is
///   out of scope for Phase 0.
/// - **Append-only blob.** `write_chunk` always seeks to end and
///   appends. Re-writing an existing key leaves the previous bytes
///   stranded inside the blob (the descriptor is updated to point
///   at the new offset). This trades disk space for crash safety
///   and write simplicity; a future `compact()` admin op can
///   reclaim space when needed.
pub struct MmapChunkStore {
    root: PathBuf,
    datasets: RwLock<std::collections::HashMap<String, DatasetState>>,
}

impl MmapChunkStore {
    /// Open or create a store rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ChunkStoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        info!(path = %root.display(), "MmapChunkStore opened");
        Ok(Self {
            root,
            datasets: RwLock::new(Default::default()),
        })
    }

    fn ensure_loaded(&self, dataset_id: &str) -> Result<(), ChunkStoreError> {
        {
            let map = self.datasets.read().unwrap();
            if map.contains_key(dataset_id) {
                return Ok(());
            }
        }
        let manifest_p = manifest_path(&self.root, dataset_id);
        if !manifest_p.exists() {
            return Err(ChunkStoreError::DatasetNotFound(dataset_id.to_owned()));
        }
        let manifest_bytes = std::fs::read(&manifest_p)?;
        let manifest: ArrayManifest = serde_json::from_slice(&manifest_bytes)?;
        let blob = MappedBlob::open(&blob_path(&self.root, dataset_id))?;
        debug!(dataset = %dataset_id, blob_len = blob.len(), "dataset loaded");
        let mut map = self.datasets.write().unwrap();
        map.insert(dataset_id.to_owned(), DatasetState { manifest, blob });
        Ok(())
    }

    /// Check whether a dataset exists on disk.
    pub fn has_dataset(&self, dataset_id: &str) -> bool {
        manifest_path(&self.root, dataset_id).exists()
    }

    /// Create a new (empty) dataset and persist its manifest.
    pub fn create_dataset(&self, manifest: ArrayManifest) -> Result<(), ChunkStoreError> {
        let dataset_id = manifest.dataset_id.clone();
        let manifest_p = manifest_path(&self.root, &dataset_id);
        let blob_p = blob_path(&self.root, &dataset_id);
        std::fs::write(&manifest_p, serde_json::to_vec_pretty(&manifest)?)?;
        // Touch the blob so it exists for later mmap.
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&blob_p)?;
        let blob = MappedBlob::open(&blob_p)?;
        info!(dataset = %dataset_id, "dataset created");
        let mut map = self.datasets.write().unwrap();
        map.insert(dataset_id, DatasetState { manifest, blob });
        Ok(())
    }

    /// Append bytes for a chunk and update the manifest.
    ///
    /// This re-maps the blob after writing so subsequent reads see the
    /// new bytes. Concurrent writers are serialized by the dataset
    /// write lock.
    ///
    /// # Ordering guarantees
    ///
    /// The on-disk write order is fixed and intentional:
    ///
    /// 1. Append bytes to the blob (`seek(End) + write_all + flush`).
    /// 2. Re-mmap the blob so subsequent reads see the new tail.
    /// 3. Update the in-memory descriptor list (last-write-wins on
    ///    duplicate keys).
    /// 4. Persist the updated manifest.
    ///
    /// A crash between (1) and (4) leaves an over-long blob whose
    /// trailing bytes the manifest never references — safe but
    /// wastes space. A crash *during* (4) is mitigated by
    /// `serde_json::to_vec_pretty`'s buffered write; a torn write
    /// would surface as a parse error on next open and require
    /// operator intervention. Phase 2 will introduce a `manifest.tmp`
    /// + `rename` dance once we observe this in the wild.
    pub fn write_chunk(
        &self,
        dataset_id: &str,
        chunk_key: &str,
        bytes: &[u8],
    ) -> Result<ChunkDescriptor, ChunkStoreError> {
        self.ensure_loaded(dataset_id)?;
        let mut map = self.datasets.write().unwrap();
        let state = map
            .get_mut(dataset_id)
            .ok_or_else(|| ChunkStoreError::DatasetNotFound(dataset_id.to_owned()))?;
        let blob_p = blob_path(&self.root, dataset_id);

        // Append to blob.
        let mut f = OpenOptions::new().read(true).write(true).open(&blob_p)?;
        let offset = f.seek(SeekFrom::End(0))?;
        f.write_all(bytes)?;
        f.flush()?;
        drop(f);

        // Re-map blob.
        state.blob = MappedBlob::open(&blob_p)?;

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let checksum = hex::encode(hasher.finalize());

        let descriptor = ChunkDescriptor {
            key: chunk_key.to_owned(),
            offset,
            length: bytes.len() as u64,
            checksum: Some(checksum),
        };
        // Replace existing descriptor with the same key (last write wins).
        state.manifest.descriptors.retain(|d| d.key != chunk_key);
        state.manifest.descriptors.push(descriptor.clone());

        // Persist manifest.
        std::fs::write(
            manifest_path(&self.root, dataset_id),
            serde_json::to_vec_pretty(&state.manifest)?,
        )?;
        debug!(
            dataset = %dataset_id,
            key = %chunk_key,
            offset,
            length = bytes.len(),
            "chunk written"
        );
        Ok(descriptor)
    }

    /// Read a single chunk in full.
    pub fn read_chunk(&self, dataset_id: &str, chunk_key: &str) -> Result<Vec<u8>, ChunkStoreError> {
        self.ensure_loaded(dataset_id)?;
        let map = self.datasets.read().unwrap();
        let state = map
            .get(dataset_id)
            .ok_or_else(|| ChunkStoreError::DatasetNotFound(dataset_id.to_owned()))?;
        let descriptor = state
            .manifest
            .descriptor(chunk_key)
            .ok_or_else(|| ChunkStoreError::ChunkNotFound(chunk_key.to_owned()))?
            .clone();
        let slice = state.blob.slice(descriptor.offset, descriptor.length)?;
        Ok(slice.to_vec())
    }

    /// Read an arbitrary byte range within a chunk (for HTTP Range serving).
    ///
    /// `range` is interpreted as `[start, end)` relative to the start of
    /// the chunk's bytes, not the blob.
    ///
    /// # Why chunk-relative, not blob-relative?
    ///
    /// HTTP `Range` headers from `zarr-python` and friends address
    /// bytes within a single object (the chunk URL). Translating
    /// them at the store boundary keeps the HTTP layer ignorant of
    /// the blob packing scheme: callers say "give me bytes 1024..2048
    /// of chunk `c/0/3`" and the store internally adds the chunk's
    /// blob offset. This is the same boundary at which a future
    /// object-store backend would translate the request into an S3
    /// `Range: bytes=...` header against a per-chunk key.
    pub fn read_chunk_range(
        &self,
        dataset_id: &str,
        chunk_key: &str,
        start: u64,
        end: u64,
    ) -> Result<Vec<u8>, ChunkStoreError> {
        if end <= start {
            return Err(ChunkStoreError::InvalidRange {
                start,
                end,
                len: 0,
            });
        }
        self.ensure_loaded(dataset_id)?;
        let map = self.datasets.read().unwrap();
        let state = map
            .get(dataset_id)
            .ok_or_else(|| ChunkStoreError::DatasetNotFound(dataset_id.to_owned()))?;
        let descriptor = state
            .manifest
            .descriptor(chunk_key)
            .ok_or_else(|| ChunkStoreError::ChunkNotFound(chunk_key.to_owned()))?
            .clone();
        if end > descriptor.length {
            return Err(ChunkStoreError::InvalidRange {
                start,
                end,
                len: descriptor.length,
            });
        }
        let slice = state
            .blob
            .slice(descriptor.offset + start, end - start)?;
        Ok(slice.to_vec())
    }

    /// Length of a chunk in bytes.
    pub fn chunk_length(&self, dataset_id: &str, chunk_key: &str) -> Result<u64, ChunkStoreError> {
        self.ensure_loaded(dataset_id)?;
        let map = self.datasets.read().unwrap();
        let state = map
            .get(dataset_id)
            .ok_or_else(|| ChunkStoreError::DatasetNotFound(dataset_id.to_owned()))?;
        Ok(state
            .manifest
            .descriptor(chunk_key)
            .ok_or_else(|| ChunkStoreError::ChunkNotFound(chunk_key.to_owned()))?
            .length)
    }

    /// Borrow a clone of the manifest.
    pub fn manifest(&self, dataset_id: &str) -> Result<ArrayManifest, ChunkStoreError> {
        self.ensure_loaded(dataset_id)?;
        let map = self.datasets.read().unwrap();
        Ok(map
            .get(dataset_id)
            .ok_or_else(|| ChunkStoreError::DatasetNotFound(dataset_id.to_owned()))?
            .manifest
            .clone())
    }

    /// List all dataset ids known to the store (by walking the directory).
    pub fn list_datasets(&self) -> Result<Vec<String>, ChunkStoreError> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(stem) = name.strip_suffix(".manifest.json") {
                ids.push(stem.replace("__", "/"));
            }
        }
        ids.sort();
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zarr-storage-test-{}", uuid_like()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        format!("{nanos}")
    }

    #[test]
    fn create_write_read_roundtrip() {
        let dir = tempdir();
        let store = MmapChunkStore::open(&dir).unwrap();
        let manifest = ArrayManifest::new("org-a/demo", vec![4, 4], vec![2, 2], "<f4");
        store.create_dataset(manifest).unwrap();

        let chunk = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let desc = store.write_chunk("org-a/demo", "c/0/0", &chunk).unwrap();
        assert_eq!(desc.length, 10);
        assert_eq!(desc.offset, 0);

        let read = store.read_chunk("org-a/demo", "c/0/0").unwrap();
        assert_eq!(read, chunk);

        let range = store.read_chunk_range("org-a/demo", "c/0/0", 2, 6).unwrap();
        assert_eq!(range, vec![3, 4, 5, 6]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_chunk_errors() {
        let dir = tempdir();
        let store = MmapChunkStore::open(&dir).unwrap();
        store
            .create_dataset(ArrayManifest::new("d", vec![1], vec![1], "<f4"))
            .unwrap();
        match store.read_chunk("d", "c/0").unwrap_err() {
            ChunkStoreError::ChunkNotFound(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_datasets_roundtrip() {
        let dir = tempdir();
        let store = MmapChunkStore::open(&dir).unwrap();
        store
            .create_dataset(ArrayManifest::new("org-a/x", vec![1], vec![1], "<f4"))
            .unwrap();
        store
            .create_dataset(ArrayManifest::new("org-a/y", vec![1], vec![1], "<f4"))
            .unwrap();
        let ids = store.list_datasets().unwrap();
        assert_eq!(ids, vec!["org-a/x".to_owned(), "org-a/y".to_owned()]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
