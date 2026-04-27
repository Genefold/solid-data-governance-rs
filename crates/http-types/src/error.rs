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

    // ── Ported from: test/unit/util/errors/HttpError.test.ts ──────────────
    // and test/unit/util/errors/*.test.ts

    // --- status_code() ---

    #[test]
    fn status_codes_all_variants() {
        let cases: &[(&str, SolidError, u16)] = &[
            ("NotModified",          SolidError::NotModified,                     304),
            ("BadRequest",           SolidError::BadRequest("".into()),            400),
            ("Unauthorized",         SolidError::Unauthorized("".into()),          401),
            ("Forbidden",            SolidError::Forbidden("".into()),             403),
            ("NotFound",             SolidError::NotFound("".into()),              404),
            ("MethodNotAllowed",     SolidError::MethodNotAllowed("".into()),      405),
            ("NotAcceptable",        SolidError::NotAcceptable("".into()),         406),
            ("Conflict",             SolidError::Conflict("".into()),              409),
            ("Gone",                 SolidError::Gone("".into()),                  410),
            ("PreconditionFailed",   SolidError::PreconditionFailed("".into()),    412),
            ("UnsupportedMediaType", SolidError::UnsupportedMediaType("".into()), 415),
            ("UnprocessableEntity",  SolidError::UnprocessableEntity("".into()),  422),
            ("Internal",             SolidError::Internal("".into()),              500),
            ("NotImplemented",       SolidError::NotImplemented("".into()),        501),
            ("ServiceUnavailable",   SolidError::ServiceUnavailable("".into()),    503),
        ];
        for (name, err, expected_code) in cases {
            assert_eq!(
                err.status_code(), *expected_code,
                "wrong status code for {name}"
            );
        }
    }

    // --- is_client_error() / is_server_error() ---
    // Mirrors: it('should correctly identify client errors')
    //          it('should correctly identify server errors')

    #[test]
    fn bad_request_is_client_error_not_server_error() {
        let e = SolidError::BadRequest("oops".into());
        assert!(e.is_client_error(), "expected 400 to be a client error");
        assert!(!e.is_server_error(), "400 should not be a server error");
    }

    #[test]
    fn forbidden_is_client_error() {
        let e = SolidError::Forbidden("write blocked".into());
        assert!(e.is_client_error());
        assert!(!e.is_server_error());
    }

    #[test]
    fn not_found_is_client_error() {
        let e = SolidError::NotFound("resource gone".into());
        assert!(e.is_client_error());
        assert!(!e.is_server_error());
    }

    #[test]
    fn internal_is_server_error_not_client_error() {
        let e = SolidError::Internal("boom".into());
        assert!(e.is_server_error(), "expected 500 to be a server error");
        assert!(!e.is_client_error(), "500 should not be a client error");
    }

    #[test]
    fn not_implemented_is_server_error() {
        let e = SolidError::NotImplemented("NYI".into());
        assert!(e.is_server_error());
        assert!(!e.is_client_error());
    }

    #[test]
    fn not_modified_is_neither_client_nor_server_error() {
        let e = SolidError::NotModified;
        assert!(!e.is_client_error(), "304 is not a client error");
        assert!(!e.is_server_error(), "304 is not a server error");
    }

    // --- Display / message formatting ---
    // Mirrors: it('should contain the status and message in the display string')

    #[test]
    fn not_found_message_contains_status_and_payload() {
        let e = SolidError::NotFound("resource gone".into());
        let msg = e.to_string();
        assert!(msg.contains("404"),          "expected '404' in '{msg}'");
        assert!(msg.contains("resource gone"), "expected payload in '{msg}'");
    }

    #[test]
    fn forbidden_message_contains_status_and_payload() {
        let e = SolidError::Forbidden("write blocked".into());
        let msg = e.to_string();
        assert!(msg.contains("403"));
        assert!(msg.contains("write blocked"));
    }

    #[test]
    fn internal_message_contains_status_and_payload() {
        let e = SolidError::Internal("unexpected".into());
        let msg = e.to_string();
        assert!(msg.contains("500"));
        assert!(msg.contains("unexpected"));
    }

    #[test]
    fn not_modified_display_contains_304() {
        let msg = SolidError::NotModified.to_string();
        assert!(msg.contains("304"));
    }

    // --- Clone / Debug (derived) ---
    // Mirrors: it('should be clonable')

    #[test]
    fn solid_error_is_cloneable() {
        let original = SolidError::Conflict("version mismatch".into());
        let cloned = original.clone();
        assert_eq!(original.status_code(), cloned.status_code());
        assert_eq!(original.to_string(), cloned.to_string());
    }

    #[test]
    fn solid_error_is_debuggable() {
        let e = SolidError::NotFound("x".into());
        let s = format!("{e:?}");
        assert!(s.contains("NotFound"));
    }
}
