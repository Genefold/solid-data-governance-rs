//! Mmap-backed binary chunk store.

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
fn sanitize(dataset_id: &str) -> String {
    dataset_id.replace(['/', '\\'], "__")
}

/// A single dataset blob mapped into memory.
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
