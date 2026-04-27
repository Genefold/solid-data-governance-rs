//! The `KeyValueStore` trait: generic persistent key-value storage.
//!
//! Used for account data, setup flags, session tokens, etc.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use crate::error::StorageError;

/// Generic key-value store backed by any persistence mechanism.
///
/// Mirrors the TypeScript `KeyValueStorage<K, V>` interface.
#[async_trait]
pub trait KeyValueStore: Send + Sync {
    /// Retrieve the value stored under `key`, if any.
    async fn get<V: DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> Result<Option<V>, StorageError>;

    /// Store `value` under `key`, overwriting any existing entry.
    async fn set<V: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &V,
    ) -> Result<(), StorageError>;

    /// Delete the entry for `key`. No-op if the key does not exist.
    async fn delete(&self, key: &str) -> Result<(), StorageError>;

    /// Return `true` if `key` has an entry.
    async fn has(&self, key: &str) -> Result<bool, StorageError>;
}
