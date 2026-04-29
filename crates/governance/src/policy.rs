//! Dataset access policy types.
//!
//! Mirrors the Phase 1 policy schema described in the engineering plan,
//! but introduced in Phase 0 so the catalog can persist and serve
//! `policy/access.jsonld` from the day a dataset is registered.

use serde::{Deserialize, Serialize};

/// Access tier model. Phase 0 only enforces the difference between
/// `Discovery` (metadata-only) and `Training` (full access); the other
/// tiers exist so policy documents are forward-compatible.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Discovery,
    Evaluation,
    Training,
    Inference,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Discovery => "discovery",
            Tier::Evaluation => "evaluation",
            Tier::Training => "training",
            Tier::Inference => "inference",
        }
    }
}

/// One per-principal access grant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessRule {
    pub principal: String,
    pub tier: Tier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_cap: Option<u64>,
}

/// Full access policy for a dataset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub dataset_id: String,
    pub default_tier: Tier,
    #[serde(default)]
    pub rules: Vec<AccessRule>,
}

impl AccessPolicy {
    pub fn discovery_default(dataset_id: impl Into<String>) -> Self {
        Self {
            dataset_id: dataset_id.into(),
            default_tier: Tier::Discovery,
            rules: Vec::new(),
        }
    }
}
