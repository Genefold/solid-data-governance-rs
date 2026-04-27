//! In-memory implementations of `ResourceStore` and `KeyValueStore`.

use async_trait::async_trait;
use http_types::{Representation, ResourceIdentifier, metadata::RepresentationMetadata};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use bytes::Bytes;
use crate::{error::StorageError, key_value::KeyValueStore, resource_store::ResourceStore};

// ---------------------------------------------------------------------------
// MemoryResourceStore
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct MemoryResourceStore {
    inner: Arc<RwLock<HashMap<String, Representation>>>,
}

impl MemoryResourceStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ResourceStore for MemoryResourceStore {
    async fn get_representation(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<Representation, StorageError> {
        let guard = self.inner.read().await;
        guard
            .get(&identifier.path)
            .map(|r| Representation::new(r.data.clone(), r.metadata.clone()))
            .ok_or_else(|| StorageError::NotFound(identifier.path.clone()))
    }

    async fn set_representation(
        &self,
        identifier: &ResourceIdentifier,
        representation: Representation,
    ) -> Result<(), StorageError> {
        let mut guard = self.inner.write().await;
        guard.insert(identifier.path.clone(), representation);
        Ok(())
    }

    async fn delete_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<(), StorageError> {
        let mut guard = self.inner.write().await;
        guard
            .remove(&identifier.path)
            .ok_or_else(|| StorageError::NotFound(identifier.path.clone()))?;
        Ok(())
    }

    async fn has_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<bool, StorageError> {
        let guard = self.inner.read().await;
        Ok(guard.contains_key(&identifier.path))
    }
}

// ---------------------------------------------------------------------------
// MemoryKeyValueStore
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct MemoryKeyValueStore {
    inner: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl MemoryKeyValueStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KeyValueStore for MemoryKeyValueStore {
    async fn get<V: DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> Result<Option<V>, StorageError> {
        let guard = self.inner.read().await;
        match guard.get(key) {
            None => Ok(None),
            Some(v) => {
                let parsed: V = serde_json::from_value(v.clone())
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(parsed))
            }
        }
    }

    async fn set<V: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &V,
    ) -> Result<(), StorageError> {
        let v = serde_json::to_value(value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let mut guard = self.inner.write().await;
        guard.insert(key.to_owned(), v);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let mut guard = self.inner.write().await;
        guard.remove(key);
        Ok(())
    }

    async fn has(&self, key: &str) -> Result<bool, StorageError> {
        let guard = self.inner.read().await;
        Ok(guard.contains_key(key))
    }
}
