//! WebID linking between accounts and RDF identity documents.

use async_trait::async_trait;

/// Trait for WebID ↔ account associations.
///
/// Mirrors the TypeScript `WebIdStore` interface.
#[async_trait]
pub trait WebIdStore: Send + Sync {
    async fn link(&self, web_id: &str, account_id: &str) -> anyhow::Result<String>;
    async fn get_account(&self, web_id: &str) -> anyhow::Result<Option<String>>;
    async fn is_owned(&self, web_id: &str, account_id: &str) -> anyhow::Result<bool>;
}
