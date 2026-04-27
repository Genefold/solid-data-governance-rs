//! Concrete `KeyValueStorage` backends.
//!
//! | Rust type            | TypeScript equivalent           |
//! |----------------------|---------------------------------|
//! | `MemoryMapStorage`   | `MemoryMapStorage.ts`           |
//! | `JsonFileStorage`    | `JsonFileStorage.ts`            |

pub mod file;
pub mod memory;

pub use file::JsonFileStorage;
pub use memory::MemoryMapStorage;
