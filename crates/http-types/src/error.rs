//! Typed HTTP error hierarchy.
//!
//! [`SolidError`] mirrors the TypeScript `HttpError` class tree.  When the
//! **`axum`** feature is enabled the type also implements
//! `axum::response::IntoResponse` so it can be returned directly from route
//! handlers.

use thiserror::Error;

/// All error conditions that can arise inside the Solid server.
///
/// Each variant corresponds to a specific HTTP status code and carries a
/// human-readable message that is forwarded to the client.
#[derive(Debug, Error, Clone)]
pub enum SolidError {
    // ── 3xx ───────────────────────────────────────────────────────────────
    #[error("304 Not Modified")]
    NotModified,

    // ── 4xx ───────────────────────────────────────────────────────────────
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

    #[error("406 Not Acceptable: {0}")]
    NotAcceptable(String),

    #[error("409 Conflict: {0}")]
    Conflict(String),

    #[error("410 Gone: {0}")]
    Gone(String),

    #[error("412 Precondition Failed: {0}")]
    PreconditionFailed(String),

    #[error("415 Unsupported Media Type: {0}")]
    UnsupportedMediaType(String),

    #[error("422 Unprocessable Entity: {0}")]
    UnprocessableEntity(String),

    // ── 5xx ───────────────────────────────────────────────────────────────
    #[error("500 Internal Server Error: {0}")]
    Internal(String),

    #[error("501 Not Implemented: {0}")]
    NotImplemented(String),

    #[error("503 Service Unavailable: {0}")]
    ServiceUnavailable(String),
}

impl SolidError {
    /// The numeric HTTP status code for this error.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotModified => 304,
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::MethodNotAllowed(_) => 405,
            Self::NotAcceptable(_) => 406,
            Self::Conflict(_) => 409,
            Self::Gone(_) => 410,
            Self::PreconditionFailed(_) => 412,
            Self::UnsupportedMediaType(_) => 415,
            Self::UnprocessableEntity(_) => 422,
            Self::Internal(_) => 500,
            Self::NotImplemented(_) => 501,
            Self::ServiceUnavailable(_) => 503,
        }
    }

    /// Returns `true` for 4xx client errors.
    pub fn is_client_error(&self) -> bool {
        let code = self.status_code();
        (400..500).contains(&code)
    }

    /// Returns `true` for 5xx server errors.
    pub fn is_server_error(&self) -> bool {
        self.status_code() >= 500
    }
}

// ── axum IntoResponse (feature-gated) ────────────────────────────────────

#[cfg(feature = "axum")]
mod axum_impl {
    use super::SolidError;
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
        Json,
    };
    use serde_json::json;

    impl IntoResponse for SolidError {
        fn into_response(self) -> Response {
            let status = StatusCode::from_u16(self.status_code())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let body = Json(json!({
                "error": status.canonical_reason().unwrap_or("Error"),
                "message": self.to_string(),
            }));
            (status, body).into_response()
        }
    }

    impl From<SolidError> for Response {
        fn from(e: SolidError) -> Self {
            e.into_response()
        }
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes() {
        assert_eq!(SolidError::NotFound("x".into()).status_code(), 404);
        assert_eq!(SolidError::Forbidden("x".into()).status_code(), 403);
        assert_eq!(SolidError::Internal("x".into()).status_code(), 500);
        assert_eq!(SolidError::NotModified.status_code(), 304);
    }

    #[test]
    fn client_vs_server_error() {
        assert!(SolidError::BadRequest("x".into()).is_client_error());
        assert!(!SolidError::BadRequest("x".into()).is_server_error());
        assert!(SolidError::Internal("x".into()).is_server_error());
    }
}
