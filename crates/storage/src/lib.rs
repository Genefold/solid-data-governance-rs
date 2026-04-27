//! solid-storage — storage layer for the Solid Community Server (Rust port).
//!
//! Mirrors the TypeScript packages:
//!   src/storage/ResourceStore.ts      → [`ResourceStore`] + [`ResourceSet`]
//!   src/storage/keyvalue/             → [`KeyValueStorage`] + backends
//!   src/storage/PassthroughStore.ts   → [`PassthroughStore`]
//!   src/storage/BaseResourceStore.ts  → [`BaseResourceStore`]
//!   src/storage/ReadOnlyStore.ts      → [`ReadOnlyStore`]

pub mod backends;
pub mod error;
pub mod key_value;
pub mod resource_store;

pub use error::StorageError;
pub use key_value::{KeyValueStorage, StorageEntry};
pub use resource_store::{
    BaseResourceStore, ChangeMap, PassthroughStore, ReadOnlyStore, ResourceSet, ResourceStore,
};
