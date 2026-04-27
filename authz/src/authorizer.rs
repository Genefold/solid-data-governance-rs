//! The `Authorizer` trait: verifies permissions after they have been read.

use async_trait::async_trait;
use crate::{
    credentials::Credentials,
    permissions::{AccessMap, PermissionMap},
};
use http_types::SolidError;

/// Verifies that the given credentials have sufficient permission.
///
/// Mirrors the TypeScript `Authorizer` interface.
#[async_trait]
pub trait Authorizer: Send + Sync {
    async fn authorize(
        &self,
        credentials: &Credentials,
        requested_modes: &AccessMap,
        available_permissions: &PermissionMap,
    ) -> Result<(), SolidError>;
}
