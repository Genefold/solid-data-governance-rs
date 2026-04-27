//! `KeyValueStorage` trait and helpers.
//!
//! Mirrors `src/storage/keyvalue/KeyValueStorage.ts`:
//! ```ts
//! export interface KeyValueStorage<TKey, TValue> {
//!   get(key: TKey): Promise<TValue | undefined>;
//!   has(key: TKey): Promise<boolean>;
//!   set(key: TKey, value: TValue): Promise<this>;
//!   delete(key: TKey): Promise<boolean>;
//!   entries(): AsyncIterableIterator<[TKey, TValue]>;
//! }
//! ```
//!
//! Because Rust async traits with GATs (for async iterators) are still
//! stabilising, `entries` returns a `Vec` of tuples — a faithful semantic
//! equivalent that keeps the API ergonomic and avoids nightly-only features.

use crate::error::StorageError;
use async_trait::async_trait;

/// A single key-value pair returned by [`KeyValueStorage::entries`].
pub type StorageEntry<K, V> = (K, V);

/// Simple key-value storage interface.
///
/// All implementors must be `Send + Sync` so they can be shared across
/// async task boundaries (mirrors the TypeScript world where every storage
/// is inherently concurrent-safe).
#[async_trait]
pub trait KeyValueStorage<K, V>: Send + Sync
where
    K: Send + Sync,
    V: Send + Sync,
{
    /// Returns the value stored for `key`, or `None` if absent.
    async fn get(&self, key: &K) -> Result<Option<V>, StorageError>;

    /// Returns `true` if there is a value stored for `key`.
    async fn has(&self, key: &K) -> Result<bool, StorageError>;

    /// Inserts or replaces the value for `key`.
    ///
    /// Returns `&self` (mimicking `Promise<this>` in TS) expressed here as
    /// a unit, since the caller always holds its own reference.
    async fn set(&self, key: K, value: V) -> Result<(), StorageError>;

    /// Removes the entry for `key`.
    ///
    /// Returns `true` if a value was actually deleted, `false` if the key was
    /// not present (mirrors `Promise<boolean>` in TS).
    async fn delete(&self, key: &K) -> Result<bool, StorageError>;

    /// Returns all key-value pairs currently in the storage.
    ///
    /// Mirrors `AsyncIterableIterator<[TKey, TValue]>` in TS.
    async fn entries(&self) -> Result<Vec<StorageEntry<K, V>>, StorageError>;
}
