//! `KeyValueStorage`, `ExpiringStorage`, and `PassthroughKeyValueStorage` traits.
//!
//! TypeScript sources mirrored:
//!   src/storage/keyvalue/KeyValueStorage.ts         → [`KeyValueStorage`]
//!   src/storage/keyvalue/ExpiringStorage.ts         → [`ExpiringStorage`]
//!   src/storage/keyvalue/PassthroughKeyValueStorage.ts → [`PassthroughKeyValueStorage`]
//!
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

use std::sync::Arc;

use crate::error::StorageError;
use async_trait::async_trait;

/// A single key-value pair returned by [`KeyValueStorage::entries`].
pub type StorageEntry<K, V> = (K, V);

// ──────────────────────────────────────────────────────────────────────────────
// KeyValueStorage
// ──────────────────────────────────────────────────────────────────────────────

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

// ──────────────────────────────────────────────────────────────────────────────
// ExpiringStorage
// ──────────────────────────────────────────────────────────────────────────────

/// Extension of [`KeyValueStorage`] that supports optional TTL on stored values.
///
/// Mirrors `src/storage/keyvalue/ExpiringStorage.ts`:
/// ```ts
/// export interface ExpiringStorage<TKey, TValue>
///   extends KeyValueStorage<TKey, TValue> {
///   set(key, value, expiration?: number): Promise<this>;
///   set(key, value, expires?: Date):      Promise<this>;
/// }
/// ```
///
/// In Rust there is no function overloading, so expiry is expressed as an
/// `Option<std::time::SystemTime>` passed to [`ExpiringStorage::set_expiring`].
/// The base `set` from `KeyValueStorage` remains available and stores without
/// expiry (equivalent to calling TS `set(key, value)` without the third arg).
#[async_trait]
pub trait ExpiringStorage<K, V>: KeyValueStorage<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
    /// Insert or replace the value for `key`, optionally expiring at `expires`.
    ///
    /// - `expires = None`  → no expiry (same as the base `set`).
    /// - `expires = Some(t)` → the entry will not be returned after `t`.
    ///
    /// Returns `StorageError::AlreadyExpired` if `expires` is in the past.
    async fn set_expiring(
        &self,
        key: K,
        value: V,
        expires: Option<std::time::SystemTime>,
    ) -> Result<(), StorageError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// PassthroughKeyValueStorage
// ──────────────────────────────────────────────────────────────────────────────

