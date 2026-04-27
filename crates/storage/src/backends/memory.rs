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

    // ── Ported from: test/unit/storage/keyvalue/MemoryMapStorage.test.ts ──

    // it('should return undefined when the key is not present')
    #[tokio::test]
    async fn get_returns_none_when_absent() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        assert_eq!(store.get(&"missing".into()).await.unwrap(), None);
    }

    // it('should return the stored value after set')
    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v".into()).await.unwrap();
        assert_eq!(store.get(&"k".into()).await.unwrap(), Some("v".into()));
    }

    // it('should overwrite when key already exists')
    #[tokio::test]
    async fn set_overwrites_existing_value() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "first".into()).await.unwrap();
        store.set("k".into(), "second".into()).await.unwrap();
        assert_eq!(
            store.get(&"k".into()).await.unwrap(),
            Some("second".into())
        );
    }

    // it('has should return false when key absent')
    #[tokio::test]
    async fn has_returns_false_when_absent() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        assert!(!store.has(&"k".into()).await.unwrap());
    }

    // it('has should return true after set')
    #[tokio::test]
    async fn has_returns_true_after_set() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v".into()).await.unwrap();
        assert!(store.has(&"k".into()).await.unwrap());
    }

    // it('has should return false after delete')
    #[tokio::test]
    async fn has_returns_false_after_delete() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v".into()).await.unwrap();
        store.delete(&"k".into()).await.unwrap();
        assert!(!store.has(&"k".into()).await.unwrap());
    }

    // it('delete returns false when key was not present')
    #[tokio::test]
    async fn delete_returns_false_for_missing_key() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        assert!(!store.delete(&"k".into()).await.unwrap());
    }

    // it('delete returns true when key was present')
    #[tokio::test]
    async fn delete_returns_true_for_existing_key() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v".into()).await.unwrap();
        assert!(store.delete(&"k".into()).await.unwrap());
    }

    // it('delete removes the entry so get returns None afterwards')
    #[tokio::test]
    async fn delete_removes_entry() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v".into()).await.unwrap();
        store.delete(&"k".into()).await.unwrap();
        assert_eq!(store.get(&"k".into()).await.unwrap(), None);
    }

    // it('entries returns all key-value pairs')
    #[tokio::test]
    async fn entries_returns_all_pairs() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("a".into(), "apple".into()).await.unwrap();
        store.set("b".into(), "banana".into()).await.unwrap();
        let mut entries = store.entries().await.unwrap();
        entries.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(
            entries,
            vec![
                ("a".to_string(), "apple".to_string()),
                ("b".to_string(), "banana".to_string()),
            ]
        );
    }

    // it('entries returns empty vec for empty store')
    #[tokio::test]
    async fn entries_empty_for_new_store() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        assert!(store.entries().await.unwrap().is_empty());
    }

    // it('entries reflects deletions')
    #[tokio::test]
    async fn entries_reflects_deletions() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("a".into(), "apple".into()).await.unwrap();
        store.set("b".into(), "banana".into()).await.unwrap();
        store.delete(&"a".into()).await.unwrap();
        let entries = store.entries().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], ("b".to_string(), "banana".to_string()));
    }

    // it('clone shares the same underlying map (Arc)')
    #[tokio::test]
    async fn clone_shares_underlying_map() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::new();
        store.set("k".into(), "v1".into()).await.unwrap();
        let clone = store.clone();
        clone.set("k".into(), "v2".into()).await.unwrap();
        // Both handles see the updated value
        assert_eq!(store.get(&"k".into()).await.unwrap(), Some("v2".into()));
        assert_eq!(clone.get(&"k".into()).await.unwrap(), Some("v2".into()));
    }

    // it('from_iter pre-populates the store')
    #[tokio::test]
    async fn from_iter_pre_populates() {
        let store: MemoryMapStorage<String> = MemoryMapStorage::from_iter([
            ("x".to_string(), "1".to_string()),
            ("y".to_string(), "2".to_string()),
        ]);
        assert_eq!(store.get(&"x".into()).await.unwrap(), Some("1".into()));
        assert_eq!(store.get(&"y".into()).await.unwrap(), Some("2".into()));
    }
}
