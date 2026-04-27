//! Storage-layer error type.
//!
//! Maps the HTTP-level errors from the TS source into a Rust enum so that each
//! storage method can `?`-propagate and callers can match on semantics.

use thiserror::Error;

/// All errors that can occur in the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// 404 — resource was not found (NotFoundHttpError in TS).
    #[error("Not found: {0}")]
    NotFound(String),

    /// 409 — the operation would create a conflict (ConflictHttpError in TS).
    #[error("Conflict: {0}")]
    Conflict(String),

    /// 403 — the caller lacks permission (ForbiddenHttpError in TS).
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// 501 — the operation is not implemented (NotImplementedHttpError in TS).
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// 500 — unexpected internal error (InternalServerError in TS).
    #[error("Internal error: {0}")]
    Internal(String),

    /// Filesystem I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialisation failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
