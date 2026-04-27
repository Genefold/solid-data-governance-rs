//! File-backed `KeyValueStorage` using a single JSON file.
//!
//! Mirrors `src/storage/keyvalue/JsonFileStorage.ts`:
//! ```ts
//! export class JsonFileStorage<T>
//!   implements KeyValueStorage<string, T> {
//!   constructor(
//!     private readonly filePath: string,
//!     private readonly locker: ReadWriteLocker,
//!   ) {}
//! }
//! ```
//!
//! The entire key-value map is serialised as a single JSON object on every
//! write, exactly mirroring the TS implementation.  A `tokio::sync::RwLock`
//! protects concurrent access (equivalent to the `ReadWriteLocker` abstraction
//! used in the TS source).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    error::StorageError,
    key_value::{KeyValueStorage, StorageEntry},
};

/// File-backed key-value storage.
///
/// All data is kept in a single JSON file at `file_path`.  Reads acquire a
/// shared (read) lock; writes acquire an exclusive (write) lock — exactly
/// mirroring the `ReadWriteLocker` injected into `JsonFileStorage` in the TS
/// source.
///
/// `V` must implement `serde::Serialize + DeserializeOwned`.
pub struct JsonFileStorage<V> {
    file_path: PathBuf,
    lock: Arc<RwLock<()>>,
    _marker: std::marker::PhantomData<V>,
}

impl<V> JsonFileStorage<V> {
    /// Create a new `JsonFileStorage` for `file_path`.
    ///
    /// The file does not need to exist yet; it will be created on the first
    /// write.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            lock: Arc::new(RwLock::new(())),
            _marker: std::marker::PhantomData,
        }
    }

    // ── private helpers ────────────────────────────────────────────────────

    /// Read the entire JSON file and deserialise it into a `HashMap<String, Value>`.
    ///
    /// Returns an empty map if the file does not exist (new storage).
    async fn read_map(&self) -> Result<HashMap<String, Value>, StorageError> {
        match tokio::fs::read_to_string(&self.file_path).await {
            Ok(contents) => {
                let map: HashMap<String, Value> =
                    serde_json::from_str(&contents).map_err(StorageError::Json)?;
                Ok(map)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    /// Serialise `map` and atomically overwrite the JSON file.
    async fn write_map(&self, map: &HashMap<String, Value>) -> Result<(), StorageError> {
        // Create parent directories if they don't exist.
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(StorageError::Io)?;
        }
        let json = serde_json::to_string_pretty(map).map_err(StorageError::Json)?;
        tokio::fs::write(&self.file_path, json)
            .await
            .map_err(StorageError::Io)
    }
}

#[async_trait]
impl<V> KeyValueStorage<String, V> for JsonFileStorage<V>
where
    V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    async fn get(&self, key: &String) -> Result<Option<V>, StorageError> {
        let _guard = self.lock.read().await;
        let map = self.read_map().await?;
        match map.get(key) {
            None => Ok(None),
            Some(raw) => {
                let v: V = serde_json::from_value(raw.clone()).map_err(StorageError::Json)?;
                Ok(Some(v))
            }
        }
    }

    async fn has(&self, key: &String) -> Result<bool, StorageError> {
        let _guard = self.lock.read().await;
        let map = self.read_map().await?;
        Ok(map.contains_key(key))
    }

    async fn set(&self, key: String, value: V) -> Result<(), StorageError> {
        let _guard = self.lock.write().await;
        let mut map = self.read_map().await?;
        let raw = serde_json::to_value(value).map_err(StorageError::Json)?;
        map.insert(key, raw);
        self.write_map(&map).await
    }

    async fn delete(&self, key: &String) -> Result<bool, StorageError> {
        let _guard = self.lock.write().await;
        let mut map = self.read_map().await?;
        let existed = map.remove(key).is_some();
        if existed {
            self.write_map(&map).await?;
        }
        Ok(existed)
    }

    async fn entries(&self) -> Result<Vec<StorageEntry<String, V>>, StorageError> {
        let _guard = self.lock.read().await;
        let map = self.read_map().await?;
        let mut result = Vec::with_capacity(map.len());
        for (k, raw) in map {
            let v: V = serde_json::from_value(raw).map_err(StorageError::Json)?;
            result.push((k, v));
        }
        Ok(result)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store(dir: &TempDir) -> JsonFileStorage<String> {
        JsonFileStorage::new(dir.path().join("storage.json"))
    }

    /// Mirrors `JsonFileStorage.test.ts` — basic read/write/delete cycle.
    #[tokio::test]
    async fn test_read_write_delete() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        // Initially absent
        assert_eq!(store.get(&"apple".into()).await.unwrap(), None);
        assert!(!store.has(&"apple".into()).await.unwrap());
        assert!(!store.delete(&"apple".into()).await.unwrap());

        // Write
        store.set("apple".into(), "sweet".into()).await.unwrap();
        assert_eq!(store.get(&"apple".into()).await.unwrap(), Some("sweet".into()));
        assert!(store.has(&"apple".into()).await.unwrap());

        // Second key
        store.set("lemon".into(), "sour".into()).await.unwrap();
        assert_eq!(store.get(&"lemon".into()).await.unwrap(), Some("sour".into()));

        // Entries reflect both
        let mut e = store.entries().await.unwrap();
        e.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(e, vec![("apple".into(), "sweet".into()), ("lemon".into(), "sour".into())]);

        // Delete first
        assert!(store.delete(&"apple".into()).await.unwrap());
        let mut e2 = store.entries().await.unwrap();
        e2.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(e2, vec![("lemon".into(), "sour".into())]);
    }

    #[tokio::test]
    async fn test_invalid_json_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.json");
        tokio::fs::write(&path, b"this is not json").await.unwrap();
        let store: JsonFileStorage<String> = JsonFileStorage::new(&path);
        let result = store.get(&"any".into()).await;
        assert!(result.is_err());
    }
}
