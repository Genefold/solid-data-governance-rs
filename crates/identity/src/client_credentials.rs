//! Client-credentials storage — faithful port of the TypeScript
//! `ClientCredentialsStore` / `BaseClientCredentialsStore` hierarchy.
//!
//! A **client-credential** token lets a service account authenticate as a
//! specific WebID using an OIDC client-credentials grant.  The TypeScript
//! `ClientCredentialsAdapter` validates the secret against this store at
//! token-request time.
//!
//! Fields mirror the TS `ClientCredentials` interface:
//! ```text
//! { id, label, webId, accountId, secret }
//! ```

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use url::Url;
use uuid::Uuid;

// ─── Domain types ─────────────────────────────────────────────────────────────

/// Mirrors the TypeScript `ClientCredentials` interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCredentials {
    /// Opaque token ID.
    pub id: String,
    /// Human-readable label (unique per account; used as the OIDC client_id).
    pub label: String,
    /// The WebID this token is allowed to authenticate as.
    pub web_id: Url,
    /// Account that created the token.
    pub account_id: String,
    /// The plain-text secret shown to the user once at creation time.
    /// In production this would be stored hashed; the in-memory impl keeps
    /// plain-text for simplicity.
    pub secret: String,
}

// ─── ClientCredentialsStore trait ────────────────────────────────────────────

/// Mirrors the TypeScript `ClientCredentialsStore` interface.
#[async_trait]
pub trait ClientCredentialsStore: Send + Sync {
    /// Returns the token identified by `id`, or `None`.
    async fn get(&self, id: &str) -> anyhow::Result<Option<ClientCredentials>>;

    /// Returns the token whose `label == label`, or `None`.
    ///
    /// Labels function as the OIDC `client_id`; they are globally unique
    /// (a UUID is appended to user-supplied names in the TS source).
    async fn find_by_label(&self, label: &str) -> anyhow::Result<Option<ClientCredentials>>;

    /// Returns all tokens that belong to `account_id`.
    async fn find_by_account(&self, account_id: &str) -> anyhow::Result<Vec<ClientCredentials>>;

    /// Creates a new token and returns the full record (including secret).
    ///
    /// * `label`      — caller-supplied (already sanitised + UUID-suffixed).
    /// * `web_id`     — the WebID to authenticate as.
    /// * `account_id` — the owning account.
    async fn create(
        &self,
        label: &str,
        web_id: &Url,
        account_id: &str,
    ) -> anyhow::Result<ClientCredentials>;

    /// Permanently removes the token identified by `id`.
    async fn delete(&self, id: &str) -> anyhow::Result<()>;
}

// ─── BaseClientCredentialsStore ───────────────────────────────────────────────

/// In-memory [`ClientCredentialsStore`].
///
/// Mirrors `BaseClientCredentialsStore` from the TypeScript source.
///
/// The TypeScript implementation stores credentials in an
/// `AccountLoginStorage` with two indexes (`accountId`, `label`).  Here
/// we replicate those indexes with two `HashMap`s:
/// * `by_id`    — primary store (token ID → record)
/// * `by_label` — secondary index (label → token ID)
pub struct BaseClientCredentialsStore {
    by_id: Arc<RwLock<HashMap<String, ClientCredentials>>>,
    /// label → token ID
    by_label: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for BaseClientCredentialsStore {
    fn default() -> Self {
        Self {
            by_id: Arc::new(RwLock::new(HashMap::new())),
            by_label: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl BaseClientCredentialsStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates a 64-byte hex secret.
    /// Mirrors the TypeScript: `randomBytes(64).toString('hex')`.
    fn generate_secret() -> String {
        use std::fmt::Write;
        // Use UUID bytes as entropy source — avoids adding `rand` as a dep.
        let mut s = String::with_capacity(128);
        for _ in 0..8 {
            for b in Uuid::new_v4().as_bytes() {
                let _ = write!(s, "{b:02x}");
            }
        }
        s
    }
}

#[async_trait]
impl ClientCredentialsStore for BaseClientCredentialsStore {
    async fn get(&self, id: &str) -> anyhow::Result<Option<ClientCredentials>> {
        Ok(self.by_id.read().unwrap().get(id).cloned())
    }

    async fn find_by_label(&self, label: &str) -> anyhow::Result<Option<ClientCredentials>> {
        let label_idx = self.by_label.read().unwrap();
        let id = match label_idx.get(label) {
            Some(id) => id.clone(),
            None => return Ok(None),
        };
        drop(label_idx);
        Ok(self.by_id.read().unwrap().get(&id).cloned())
    }

    async fn find_by_account(&self, account_id: &str) -> anyhow::Result<Vec<ClientCredentials>> {
        let guard = self.by_id.read().unwrap();
        Ok(guard
            .values()
            .filter(|c| c.account_id == account_id)
            .cloned()
            .collect())
    }

    async fn create(
        &self,
        label: &str,
        web_id: &Url,
        account_id: &str,
    ) -> anyhow::Result<ClientCredentials> {
        let id = Uuid::new_v4().to_string();
        let secret = Self::generate_secret();
        let creds = ClientCredentials {
            id: id.clone(),
            label: label.to_owned(),
            web_id: web_id.clone(),
            account_id: account_id.to_owned(),
            secret,
        };
        self.by_id.write().unwrap().insert(id.clone(), creds.clone());
        self.by_label.write().unwrap().insert(label.to_owned(), id);
        Ok(creds)
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let removed = self.by_id.write().unwrap().remove(id);
        if let Some(creds) = removed {
            self.by_label.write().unwrap().remove(&creds.label);
            Ok(())
        } else {
            anyhow::bail!("client credentials not found: {id}")
        }
    }
}
