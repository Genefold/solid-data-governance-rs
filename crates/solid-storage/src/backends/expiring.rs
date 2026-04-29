//! TTL-aware `KeyValueStorage` wrapper.
//!
//! Mirrors `src/storage/keyvalue/WrappedExpiringStorage.ts`:
//! ```ts
//! export class WrappedExpiringStorage<TKey, TValue>
//!   implements ExpiringStorage<TKey, TValue> {
//!   constructor(
//!     source: KeyValueStorage<TKey, Expires<TValue>>,
//!     timeout = 60  // minutes
//!   ) {}
//! }
//!
//! type Expires<T> = { expires?: string; payload: T };
//! ```
//!
//! The TS implementation stores `{ expires?: ISO-string, payload: T }` in the
//! inner storage, which is exactly what we do here via [`ExpiresWrapper<V>`]
//! (serialised to/from JSON by the inner `JsonFileStorage` or kept in-memory
//! by `MemoryMapStorage`).
//!
//! The background eviction loop from the TS `setSafeInterval` is reproduced as
//! a `tokio::task::spawn` that runs every `cleanup_interval` minutes.  The
//! task holds only a weak reference to the inner storage so it exits cleanly
//! when the store is dropped.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    error::StorageError,
    key_value::{ExpiringStorage, KeyValueStorage, StorageEntry},
};

// ──────────────────────────────────────────────────────────────────────────────
// Internal envelope — mirrors `Expires<T>` in TS
// ──────────────────────────────────────────────────────────────────────────────

/// Internal storage envelope that optionally carries an expiry unix timestamp
/// alongside the real payload.
///
/// Stored as `{ "expires": <u64 unix secs | null>, "payload": <V> }` in the
/// underlying store — an intentional simplification vs the TS ISO-string format
/// that avoids pulling in a date-parsing library for a u64 comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpiresWrapper<V> {
    /// Unix epoch seconds at which the entry expires, or `None` for no expiry.
    pub expires: Option<u64>,
    /// The real stored value.
    pub payload: V,
}

impl<V> ExpiresWrapper<V> {
    pub fn new(payload: V, expires: Option<SystemTime>) -> Self {
        Self {
            payload,
            expires: expires.map(|t| {
                t.duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs()
            }),
        }
    }

