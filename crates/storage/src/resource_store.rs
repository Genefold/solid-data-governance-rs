//! `ResourceStore` and `ResourceSet` traits plus decorator structs.
//!
//! TypeScript sources mirrored:
//!   src/storage/ResourceStore.ts   → `ResourceStore`, `ResourceSet`, `ChangeMap`
//!   src/storage/BaseResourceStore.ts → `BaseResourceStore`
//!   src/storage/PassthroughStore.ts  → `PassthroughStore`
//!   src/storage/ReadOnlyStore.ts     → `ReadOnlyStore`

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use url::Url;

use crate::error::StorageError;

// ──────────────────────────────────────────────────────────────────────────────
// Core domain types (inline stubs — the real types live in `solid-http-types`;
// we re-export compatible shapes here so the storage crate compiles stand-alone
// and the trait bounds are self-contained).
// ──────────────────────────────────────────────────────────────────────────────

/// Opaque binary body of a representation, together with its content-type.
#[derive(Debug, Clone)]
pub struct RepresentationBody {
    pub content_type: String,
    pub data: Bytes,
}

/// Lightweight metadata attached to a changed resource in a `ChangeMap`.
///
/// Mirrors `RepresentationMetadata` carrying a `solid:activity` quad.
#[derive(Debug, Clone)]
pub struct ChangeMetadata {
    /// URI of the ActivityStreams activity, e.g.
    /// `https://www.w3.org/ns/activitystreams#Create`.
    pub activity: String,
}

impl ChangeMetadata {
    pub fn new(activity: impl Into<String>) -> Self {
        Self {
            activity: activity.into(),
        }
    }

    // Common activity constants (mirrors AS.terms.* in TS vocabularies)
    pub const AS_CREATE: &'static str =
        "https://www.w3.org/ns/activitystreams#Create";
    pub const AS_UPDATE: &'static str =
        "https://www.w3.org/ns/activitystreams#Update";
    pub const AS_DELETE: &'static str =
        "https://www.w3.org/ns/activitystreams#Delete";
    pub const AS_ADD: &'static str =
        "https://www.w3.org/ns/activitystreams#Add";
    pub const AS_REMOVE: &'static str =
        "https://www.w3.org/ns/activitystreams#Remove";
}

/// A map from resource URL string → change metadata.
///
/// Mirrors `ChangeMap = IdentifierMap<RepresentationMetadata>` in TS.
/// Returned by every mutating `ResourceStore` method so callers (e.g.
/// `MonitoringStore`) can emit fine-grained change events.
pub type ChangeMap = HashMap<String, ChangeMetadata>;

/// A resource representation as seen by the `ResourceStore`.
#[derive(Debug, Clone)]
pub struct Representation {
    pub identifier: Url,
    pub body: RepresentationBody,
    pub metadata: HashMap<String, String>,
}

/// Desired content-type preferences (mirrors `RepresentationPreferences`).
#[derive(Debug, Clone, Default)]
pub struct RepresentationPreferences {
    /// Weighted content-type preferences, e.g. `{ "text/turtle": 1.0 }`.
    pub r#type: HashMap<String, f64>,
}

// ──────────────────────────────────────────────────────────────────────────────
// ResourceSet
// ──────────────────────────────────────────────────────────────────────────────

/// Read-only existence check on a collection of resources.
///
/// Mirrors `ResourceSet.ts`:
/// ```ts
/// export interface ResourceSet {
///   hasResource(identifier: ResourceIdentifier): Promise<boolean>;
/// }
/// ```
#[async_trait]
pub trait ResourceSet: Send + Sync {
    /// Returns `true` if the resource identified by `url` exists.
    async fn has_resource(&self, url: &Url) -> Result<bool, StorageError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// ResourceStore
// ──────────────────────────────────────────────────────────────────────────────

/// Full CRUD store for Solid resources.
///
/// Mirrors `ResourceStore.ts`:
/// ```ts
/// export interface ResourceStore extends ResourceSet {
///   getRepresentation(id, prefs, conds?): Promise<Representation>;
///   addResource(container, repr, conds?): Promise<ChangeMap>;
///   setRepresentation(id, repr, conds?): Promise<ChangeMap>;
///   deleteResource(id, conds?): Promise<ChangeMap>;
///   modifyResource(id, patch, conds?): Promise<ChangeMap>;
/// }
/// ```
///
/// Every mutating method returns a `ChangeMap` so monitoring / notification
/// layers can react to changes without polling.
#[async_trait]
pub trait ResourceStore: ResourceSet {
    /// Retrieve a representation of the resource at `url`.
    async fn get_representation(
        &self,
        url: &Url,
        preferences: &RepresentationPreferences,
    ) -> Result<Representation, StorageError>;

    /// Create a new child resource inside `container_url`.
    ///
    /// Returns a `ChangeMap` whose entries cover at least the new child
    /// (Create) and the parent container (Update).
    async fn add_resource(
        &self,
        container_url: &Url,
        representation: Representation,
    ) -> Result<ChangeMap, StorageError>;

    /// Create or replace the resource at `url`.
    async fn set_representation(
        &self,
        url: &Url,
        representation: Representation,
    ) -> Result<ChangeMap, StorageError>;

    /// Delete the resource at `url`.
    async fn delete_resource(&self, url: &Url) -> Result<ChangeMap, StorageError>;

