//! File-system-backed `ResourceStore`.
//!
//! Each resource is stored as a file under a configurable root directory.
//! Metadata is persisted as a sidecar `.meta` JSON file.

use async_trait::async_trait;
use http_types::{Representation, ResourceIdentifier, metadata::RepresentationMetadata};
use std::path::{Path, PathBuf};
use tokio::fs;
use crate::{error::StorageError, resource_store::ResourceStore};

pub struct FileResourceStore {
    root: PathBuf,
}

impl FileResourceStore {
    /// Create a new store rooted at `root`. The directory is created if absent.
    pub async fn new(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        fs::create_dir_all(&root).await?;
        Ok(Self { root })
    }

    fn resource_path(&self, identifier: &ResourceIdentifier) -> PathBuf {
        // Strip leading slash and join with root.
        let rel = identifier.path.trim_start_matches('/');
        self.root.join(rel)
    }

    fn meta_path(&self, identifier: &ResourceIdentifier) -> PathBuf {
        let mut p = self.resource_path(identifier);
        let name = p
            .file_name()
            .map(|n| format!("{}.meta", n.to_string_lossy()))
            .unwrap_or_else(|| ".meta".into());
        p.set_file_name(name);
        p
    }
}

#[async_trait]
impl ResourceStore for FileResourceStore {
    async fn get_representation(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<Representation, StorageError> {
        let path = self.resource_path(identifier);
        let data = fs::read(&path)
            .await
            .map_err(|_| StorageError::NotFound(identifier.path.clone()))?;

        let meta_path = self.meta_path(identifier);
        let metadata = if meta_path.exists() {
            let raw = fs::read_to_string(&meta_path).await?;
            serde_json::from_str::<RepresentationMetadata>(&raw)
                .map_err(|e| StorageError::Serialization(e.to_string()))?
        } else {
            RepresentationMetadata::new()
        };

        Ok(Representation::new(data, metadata))
    }

    async fn set_representation(
        &self,
        identifier: &ResourceIdentifier,
        representation: Representation,
    ) -> Result<(), StorageError> {
        let path = self.resource_path(identifier);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&path, &representation.data).await?;

        let meta_json = serde_json::to_string_pretty(&representation.metadata)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        fs::write(self.meta_path(identifier), meta_json).await?;
        Ok(())
    }

    async fn delete_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<(), StorageError> {
        let path = self.resource_path(identifier);
        fs::remove_file(&path)
            .await
            .map_err(|_| StorageError::NotFound(identifier.path.clone()))?;
        let meta = self.meta_path(identifier);
        if meta.exists() {
            let _ = fs::remove_file(meta).await;
        }
        Ok(())
    }

    async fn has_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<bool, StorageError> {
        Ok(self.resource_path(identifier).exists())
    }
}
