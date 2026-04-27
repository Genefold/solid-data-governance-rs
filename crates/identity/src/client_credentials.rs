//! Client credentials token management.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A client credentials record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCredentials {
    pub id: String,
    pub secret: String,
    pub web_id: String,
    pub account_id: String,
    pub label: String,
}

/// Trait for client credentials storage.
///
/// Mirrors the TypeScript `BaseClientCredentialsStore` interface.
#[async_trait]
pub trait ClientCredentialsStore: Send + Sync {
    async fn create(
        &self,
        label: &str,
        web_id: &str,
        account_id: &str,
    ) -> anyhow::Result<ClientCredentials>;

    async fn get(&self, id: &str) -> anyhow::Result<Option<ClientCredentials>>;

    async fn delete(&self, id: &str) -> anyhow::Result<()>;
}
