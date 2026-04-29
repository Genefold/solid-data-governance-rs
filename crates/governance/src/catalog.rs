//! On-disk catalog of governed datasets.
//!
//! Each registered dataset has a directory under `<catalog_root>/<sanitized_id>/`
//! containing:
//!
//! ```text
//! metadata/dataset.jsonld   — dataset description (registration form input)
//! policy/access.jsonld      — current AccessPolicy
//! audit/events.ndjson       — append-only AuditLog
//! stac-item.json            — placeholder STAC item (Phase 2 fills this in)
//! ```
//!
//! The catalog object is the single point of contact for the HTTP layer
//! and writes through to all four artifacts atomically per-call.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use crate::{
    audit::{AuditEvent, AuditLog},
    policy::AccessPolicy,
};

/// One catalog entry as exposed via the HTTP API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub dataset_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub policy: Option<AccessPolicy>,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("dataset already exists: {0}")]
    AlreadyExists(String),
    #[error("dataset not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

fn sanitize(id: &str) -> String {
    id.replace(['/', '\\'], "__")
}

/// Filesystem-backed catalog.
pub struct Catalog {
    root: PathBuf,
}

impl Catalog {
    /// Open or create a catalog rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, CatalogError> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        info!(path = %root.display(), "Catalog opened");
        Ok(Self { root })
    }

    fn dataset_dir(&self, dataset_id: &str) -> PathBuf {
        self.root.join(sanitize(dataset_id))
    }

    /// Register a new dataset. Returns an error if it already exists.
    pub fn register(
        &self,
        dataset_id: &str,
        title: &str,
        description: &str,
    ) -> Result<CatalogEntry, CatalogError> {
        let dir = self.dataset_dir(dataset_id);
        if dir.exists() {
            return Err(CatalogError::AlreadyExists(dataset_id.to_owned()));
        }
        std::fs::create_dir_all(dir.join("metadata"))?;
        std::fs::create_dir_all(dir.join("policy"))?;
        std::fs::create_dir_all(dir.join("audit"))?;

        let entry = CatalogEntry {
            dataset_id: dataset_id.to_owned(),
            title: title.to_owned(),
            description: description.to_owned(),
            created_at: chrono::Utc::now(),
            policy: Some(AccessPolicy::discovery_default(dataset_id)),
        };

        std::fs::write(
            dir.join("metadata").join("dataset.jsonld"),
            serde_json::to_vec_pretty(&entry)?,
        )?;
        std::fs::write(
            dir.join("policy").join("access.jsonld"),
            serde_json::to_vec_pretty(entry.policy.as_ref().unwrap())?,
        )?;
        // Empty STAC placeholder for Phase 2.
        std::fs::write(
            dir.join("stac-item.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "type": "Feature",
                "id": dataset_id,
                "stac_version": "1.0.0",
                "properties": { "datetime": entry.created_at },
                "geometry": null,
                "links": [],
                "assets": {},
            }))?,
        )?;

        let audit = AuditLog::open(dir.join("audit").join("events.ndjson"))?;
        audit.append(&AuditEvent::new("dataset.create", dataset_id))?;
        info!(dataset_id, title, "dataset registered");
        Ok(entry)
    }

    /// Load a catalog entry by dataset id.
    pub fn get(&self, dataset_id: &str) -> Result<CatalogEntry, CatalogError> {
        let dir = self.dataset_dir(dataset_id);
        if !dir.exists() {
            return Err(CatalogError::NotFound(dataset_id.to_owned()));
        }
        let bytes = std::fs::read(dir.join("metadata").join("dataset.jsonld"))?;
        let mut entry: CatalogEntry = serde_json::from_slice(&bytes)?;
        if let Ok(pol_bytes) = std::fs::read(dir.join("policy").join("access.jsonld")) {
            entry.policy = serde_json::from_slice(&pol_bytes).ok();
        }
        Ok(entry)
    }

    /// Replace the policy for a dataset.
    pub fn put_policy(&self, dataset_id: &str, policy: &AccessPolicy) -> Result<(), CatalogError> {
        let dir = self.dataset_dir(dataset_id);
        if !dir.exists() {
            return Err(CatalogError::NotFound(dataset_id.to_owned()));
        }
        std::fs::write(
            dir.join("policy").join("access.jsonld"),
            serde_json::to_vec_pretty(policy)?,
        )?;
        let audit = AuditLog::open(dir.join("audit").join("events.ndjson"))?;
        audit.append(&AuditEvent::new("policy.update", dataset_id))?;
        Ok(())
    }

    /// Open the audit log for a dataset.
    pub fn audit_log(&self, dataset_id: &str) -> Result<AuditLog, CatalogError> {
        let dir = self.dataset_dir(dataset_id);
        if !dir.exists() {
            return Err(CatalogError::NotFound(dataset_id.to_owned()));
        }
        Ok(AuditLog::open(dir.join("audit").join("events.ndjson"))?)
    }

    /// List all dataset ids.
    pub fn list(&self) -> Result<Vec<String>, CatalogError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                out.push(name.replace("__", "/"));
            }
        }
        out.sort();
        Ok(out)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("catalog-test-{}", uuid::Uuid::new_v4()));
        p
    }

    #[test]
    fn register_get_list() {
        let dir = tempdir();
        let cat = Catalog::open(&dir).unwrap();
        let entry = cat
            .register("org-a/bert-v2", "BERT v2 Embeddings", "demo")
            .unwrap();
        assert_eq!(entry.dataset_id, "org-a/bert-v2");
        assert_eq!(entry.policy.as_ref().unwrap().default_tier.as_str(), "discovery");

        let got = cat.get("org-a/bert-v2").unwrap();
        assert_eq!(got.title, "BERT v2 Embeddings");

        let list = cat.list().unwrap();
        assert!(list.contains(&"org-a/bert-v2".to_owned()));

        let log = cat.audit_log("org-a/bert-v2").unwrap();
        let events = log.read_all(None).unwrap();
        assert!(events.iter().any(|e| e.action == "dataset.create"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
