//! Permission map types and the `PermissionReader` trait.

use async_trait::async_trait;
use http_types::{AccessMode, ResourceIdentifier};
use std::collections::{HashMap, HashSet};
use crate::credentials::Credentials;

/// The set of access modes requested for each resource.
pub type AccessMap = HashMap<ResourceIdentifier, HashSet<AccessMode>>;

/// The set of access modes that are actually permitted for each resource.
pub type PermissionMap = HashMap<ResourceIdentifier, HashMap<AccessMode, bool>>;

/// Reads the permissions for given credentials and requested access modes.
///
/// Mirrors the TypeScript `PermissionReader` interface.
#[async_trait]
pub trait PermissionReader: Send + Sync {
    async fn read(
        &self,
        credentials: &Credentials,
        requested_modes: &AccessMap,
    ) -> anyhow::Result<PermissionMap>;
}
