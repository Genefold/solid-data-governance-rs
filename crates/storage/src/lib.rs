//! solid-storage — storage layer for the Solid Community Server (Rust port).
//!
//! Mirrors the TypeScript packages:
//!   src/storage/ResourceStore.ts              → [`ResourceStore`] + [`ResourceSet`]
//!   src/storage/keyvalue/KeyValueStorage.ts   → [`KeyValueStorage`]
//!   src/storage/keyvalue/ExpiringStorage.ts   → [`ExpiringStorage`]
//!   src/storage/keyvalue/PassthroughKeyValueStorage.ts → [`PassthroughKeyValueStorage`]
//!   src/storage/keyvalue/MemoryMapStorage.ts  → [`backends::MemoryMapStorage`]
//!   src/storage/keyvalue/JsonFileStorage.ts   → [`backends::JsonFileStorage`]
//!   src/storage/keyvalue/WrappedExpiringStorage.ts → [`backends::WrappedExpiringStorage`]
//!   src/storage/PassthroughStore.ts           → [`PassthroughStore`]
//!   src/storage/BaseResourceStore.ts          → [`BaseResourceStore`]
//!   src/storage/ReadOnlyStore.ts              → [`ReadOnlyStore`]

pub mod backends;
pub mod error;
pub mod key_value;
pub mod resource_store;

pub use backends::{ExpiresWrapper, JsonFileStorage, MemoryMapStorage, WrappedExpiringStorage};
pub use error::StorageError;
pub use key_value::{ExpiringStorage, KeyValueStorage, PassthroughKeyValueStorage, StorageEntry};
pub use resource_store::{
    BaseResourceStore, ChangeMap, PassthroughStore, ReadOnlyStore, ResourceSet, ResourceStore,
};
