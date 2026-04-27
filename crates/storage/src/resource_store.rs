//! The `ResourceStore` trait: the primary abstraction for LDP resource operations.

use async_trait::async_trait;
use http_types::{Representation, ResourceIdentifier};
use crate::error::StorageError;

/// Core trait for reading, writing, and deleting Linked Data Platform resources.
///
/// Mirrors the TypeScript `ResourceStore` interface.
#[async_trait]
pub trait ResourceStore: Send + Sync {
    /// Retrieve a representation of the resource at `identifier`.
    async fn get_representation(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<Representation, StorageError>;

    /// Create or replace the resource at `identifier` with `representation`.
    async fn set_representation(
        &self,
        identifier: &ResourceIdentifier,
        representation: Representation,
    ) -> Result<(), StorageError>;

    /// Delete the resource at `identifier`.
    async fn delete_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<(), StorageError>;

    /// Return `true` if the resource exists.
    async fn has_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<bool, StorageError>;
}