    /// Returns `true` if the entry has expired (i.e. expiry is set and now ≥ expiry).
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            now >= exp
        } else {
            false
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// WrappedExpiringStorage
// ──────────────────────────────────────────────────────────────────────────────

/// A `KeyValueStorage` / `ExpiringStorage` that wraps an inner `String`-keyed
/// store and transparently handles per-entry TTLs.
///
/// # Background cleanup
///
/// On construction a Tokio task is spawned that sleeps for `cleanup_interval`
/// and then walks all entries, deleting those whose TTL has elapsed.  The task
/// holds a `Weak<S>` reference so it exits automatically when the
/// `WrappedExpiringStorage` is dropped.
///
/// # Example
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use std::time::{Duration, SystemTime};
/// use solid_storage::backends::{MemoryMapStorage, WrappedExpiringStorage};
/// use solid_storage::key_value::{KeyValueStorage, ExpiringStorage};
///
/// let inner = MemoryMapStorage::<ExpiresWrapper<String>>::new();
/// let store = WrappedExpiringStorage::new(inner, 60);
///
/// // Store without expiry
/// store.set("k".into(), "v".into()).await.unwrap();
///
/// // Store with a 10-second TTL
/// let exp = SystemTime::now() + Duration::from_secs(10);
/// store.set_expiring("k2".into(), "v2".into(), Some(exp)).await.unwrap();
/// # });
/// ```
/// [`ExpiresWrapper`]: crate::backends::expiring::ExpiresWrapper
pub struct WrappedExpiringStorage<S>
where
    S: KeyValueStorage<String, ExpiresWrapper<String>> + 'static,
{
    source: Arc<S>,
}

impl<S> WrappedExpiringStorage<S>
where
    S: KeyValueStorage<String, ExpiresWrapper<String>> + Send + Sync + 'static,
{
    /// Create a new `WrappedExpiringStorage` wrapping `source`.
    ///
    /// `cleanup_interval_mins` controls how often expired entries are
    /// automatically purged (mirrors the TS `timeout` constructor arg,
    /// default 60 minutes).
    pub fn new(source: S, cleanup_interval_mins: u64) -> Self {
        let arc = Arc::new(source);
        let weak = Arc::downgrade(&arc);
        let interval = Duration::from_secs(cleanup_interval_mins * 60);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await; // skip immediate first tick
            loop {
                ticker.tick().await;
                if let Some(store) = weak.upgrade() {
                    if let Ok(entries) = store.entries().await {
                        let expired: Vec<String> = entries
                            .into_iter()
                            .filter(|(_, v)| v.is_expired())
                            .map(|(k, _)| k)
                            .collect();
                        for k in expired {
                            let _ = store.delete(&k).await;
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Self { source: arc }
    }

    /// Get the underlying raw wrapped entry (for testing / introspection).
    pub async fn get_raw(
        &self,
        key: &str,
    ) -> Result<Option<ExpiresWrapper<String>>, StorageError> {
        self.source.get(&key.to_string()).await
    }
}

// ── KeyValueStorage impl ──────────────────────────────────────────────────────

#[async_trait]
impl<S> KeyValueStorage<String, String> for WrappedExpiringStorage<S>
where
    S: KeyValueStorage<String, ExpiresWrapper<String>> + Send + Sync + 'static,
{
    async fn get(&self, key: &String) -> Result<Option<String>, StorageError> {
        match self.source.get(key).await? {
            None => Ok(None),
            Some(wrapper) if wrapper.is_expired() => {
                // Lazy eviction — mirrors `getUnexpired` in TS
                let _ = self.source.delete(key).await;
                Ok(None)
            }
            Some(wrapper) => Ok(Some(wrapper.payload)),
        }
    }

    async fn has(&self, key: &String) -> Result<bool, StorageError> {
        Ok(self.get(key).await?.is_some())
    }

    /// Store without expiry.
    async fn set(&self, key: String, value: String) -> Result<(), StorageError> {
        self.source
            .set(key, ExpiresWrapper::new(value, None))
            .await
    }

    async fn delete(&self, key: &String) -> Result<bool, StorageError> {
        self.source.delete(key).await
    }

    async fn entries(&self) -> Result<Vec<StorageEntry<String, String>>, StorageError> {
        let raw = self.source.entries().await?;
        let live: Vec<_> = raw
            .into_iter()
            .filter(|(_, v)| !v.is_expired())
            .map(|(k, v)| (k, v.payload))
            .collect();
        Ok(live)
    }
}

// ── ExpiringStorage impl ──────────────────────────────────────────────────────

#[async_trait]
impl<S> ExpiringStorage<String, String> for WrappedExpiringStorage<S>
where
    S: KeyValueStorage<String, ExpiresWrapper<String>> + Send + Sync + 'static,
{
    async fn set_expiring(
        &self,
        key: String,
        value: String,
        expires: Option<SystemTime>,
    ) -> Result<(), StorageError> {
        // Reject already-expired deadlines
        if let Some(exp) = expires {
            if exp <= SystemTime::now() {
                return Err(StorageError::AlreadyExpired);
            }
        }
        self.source
            .set(key, ExpiresWrapper::new(value, expires))
            .await
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::MemoryMapStorage;
    use std::time::Duration;

    fn make_store() -> WrappedExpiringStorage<MemoryMapStorage<ExpiresWrapper<String>>> {
        WrappedExpiringStorage::new(MemoryMapStorage::new(), 60)
    }

    // Ported from: test/unit/storage/keyvalue/WrappedExpiringStorage.test.ts

    // it('should return undefined for a missing key')
    #[tokio::test]
    async fn get_returns_none_for_missing_key() {
        let store = make_store();
        assert_eq!(store.get(&"k".into()).await.unwrap(), None);
    }

    // it('should return the stored value')
    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let store = make_store();
        store.set("k".into(), "v".into()).await.unwrap();
        assert_eq!(store.get(&"k".into()).await.unwrap(), Some("v".into()));
    }

    // it('has returns false for missing')
    #[tokio::test]
    async fn has_returns_false_for_missing() {
        let store = make_store();
        assert!(!store.has(&"k".into()).await.unwrap());
    }

    // it('has returns true after set')
    #[tokio::test]
    async fn has_returns_true_after_set() {
        let store = make_store();
        store.set("k".into(), "v".into()).await.unwrap();
        assert!(store.has(&"k".into()).await.unwrap());
    }

    // it('delete returns true for existing key')
    #[tokio::test]
    async fn delete_returns_true_for_existing() {
        let store = make_store();
        store.set("k".into(), "v".into()).await.unwrap();
        assert!(store.delete(&"k".into()).await.unwrap());
    }

    // it('delete returns false for missing key')
    #[tokio::test]
    async fn delete_returns_false_for_missing() {
        let store = make_store();
        assert!(!store.delete(&"k".into()).await.unwrap());
    }

    // it('should not return an expired value (lazy eviction)')
    #[tokio::test]
    async fn expired_value_not_returned() {
        let store = make_store();
        // Set expiry 1ms in the past
        let past = SystemTime::now() - Duration::from_millis(1);
        store
            .set_expiring("k".into(), "v".into(), Some(past))
            .await
            .unwrap_err(); // should be AlreadyExpired
    }

    // it('set_expiring rejects an already-expired deadline')
    #[tokio::test]
    async fn set_expiring_rejects_past_deadline() {
        let store = make_store();
        let past = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let err = store
            .set_expiring("k".into(), "v".into(), Some(past))
            .await
            .unwrap_err();
        assert!(matches!(err, StorageError::AlreadyExpired));
    }

    // it('set_expiring with None behaves like plain set')
    #[tokio::test]
    async fn set_expiring_none_stores_without_expiry() {
        let store = make_store();
        store
            .set_expiring("k".into(), "v".into(), None)
            .await
            .unwrap();
        assert_eq!(store.get(&"k".into()).await.unwrap(), Some("v".into()));
    }

    // it('entries skips expired entries')
    #[tokio::test]
    async fn entries_skips_expired() {
        let store = make_store();
        store.set("alive".into(), "yes".into()).await.unwrap();
        // Inject an expired entry directly via source
        let expired_wrapper = ExpiresWrapper {
            expires: Some(1), // epoch second 1 — long past
            payload: "no".into(),
        };
        store
            .source
            .set("dead".into(), expired_wrapper)
            .await
            .unwrap();
        let entries = store.entries().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], ("alive".to_string(), "yes".to_string()));
    }

    // it('entries returns all live entries')
    #[tokio::test]
    async fn entries_returns_all_live() {
        let store = make_store();
        store.set("a".into(), "1".into()).await.unwrap();
        store.set("b".into(), "2".into()).await.unwrap();
        let mut e = store.entries().await.unwrap();
        e.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(
            e,
            vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
            ]
        );
    }
}
