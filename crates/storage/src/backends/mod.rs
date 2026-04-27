//! Storage backend implementations.
//!
//! | Rust module       | TypeScript source                                  |
//! |-------------------|-------------------------------------------------   |
//! | `memory`          | `src/storage/keyvalue/MemoryMapStorage.ts`         |
//! | `file`            | `src/storage/keyvalue/JsonFileStorage.ts`          |
//! | `expiring`        | `src/storage/keyvalue/WrappedExpiringStorage.ts`   |

pub mod expiring;
pub mod file;
pub mod memory;

pub use expiring::{ExpiresWrapper, WrappedExpiringStorage};
pub use file::JsonFileStorage;
pub use memory::MemoryMapStorage;
