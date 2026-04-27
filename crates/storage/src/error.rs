//! Storage-layer errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    #[error("Not a directory: {0}")]
    NotDirectory(String),

    #[error("Not a file: {0}")]
    NotFile(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("{0}")]
    Other(String),
}
