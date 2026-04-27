//! Pod storage — faithful port of the TypeScript
//! `PodStore` / `BasePodStore` / `PodSettings` hierarchy.
//!
//! Key concepts carried over from the TS source:
//! * A **pod** is identified by an opaque `id` string and has a `base_url`
//!   (the storage root) plus the `account_id` of its creator.
//! * Each pod can have multiple **owners** — `(web_id, visible)` pairs.
//! * `BasePodStore` mirrors the two-table design: a `pod` table and an
//!   `owner` table, with rollback on failure.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use url::Url;
use uuid::Uuid;

// ─── Domain types ─────────────────────────────────────────────────────────────

/// Mirrors the `PodSettings` TypeScript interface used when creating a pod.
#[derive(Debug, Clone)]
pub struct PodSettings {
    /// Storage root URL for the new pod.
    pub base: Url,
    /// WebID to associate with the pod (may be an external WebID).
    pub web_id: Option<Url>,
    /// OIDC issuer to embed in the pod — typically the server's base URL.
    pub oidc_issuer: Option<Url>,
}

/// A single pod record.
#[derive(Debug, Clone)]
pub struct PodInfo {
    pub id: String,
    pub base_url: Url,
    pub account_id: String,
}

/// One entry in the pod's owner list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodOwner {
    pub web_id: Url,
    /// Whether this ownership relation is publicly visible in the pod metadata.
    pub visible: bool,
}

// ─── PodStore trait ───────────────────────────────────────────────────────────

/// Mirrors the `PodStore` TypeScript interface.
#[async_trait]
pub trait PodStore: Send + Sync {
    /// Creates a new pod for `account_id` with the given `settings`.
    ///
    /// `overwrite` mirrors the second boolean parameter of
    /// `PodManager.createPod`. Returns the opaque pod ID.
    async fn create(
        &self,
        account_id: &str,
        settings: PodSettings,
        overwrite: bool,
    ) -> anyhow::Result<String>;

    /// Returns the pod identified by `id`, or `None`.
    async fn get(&self, id: &str) -> anyhow::Result<Option<PodInfo>>;

    /// Returns all pods that belong to `account_id`.
    async fn find_pods(&self, account_id: &str) -> anyhow::Result<Vec<PodInfo>>;

    /// Resolves a pod by its `base_url`. Returns `None` when no pod matches.
    async fn find_by_base_url(&self, base_url: &Url) -> anyhow::Result<Option<PodInfo>>;

    /// Returns all owners of pod `id`, or `None` when the pod has no owners
    /// yet (matches the TypeScript `undefined` return).
    async fn get_owners(&self, id: &str) -> anyhow::Result<Option<Vec<PodOwner>>>;

    /// Creates or updates the visibility of `web_id` as an owner of pod `id`.
    async fn update_owner(&self, id: &str, web_id: &Url, visible: bool) -> anyhow::Result<()>;

    /// Removes `web_id` from the owner list of pod `id`.
    /// Does nothing when the web_id is not an owner.
    async fn remove_owner(&self, id: &str, web_id: &Url) -> anyhow::Result<()>;
}

// ─── Internal storage rows ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct PodRow {
    id: String,
    base_url: Url,
    account_id: String,
}

#[derive(Clone, Debug)]
struct OwnerRow {
    id: String,
    pod_id: String,
    web_id: Url,
    visible: bool,
}

// ─── BasePodStore ─────────────────────────────────────────────────────────────

/// In-memory [`PodStore`].
///
/// Mirrors the two-collection design from `BasePodStore.ts`:
/// * `pods`   → keyed by pod ID
/// * `owners` → keyed by owner-entry ID; filtered by `pod_id`
pub struct BasePodStore {
    pods: Arc<RwLock<HashMap<String, PodRow>>>,
    owners: Arc<RwLock<HashMap<String, OwnerRow>>>,
}

impl Default for BasePodStore {
    fn default() -> Self {
        Self {
            pods: Arc::new(RwLock::new(HashMap::new())),
            owners: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl BasePodStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PodStore for BasePodStore {
    async fn create(
        &self,
        account_id: &str,
        settings: PodSettings,
        _overwrite: bool,
    ) -> anyhow::Result<String> {
        let pod_id = Uuid::new_v4().to_string();
        self.pods.write().unwrap().insert(
            pod_id.clone(),
            PodRow {
                id: pod_id.clone(),
                base_url: settings.base.clone(),
                account_id: account_id.to_owned(),
            },
        );

        // Record the initial (invisible) owner when a WebID is provided.
        // Mirrors: `storage.create(OWNER_TYPE, { podId, webId, visible: false })`
        if let Some(web_id) = settings.web_id {
            let owner_id = Uuid::new_v4().to_string();
            self.owners.write().unwrap().insert(
                owner_id.clone(),
                OwnerRow {
                    id: owner_id,
                    pod_id: pod_id.clone(),
                    web_id,
                    visible: false,
                },
            );
        }

        Ok(pod_id)
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<PodInfo>> {
        Ok(self.pods.read().unwrap().get(id).map(|r| PodInfo {
            id: r.id.clone(),
            base_url: r.base_url.clone(),
            account_id: r.account_id.clone(),
        }))
    }

    async fn find_pods(&self, account_id: &str) -> anyhow::Result<Vec<PodInfo>> {
        let guard = self.pods.read().unwrap();
        Ok(guard
            .values()
            .filter(|r| r.account_id == account_id)
            .map(|r| PodInfo {
                id: r.id.clone(),
                base_url: r.base_url.clone(),
                account_id: r.account_id.clone(),
            })
            .collect())
    }

    async fn find_by_base_url(&self, base_url: &Url) -> anyhow::Result<Option<PodInfo>> {
        let guard = self.pods.read().unwrap();
        Ok(guard
            .values()
            .find(|r| &r.base_url == base_url)
            .map(|r| PodInfo {
                id: r.id.clone(),
                base_url: r.base_url.clone(),
                account_id: r.account_id.clone(),
            }))
    }

    async fn get_owners(&self, id: &str) -> anyhow::Result<Option<Vec<PodOwner>>> {
        let guard = self.owners.read().unwrap();
        let owners: Vec<PodOwner> = guard
            .values()
            .filter(|r| r.pod_id == id)
            .map(|r| PodOwner {
                web_id: r.web_id.clone(),
                visible: r.visible,
            })
            .collect();
        if owners.is_empty() {
            Ok(None)
        } else {
            Ok(Some(owners))
        }
    }

    async fn update_owner(&self, id: &str, web_id: &Url, visible: bool) -> anyhow::Result<()> {
        let mut guard = self.owners.write().unwrap();
        let existing = guard
            .values_mut()
            .find(|r| r.pod_id == id && &r.web_id == web_id);
        if let Some(row) = existing {
            row.visible = visible;
        } else {
            let owner_id = Uuid::new_v4().to_string();
            guard.insert(
                owner_id.clone(),
                OwnerRow {
                    id: owner_id,
                    pod_id: id.to_owned(),
                    web_id: web_id.clone(),
                    visible,
                },
            );
        }
        Ok(())
    }

    async fn remove_owner(&self, id: &str, web_id: &Url) -> anyhow::Result<()> {
        let mut guard = self.owners.write().unwrap();
        let entry_id = guard
            .values()
            .find(|r| r.pod_id == id && &r.web_id == web_id)
            .map(|r| r.id.clone());
        if let Some(eid) = entry_id {
            guard.remove(&eid);
        }
        Ok(())
    }
}