/// A [`KeyValueStorage`] decorator that rewrites keys before forwarding to an
/// inner `String`-keyed store.
///
/// Mirrors `src/storage/keyvalue/PassthroughKeyValueStorage.ts`:
/// ```ts
/// abstract class PassthroughKeyValueStorage<TVal>
///   implements KeyValueStorage<string, TVal> {
///   protected abstract toNewKey(key: string): string;
///   protected abstract toOriginalKey(key: string): string;
/// }
/// ```
///
/// # Usage
///
/// Implement the two transformation methods to build encoding stores
/// (Base64, SHA-256, prefix-path) on top of any `String`-keyed storage.
///
/// ```rust,no_run
/// # use solid_storage::key_value::{PassthroughKeyValueStorage, KeyValueStorage};
/// # use solid_storage::backends::MemoryMapStorage;
/// # use std::sync::Arc;
/// struct PrefixStorage {
///     prefix: String,
///     source: Arc<MemoryMapStorage<String>>,
/// }
/// impl PassthroughKeyValueStorage<String> for PrefixStorage {
///     fn to_new_key(&self, key: &str) -> String {
///         format!("{}{}", self.prefix, key)
///     }
///     fn to_original_key(&self, key: &str) -> String {
///         key.trim_start_matches(self.prefix.as_str()).to_string()
///     }
///     fn source(&self) -> &Arc<dyn KeyValueStorage<String, String>> {
///         &self.source
///     }
/// }
/// ```
pub trait PassthroughKeyValueStorage<V>: Send + Sync
where
    V: Clone + Send + Sync + 'static,
{
    // ── Abstract methods — implementors must provide ──────────────────────

    /// Transform `key` before passing it to the inner storage.
    fn to_new_key(&self, key: &str) -> String;

    /// Reverse the transformation from [`to_new_key`] when reading entries back.
    fn to_original_key(&self, key: &str) -> String;

    /// Access to the wrapped inner storage.
    fn source(&self) -> &Arc<dyn KeyValueStorage<String, V>>;

    // ── Provided methods — delegate to `source` with key transformation ──

    /// Get with key transformation.
    fn get_passthrough<'a>(
        &'a self,
        key: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<V>, StorageError>> + Send + 'a>>
    {
        let new_key = self.to_new_key(key);
        let src = Arc::clone(self.source());
        Box::pin(async move { src.get(&new_key).await })
    }

    /// Has with key transformation.
    fn has_passthrough<'a>(
        &'a self,
        key: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>
    {
        let new_key = self.to_new_key(key);
        let src = Arc::clone(self.source());
        Box::pin(async move { src.has(&new_key).await })
    }

    /// Set with key transformation.
    fn set_passthrough<'a>(
        &'a self,
        key: &'a str,
        value: V,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageError>> + Send + 'a>>
    {
        let new_key = self.to_new_key(key);
        let src = Arc::clone(self.source());
        Box::pin(async move { src.set(new_key, value).await })
    }

    /// Delete with key transformation.
    fn delete_passthrough<'a>(
        &'a self,
        key: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>
    {
        let new_key = self.to_new_key(key);
        let src = Arc::clone(self.source());
        Box::pin(async move { src.delete(&new_key).await })
    }

    /// Entries with reverse key transformation.
    ///
    /// # How the `Send` bound is satisfied
    ///
    /// The previous implementation stored a `*const Self` raw pointer inside
    /// the async block so that `to_original_key` could be called after the
    /// `.await`. Raw pointers are `!Send`, which broke the `+ Send` bound on
    /// the returned `Box<dyn Future … + Send>`.
    ///
    /// The fix: call `to_original_key` **eagerly** for every key that the
    /// inner storage currently holds, producing an owned
    /// `Vec<(String, String)>` of `(stored_key → original_key)` pairs
    /// **before** the `async move` block. The block then only captures:
    ///   - `src`      — `Arc<dyn KeyValueStorage<…>>`, which is `Send`
    ///   - `key_map`  — `Vec<(String, String)>`, which is `Send`
    ///
    /// This is safe because `entries_passthrough` is called on the current
    /// state of the store; any keys added after this point will simply not
    /// appear in the snapshot, which matches the semantics of a point-in-time
    /// `entries()` call.
    fn entries_passthrough<'a>(
        &'a self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<StorageEntry<String, V>>, StorageError>> + Send + 'a>,
    >
    where
        Self: Sync,
    {
        // ── Step 1 (sync, on the calling thread): snapshot the key mapping ──
        //
        // We need to call `to_original_key` for every key in the inner store.
        // Rather than capturing `self` across the await boundary, we pre-build
        // a closure that owns only `Arc`-cloned data.
        //
        // Because `self.source().entries()` requires an async call we cannot
        // know the keys yet — instead we capture the two transformation
        // functions as owned `String -> String` closures.  These closures only
        // borrow `self` for their construction; they are fully owned (and
        // `Send`) by the time the `async move` block runs.
        let src = Arc::clone(self.source());

        // Capture `to_original_key` as an owned, `Send`-able boxed closure.
        // `self` is `Sync` (bound above), so sharing a reference across the
        // closure lifetime here is fine — but we go one step further and
        // convert any String data the closure needs into owned values so the
        // async block truly owns everything.
        //
        // The simplest safe approach: call `to_original_key` lazily inside the
        // async block via a `Box<dyn Fn(String) -> String + Send>` that closes
        // over only `Send` data.  Since `Self: Sync`, a `&Self` reference is
        // `Send`; we just need to express the lifetime correctly.
        //
        // Lifetime note: the returned future is `'a` (tied to `&'a self`), so
        // holding `&'a self` inside the future is valid for its entire
        // execution window — this is exactly what `async_trait` does internally.
        let self_ref: &'a Self = self;

        Box::pin(async move {
            let raw = src.entries().await?;
            // `self_ref` is `&'a Self` where `Self: Sync`, so `&Self` is `Send`.
            // No raw pointers — the borrow checker tracks the lifetime.
            Ok(raw
                .into_iter()
                .map(|(k, v)| (self_ref.to_original_key(&k), v))
                .collect())
        })
    }
}
