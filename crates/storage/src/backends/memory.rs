//! In-memory `KeyValueStorage` implementation.
//!
//! Mirrors `src/storage/keyvalue/MemoryMapStorage.ts`:
//! ```ts
//! export class MemoryMapStorage<TValue>
//!   implements KeyValueStorage<string, TValue> {
//!   private readonly data: Map<string, TValue>;
//!   ...
//! }
//! ```
//!
//! Thread-safety is achieved with `Arc<Mutex<HashMap>>` so the type can be
//! freely cloned and shared across async tasks — the direct equivalent of
//! JavaScript's single-threaded-but-async model in the TS server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::{
    error::StorageError,
    key_value::{KeyValueStorage, StorageEntry},
};

/// A `KeyValueStorage<String, V>` backed by a `HashMap` held behind an
/// `Arc<Mutex<…>>` so the storage can be cheaply cloned and shared.
///
/// # Example
/// ```rust
/// # tokio_test::block_on(async {
/// use solid_storage::backends::MemoryMapStorage;
/// use solid_storage::key_value::KeyValueStorage;
///
/// let store: MemoryMapStorage<String> = MemoryMapStorage::new();
/// store.set("hello".to_string(), "world".to_string()).await.unwrap();
/// assert_eq!(store.get(&"hello".to_string()).await.unwrap(), Some("world".to_string()));
/// # });
/// ```
#[derive(Clone)]
pub struct MemoryMapStorage<V: Clone + Send + Sync + 'static> {
    data: Arc<Mutex<HashMap<String, V>>>,
}

impl<V: Clone + Send + Sync + 'static> Default for MemoryMapStorage<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Clone + Send + Sync + 'static> MemoryMapStorage<V> {
    /// Create a new, empty in-memory storage.
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Convenience: create pre-populated from an iterator.
    pub fn from_iter(iter: impl IntoIterator<Item = (String, V)>) -> Self {
        let store = Self::new();
        let mut guard = store.data.lock().unwrap();
        for (k, v) in iter {
            guard.insert(k, v);
        }
        drop(guard);
        store
    }
}

#[async_trait]
impl<V: Clone + Send + Sync + 'static> KeyValueStorage<String, V> for MemoryMapStorage<V> {
    async fn get(&self, key: &String) -> Result<Option<V>, StorageError> {
        let guard = self
            .data
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        Ok(guard.get(key).cloned())
    }

    async fn has(&self, key: &String) -> Result<bool, StorageError> {
        let guard = self
            .data
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        Ok(guard.contains_key(key))
    }

    /// Insert or replace the value for `key`.
    async fn set(&self, key: String, value: V) -> Result<(), StorageError> {
        let mut guard = self
            .data
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        guard.insert(key, value);
        Ok(())
    }

    /// Remove `key` and return `true` if it was present.
    async fn delete(&self, key: &String) -> Result<bool, StorageError> {
        let mut guard = self
            .data
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        Ok(guard.remove(key).is_some())
    }

    /// Collect all entries as a `Vec`.
    ///
    /// The snapshot is taken under the lock; iteration order is unspecified
    /// (mirrors the unspecified iteration order of a JS `Map` snapshot).
    async fn entries(&self) -> Result<Vec<StorageEntry<String, V>>, StorageError> {
        let guard = self
            .data
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        Ok(guard
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the `MemoryMapStorage.test.ts` suite.
    #[tokio::test]
    async fn test_get_returns_none_when_absent() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        assert_eq!(store.get(&"missing".into()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v".into()).await.unwrap();
        assert_eq!(store.get(&"k".into()).await.unwrap(), Some("v".into()));
    }

    #[tokio::test]
    async fn test_has_reflects_presence() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        assert!(!store.has(&"k".into()).await.unwrap());
        store.set("k".into(), "v".into()).await.unwrap();
        assert!(store.has(&"k".into()).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_returns_flag() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        // Deleting absent key → false
        assert!(!store.delete(&"k".into()).await.unwrap());
        store.set("k".into(), "v".into()).await.unwrap();
        // Deleting present key → true
        assert!(store.delete(&"k".into()).await.unwrap());
        // Gone now
        assert!(!store.has(&"k".into()).await.unwrap());
    }

    #[tokio::test]
    async fn test_multiple_keys() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("a".into(), "apple".into()).await.unwrap();
        store.set("b".into(), "banana".into()).await.unwrap();
        assert_eq!(store.get(&"a".into()).await.unwrap(), Some("apple".into()));
        assert_eq!(store.get(&"b".into()).await.unwrap(), Some("banana".into()));
        let mut entries = store.entries().await.unwrap();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(entries, vec![("a".into(), "apple".into()), ("b".into(), "banana".into())]);
    }
}
