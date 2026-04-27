//! Core HTTP domain types for the Solid Community Server.
//!
//! This crate defines the fundamental types that flow through every layer of
//! the request pipeline: identifiers, operations, representations, metadata,
//! and the server error hierarchy.

pub mod error;
pub mod identifier;
pub mod metadata;
pub mod operation;
pub mod representation;

pub use error::SolidError;
pub use identifier::ResourceIdentifier;
pub use operation::{AccessMode, HttpMethod, Operation};
pub use representation::Representation;
