//! Authorization layer: WAC and ACP permission evaluation.
//!
//! Mirrors the TypeScript `src/authorization` module.

pub mod authorizer;
pub mod credentials;
pub mod permissions;

pub use authorizer::Authorizer;
pub use credentials::Credentials;
pub use permissions::{AccessMap, PermissionMap, PermissionReader};
