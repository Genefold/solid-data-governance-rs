//! Core HTTP domain types for the Solid Community Server.
//!
//! # Feature flags
//! - **`axum`** *(off by default)* — enables [`SolidError`] as an axum `IntoResponse`,
//!   and exposes the [`handler`] module containing [`HttpHandler`] and
//!   [`OperationHttpHandler`] trait definitions.

pub mod error;
pub mod identifier;
pub mod metadata;
pub mod operation;
pub mod representation;

#[cfg(feature = "axum")]
pub mod handler;

// ── flat re-exports ────────────────────────────────────────────────────────
pub use error::SolidError;
pub use identifier::ResourceIdentifier;
pub use metadata::{ConditionalHeaders, LinkHeader, RepresentationMetadata};
pub use operation::{AccessMode, ContentPreferences, HttpMethod, MediaRange, Operation};
pub use representation::Representation;

#[cfg(feature = "axum")]
pub use handler::{HttpHandler, OperationHttpHandler};