    /// Apply a partial patch to the resource at `url`.
    ///
    /// The `patch` bytes contain the patch body (e.g. SPARQL-Update or N3
    /// Patch); implementations may delegate to a `PatchHandler`.
    async fn modify_resource(
        &self,
        url: &Url,
        patch: Bytes,
    ) -> Result<ChangeMap, StorageError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// BaseResourceStore
// ──────────────────────────────────────────────────────────────────────────────

/// Default implementation of `ResourceStore` that rejects every call with
/// `StorageError::NotImplemented`.
///
/// Mirrors `BaseResourceStore.ts` — useful as a starting point for custom
/// stores that only need to override a subset of methods.
pub struct BaseResourceStore;

#[async_trait]
impl ResourceSet for BaseResourceStore {
    async fn has_resource(&self, _url: &Url) -> Result<bool, StorageError> {
        Err(StorageError::NotImplemented(
            "BaseResourceStore::has_resource".into(),
        ))
    }
}

#[async_trait]
impl ResourceStore for BaseResourceStore {
    async fn get_representation(
        &self,
        _url: &Url,
        _preferences: &RepresentationPreferences,
    ) -> Result<Representation, StorageError> {
        Err(StorageError::NotImplemented(
            "BaseResourceStore::get_representation".into(),
        ))
    }

    async fn add_resource(
        &self,
        _container_url: &Url,
        _representation: Representation,
    ) -> Result<ChangeMap, StorageError> {
        Err(StorageError::NotImplemented(
            "BaseResourceStore::add_resource".into(),
        ))
    }

    async fn set_representation(
        &self,
        _url: &Url,
        _representation: Representation,
    ) -> Result<ChangeMap, StorageError> {
        Err(StorageError::NotImplemented(
            "BaseResourceStore::set_representation".into(),
        ))
    }

    async fn delete_resource(&self, _url: &Url) -> Result<ChangeMap, StorageError> {
        Err(StorageError::NotImplemented(
            "BaseResourceStore::delete_resource".into(),
        ))
    }

    async fn modify_resource(
        &self,
        _url: &Url,
        _patch: Bytes,
    ) -> Result<ChangeMap, StorageError> {
        Err(StorageError::NotImplemented(
            "BaseResourceStore::modify_resource".into(),
        ))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// PassthroughStore
// ──────────────────────────────────────────────────────────────────────────────

/// Decorator that forwards every call to an inner `ResourceStore`.
///
/// Mirrors `PassthroughStore.ts` — subclass (in Rust terms: wrap) to intercept
/// only the methods you care about.
pub struct PassthroughStore<S: ResourceStore> {
    pub source: Arc<S>,
}

impl<S: ResourceStore + 'static> PassthroughStore<S> {
    pub fn new(source: Arc<S>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl<S: ResourceStore + 'static> ResourceSet for PassthroughStore<S> {
    async fn has_resource(&self, url: &Url) -> Result<bool, StorageError> {
        self.source.has_resource(url).await
    }
}

#[async_trait]
impl<S: ResourceStore + 'static> ResourceStore for PassthroughStore<S> {
    async fn get_representation(
        &self,
        url: &Url,
        preferences: &RepresentationPreferences,
    ) -> Result<Representation, StorageError> {
        self.source.get_representation(url, preferences).await
    }

    async fn add_resource(
        &self,
        container_url: &Url,
        representation: Representation,
    ) -> Result<ChangeMap, StorageError> {
        self.source
            .add_resource(container_url, representation)
            .await
    }

    async fn set_representation(
        &self,
        url: &Url,
        representation: Representation,
    ) -> Result<ChangeMap, StorageError> {
        self.source.set_representation(url, representation).await
    }

    async fn delete_resource(&self, url: &Url) -> Result<ChangeMap, StorageError> {
        self.source.delete_resource(url).await
    }

    async fn modify_resource(
        &self,
        url: &Url,
        patch: Bytes,
    ) -> Result<ChangeMap, StorageError> {
        self.source.modify_resource(url, patch).await
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ReadOnlyStore
// ──────────────────────────────────────────────────────────────────────────────

/// Wraps a `ResourceStore` and rejects any mutating operation with `403 Forbidden`.
///
/// Mirrors `ReadOnlyStore.ts`:
/// ```ts
/// export class ReadOnlyStore<T extends ResourceStore>
///   extends PassthroughStore<T> {
///   async addResource()    { throw new ForbiddenHttpError(); }
///   async setRepresentation() { ... }
///   async deleteResource()    { ... }
///   async modifyResource()    { ... }
/// }
/// ```
pub struct ReadOnlyStore<S: ResourceStore> {
    pub source: Arc<S>,
}

impl<S: ResourceStore + 'static> ReadOnlyStore<S> {
    pub fn new(source: Arc<S>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl<S: ResourceStore + 'static> ResourceSet for ReadOnlyStore<S> {
    async fn has_resource(&self, url: &Url) -> Result<bool, StorageError> {
        self.source.has_resource(url).await
    }
}

#[async_trait]
impl<S: ResourceStore + 'static> ResourceStore for ReadOnlyStore<S> {
    async fn get_representation(
        &self,
        url: &Url,
        preferences: &RepresentationPreferences,
    ) -> Result<Representation, StorageError> {
        self.source.get_representation(url, preferences).await
    }

    async fn add_resource(
        &self,
        _container_url: &Url,
        _representation: Representation,
    ) -> Result<ChangeMap, StorageError> {
        Err(StorageError::Forbidden(
            "Store is read-only".into(),
        ))
    }

    async fn set_representation(
        &self,
        _url: &Url,
        _representation: Representation,
    ) -> Result<ChangeMap, StorageError> {
        Err(StorageError::Forbidden(
            "Store is read-only".into(),
        ))
    }

    async fn delete_resource(&self, _url: &Url) -> Result<ChangeMap, StorageError> {
        Err(StorageError::Forbidden(
            "Store is read-only".into(),
        ))
    }

    async fn modify_resource(
        &self,
        _url: &Url,
        _patch: Bytes,
    ) -> Result<ChangeMap, StorageError> {
        Err(StorageError::Forbidden(
            "Store is read-only".into(),
        ))
    }
}
