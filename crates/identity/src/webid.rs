//! WebID storage — faithful port of the TypeScript
//! `WebIdStore` / `BaseWebIdStore` / `WebIdLink` hierarchy.
//!
//! A **WebID link** records that a particular WebID URL belongs to a
//! particular account.  One account may have multiple WebID links.
//! The link object is identified by its own opaque `id` (the "webIdLink"
//! field in the TypeScript source).

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use url::Url;
use uuid::Uuid;

// ─── Domain types ─────────────────────────────────────────────────────────────

/// Returned when a new WebID link is created.
/// `id` is the opaque link identifier (the "webIdLink" value in TS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebIdLink {
    /// Opaque identifier for the link record itself.
    pub id: String,
    pub web_id: Url,
    pub account_id: String,
}

// ─── WebIdStore trait ─────────────────────────────────────────────────────────

/// Mirrors the TypeScript `WebIdStore` interface.
#[async_trait]
pub trait WebIdStore: Send + Sync {
    /// Returns `true` when `web_id` is already registered to `account_id`.
    async fn is_linked(&self, web_id: &Url, account_id: &str) -> anyhow::Result<bool>;

    /// Creates a new link between `web_id` and `account_id`.
    /// Returns the [`WebIdLink`] record.
    async fn create(&self, web_id: &Url, account_id: &str) -> anyhow::Result<WebIdLink>;

    /// Returns the [`WebIdLink`] identified by `link_id`, or `None`.
    async fn get(&self, link_id: &str) -> anyhow::Result<Option<WebIdLink>>;

    /// Returns all links that belong to `account_id`.
    async fn find_by_account(&self, account_id: &str) -> anyhow::Result<Vec<WebIdLink>>;

    /// Deletes the link identified by `link_id`.
    ///
    /// Used for rollback when pod creation fails after the WebID link was
    /// already created (mirrors the TS `webIdStore.delete(webIdLink)` call
    /// inside `BasePodCreator`).
    async fn delete(&self, link_id: &str) -> anyhow::Result<()>;
}

// ─── BaseWebIdStore ───────────────────────────────────────────────────────────

/// In-memory [`WebIdStore`].
pub struct BaseWebIdStore {
    links: Arc<RwLock<HashMap<String, WebIdLink>>>,
}

impl Default for BaseWebIdStore {
    fn default() -> Self {
        Self {
            links: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl BaseWebIdStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WebIdStore for BaseWebIdStore {
    async fn is_linked(&self, web_id: &Url, account_id: &str) -> anyhow::Result<bool> {
        let guard = self.links.read().unwrap();
        Ok(guard
            .values()
            .any(|l| &l.web_id == web_id && l.account_id == account_id))
    }

    async fn create(&self, web_id: &Url, account_id: &str) -> anyhow::Result<WebIdLink> {
        let id = Uuid::new_v4().to_string();
        let link = WebIdLink {
            id: id.clone(),
            web_id: web_id.clone(),
            account_id: account_id.to_owned(),
        };
        self.links.write().unwrap().insert(id, link.clone());
        Ok(link)
    }

    async fn get(&self, link_id: &str) -> anyhow::Result<Option<WebIdLink>> {
        Ok(self.links.read().unwrap().get(link_id).cloned())
    }

    async fn find_by_account(&self, account_id: &str) -> anyhow::Result<Vec<WebIdLink>> {
        let guard = self.links.read().unwrap();
        Ok(guard
            .values()
            .filter(|l| l.account_id == account_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, link_id: &str) -> anyhow::Result<()> {
        self.links
            .write()
            .unwrap()
            .remove(link_id)
            .ok_or_else(|| anyhow::anyhow!("webid link not found: {link_id}"))?;
        Ok(())
    }
}
