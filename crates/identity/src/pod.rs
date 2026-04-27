//! Pod creation and ownership.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A Solid pod record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pod {
    pub id: String,
    pub base_url: String,
    pub account_id: String,
}

/// Trait for creating pods.
///
/// Mirrors the TypeScript `PodCreator` interface.
#[async_trait]
pub trait PodCreator: Send + Sync {
    async fn create(
        &self,
        account_id: &str,
        name: Option<&str>,
    ) -> anyhow::Result<Pod>;
}
