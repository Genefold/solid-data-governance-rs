//! Typed HTTP error hierarchy mirroring the TypeScript HttpError tree.

use thiserror::Error;

/// Top-level error type for all Solid server errors.
#[derive(Debug, Error)]
pub enum SolidError {
    #[error("400 Bad Request: {0}")]
    BadRequest(String),

    #[error("401 Unauthorized: {0}")]
    Unauthorized(String),

    #[error("403 Forbidden: {0}")]
    Forbidden(String),

    #[error("404 Not Found: {0}")]
    NotFound(String),

    #[error("405 Method Not Allowed: {0}")]
    MethodNotAllowed(String),

    #[error("409 Conflict: {0}")]
    Conflict(String),

    #[error("412 Precondition Failed: {0}")]
    PreconditionFailed(String),

    #[error("304 Not Modified")]
    NotModified,

    #[error("500 Internal Server Error: {0}")]
    Internal(String),

    #[error("501 Not Implemented: {0}")]
    NotImplemented(String),
}

impl SolidError {
    /// HTTP status code for this error.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::MethodNotAllowed(_) => 405,
            Self::Conflict(_) => 409,
            Self::PreconditionFailed(_) => 412,
            Self::NotModified => 304,
            Self::Internal(_) => 500,
            Self::NotImplemented(_) => 501,
        }
    }
}
