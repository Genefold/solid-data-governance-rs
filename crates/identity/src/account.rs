//! Account management: creation, login, and password verification.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A registered account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub verified: bool,
}

impl Account {
    pub fn new(email: impl Into<String>, password_hash: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            email: email.into(),
            password_hash: password_hash.into(),
            verified: false,
        }
    }
}

/// Trait for account storage operations.
///
/// Mirrors the TypeScript `AccountStore` interface.
#[async_trait]
pub trait AccountStore: Send + Sync {
    async fn create(&self) -> anyhow::Result<String>;
    async fn get(&self, account_id: &str) -> anyhow::Result<Option<Account>>;
    async fn delete(&self, account_id: &str) -> anyhow::Result<()>;
}

/// Trait for password operations.
///
/// Mirrors the TypeScript `PasswordStore` interface.
#[async_trait]
pub trait PasswordStore: Send + Sync {
    async fn create(
        &self,
        email: &str,
        account_id: &str,
        password: &str,
    ) -> anyhow::Result<String>;
    async fn confirm_verification(&self, id: &str) -> anyhow::Result<()>;
    async fn verify(&self, email: &str, password: &str) -> anyhow::Result<Option<String>>;
}
