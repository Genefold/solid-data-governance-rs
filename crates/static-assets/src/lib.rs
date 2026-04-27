//! Static asset serving for the Solid Community Server.
//!
//! Mirrors the TypeScript `StaticAssetHandler` and `StaticAssetEntry`.

pub mod entry;
pub mod handler;

pub use entry::StaticAssetEntry;
pub use handler::StaticAssetHandler;
