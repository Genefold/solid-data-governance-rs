//! Storage abstractions for the Solid Community Server.
//!
//! Provides the `ResourceStore` and `KeyValueStore` traits together with
//! concrete in-memory and file-backed implementations.

pub mod error;
pub mod key_value;
pub mod resource_store;

pub mod backends {
    pub mod memory;
    pub mod file;
}

pub use key_value::KeyValueStore;
pub use resource_store::ResourceStore;
