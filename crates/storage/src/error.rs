//! Error type for the storage layer.

use thiserror::Error;

/// All errors that can occur in the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Resource or key not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Write rejected on a read-only store.
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Method not implemented (mirrors `BaseResourceStore` throwing).
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// A TTL deadline that is already in the past was passed to `set_expiring`.
    ///
    /// Mirrors `InternalServerError('Value is already expired')` in TS.
    #[error("The supplied expiry deadline is already in the past")]
    AlreadyExpired,

    /// Low-level I/O error (file backend).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialisation / deserialisation error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for internal errors.
    #[error("Internal error: {0}")]
    Internal(String),
}
