//! Account storage — faithful port of the TypeScript
//! `AccountStore` / `GenericAccountStore` / `BaseAccountStore` /
//! `BaseCookieStore` hierarchy.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use uuid::Uuid;

// ─── Settings key ────────────────────────────────────────────────────────────

/// Mirrors `ACCOUNT_SETTINGS_REMEMBER_LOGIN` from the TypeScript source.
pub const ACCOUNT_SETTINGS_REMEMBER_LOGIN: &str = "rememberLogin";

// ─── AccountStore trait ───────────────────────────────────────────────────────

/// Mirrors the `AccountStore<TSettings>` TypeScript interface.
///
/// Generic over a settings map so callers can extend the minimal set
/// (`rememberLogin`) with their own keys.
#[async_trait]
pub trait AccountStore: Send + Sync {
    /// Creates a new, empty account and returns its opaque ID.
    ///
    /// Implementations **must** enforce that an account with no login method
    /// is eventually cleaned up (see [`BaseAccountStore`] for the 30-minute
    /// timer pattern).
    async fn create(&self) -> anyhow::Result<String>;

    /// Returns the value of `setting` for the account identified by `id`.
    /// Returns `None` when the account does not exist or the setting is unset.
    async fn get_setting(
        &self,
        id: &str,
        setting: &str,
    ) -> anyhow::Result<Option<serde_json::Value>>;

    /// Overwrites a single setting on the given account.
    async fn update_setting(
        &self,
        id: &str,
        setting: &str,
        value: serde_json::Value,
    ) -> anyhow::Result<()>;

    /// Permanently removes the account and all its associated data.
    async fn delete(&self, id: &str) -> anyhow::Result<()>;
}

// ─── In-memory account row ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct AccountRow {
    id: String,
    /// How many login methods are linked (mirrors `linkedLoginsCount`).
    linked_logins_count: usize,
    settings: HashMap<String, serde_json::Value>,
    /// When the account was created — used for the "no login method" timeout.
    created_at: Instant,
}

// ─── BaseAccountStore ─────────────────────────────────────────────────────────

/// In-memory implementation of [`AccountStore`].
///
/// * Accounts with `linked_logins_count == 0` after `expiry` are garbage-
///   collected lazily on the next mutating call.
///   (The TypeScript `BaseLoginAccountStorage` uses a 30-minute `setTimeout`.)
/// * Default expiry is **30 minutes**, matching the TypeScript constant.
pub struct BaseAccountStore {
    inner: Arc<RwLock<HashMap<String, AccountRow>>>,
    /// How long an account without a login method survives before it is pruned.
    expiry: Duration,
}

impl Default for BaseAccountStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(30 * 60))
    }
}

impl BaseAccountStore {
    pub fn new(expiry: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            expiry,
        }
    }

    /// Increment the linked-logins counter for `id`.
    pub fn increment_login_count(&self, id: &str) -> anyhow::Result<()> {
        let mut guard = self.inner.write().unwrap();
        let row = guard
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("account not found: {id}"))?;
        row.linked_logins_count += 1;
        Ok(())
    }

    /// Decrement the linked-logins counter.
    /// Returns `Err` if this would drop below 1 (mirrors the TS guard:
    /// "An account needs at least 1 login method").
    pub fn decrement_login_count(&self, id: &str) -> anyhow::Result<()> {
        let mut guard = self.inner.write().unwrap();
        let row = guard
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("account not found: {id}"))?;
        if row.linked_logins_count <= 1 {
            anyhow::bail!("An account needs at least 1 login method.");
        }
        row.linked_logins_count -= 1;
        Ok(())
    }

    fn prune_expired(&self) {
        let now = Instant::now();
        let mut guard = self.inner.write().unwrap();
        guard.retain(|_, row| {
            !(row.linked_logins_count == 0 && now.duration_since(row.created_at) >= self.expiry)
        });
    }
}

#[async_trait]
impl AccountStore for BaseAccountStore {
    async fn create(&self) -> anyhow::Result<String> {
        self.prune_expired();
        let id = Uuid::new_v4().to_string();
        let row = AccountRow {
            id: id.clone(),
            linked_logins_count: 0,
            settings: HashMap::new(),
            created_at: Instant::now(),
        };
        self.inner.write().unwrap().insert(id.clone(), row);
        Ok(id)
    }

    async fn get_setting(
        &self,
        id: &str,
        setting: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        self.prune_expired();
        let guard = self.inner.read().unwrap();
        Ok(guard
            .get(id)
            .and_then(|row| row.settings.get(setting).cloned()))
    }

    async fn update_setting(
        &self,
        id: &str,
        setting: &str,
        value: serde_json::Value,
    ) -> anyhow::Result<()> {
        let mut guard = self.inner.write().unwrap();
        let row = guard
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("account not found: {id}"))?;
        row.settings.insert(setting.to_owned(), value);
        Ok(())
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.inner
            .write()
            .unwrap()
            .remove(id)
            .ok_or_else(|| anyhow::anyhow!("account not found: {id}"))?;
        Ok(())
    }
}

// ─── CookieStore trait ────────────────────────────────────────────────────────

/// Mirrors the TypeScript `CookieStore` interface.
///
/// Cookies are opaque tokens that map to an `account_id`.  Each cookie has a
/// TTL that is refreshed on access when the user chose "remember me".
#[async_trait]
pub trait CookieStore: Send + Sync {
    /// Generates and stores a new cookie for `account_id`.
    /// Does **not** replace any existing cookies for the same account.
    async fn generate(&self, account_id: &str) -> anyhow::Result<String>;

    /// Returns the `account_id` associated with `cookie`, or `None` if the
    /// cookie is unknown or has expired.
    async fn get(&self, cookie: &str) -> anyhow::Result<Option<String>>;

    /// Resets the TTL on `cookie` and returns the new expiry instant.
    /// Returns `None` if the cookie no longer exists.
    async fn refresh(&self, cookie: &str) -> anyhow::Result<Option<std::time::SystemTime>>;

    /// Invalidates and removes `cookie`.
    async fn delete(&self, cookie: &str) -> anyhow::Result<bool>;
}

// ─── BaseCookieStore ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct CookieEntry {
    account_id: String,
    expires_at: Instant,
}

/// In-memory [`CookieStore`] with a configurable TTL.
///
/// Default TTL is **14 days**, matching `BaseCookieStore` in TypeScript.
pub struct BaseCookieStore {
    inner: Arc<RwLock<HashMap<String, CookieEntry>>>,
    ttl: Duration,
}

impl Default for BaseCookieStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(14 * 24 * 60 * 60))
    }
}

impl BaseCookieStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    fn prune(&self) {
        let now = Instant::now();
        self.inner.write().unwrap().retain(|_, e| e.expires_at > now);
    }
}

#[async_trait]
impl CookieStore for BaseCookieStore {
    async fn generate(&self, account_id: &str) -> anyhow::Result<String> {
        self.prune();
        let cookie = Uuid::new_v4().to_string();
        let entry = CookieEntry {
            account_id: account_id.to_owned(),
            expires_at: Instant::now() + self.ttl,
        };
        self.inner.write().unwrap().insert(cookie.clone(), entry);
        Ok(cookie)
    }

    async fn get(&self, cookie: &str) -> anyhow::Result<Option<String>> {
        let now = Instant::now();
        let guard = self.inner.read().unwrap();
        Ok(guard
            .get(cookie)
            .filter(|e| e.expires_at > now)
            .map(|e| e.account_id.clone()))
    }

    async fn refresh(
        &self,
        cookie: &str,
    ) -> anyhow::Result<Option<std::time::SystemTime>> {
        let now = Instant::now();
        let mut guard = self.inner.write().unwrap();
        if let Some(entry) = guard.get_mut(cookie) {
            if entry.expires_at > now {
                entry.expires_at = now + self.ttl;
                let expiry = std::time::SystemTime::now() + self.ttl;
                return Ok(Some(expiry));
            }
        }
        Ok(None)
    }

    async fn delete(&self, cookie: &str) -> anyhow::Result<bool> {
        Ok(self.inner.write().unwrap().remove(cookie).is_some())
    }
}
