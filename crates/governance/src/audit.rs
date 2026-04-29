//! Append-only NDJSON audit log.
//!
//! One file per dataset under `<catalog_root>/<sanitized_id>/audit/events.ndjson`.
//! The plan calls for monthly rollover (`events-YYYY-MM.ndjson`); Phase 0
//! keeps a single file because the consumer interface is the same and we
//! don't want to invent rollover logic before the API surface is wired.

use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

/// One audit event as persisted in NDJSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_id: String,
    pub action: String,
    pub dataset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

impl AuditEvent {
    pub fn new(action: impl Into<String>, dataset_id: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            event_id: uuid::Uuid::new_v4().to_string(),
            action: action.into(),
            dataset_id: dataset_id.into(),
            principal: None,
            tier: None,
            bytes: None,
            resource: None,
            status: None,
        }
    }
    pub fn with_principal(mut self, p: impl Into<String>) -> Self {
        self.principal = Some(p.into());
        self
    }
    pub fn with_tier(mut self, t: impl Into<String>) -> Self {
        self.tier = Some(t.into());
        self
    }
    pub fn with_bytes(mut self, b: u64) -> Self {
        self.bytes = Some(b);
        self
    }
    pub fn with_resource(mut self, r: impl Into<String>) -> Self {
        self.resource = Some(r.into());
        self
    }
    pub fn with_status(mut self, s: u16) -> Self {
        self.status = Some(s);
        self
    }
}

/// Per-dataset audit ledger.
pub struct AuditLog {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl AuditLog {
    /// Open or create a log at `path`. Parent directories are created
    /// if they do not exist.
    pub fn open(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            path,
            write_lock: Mutex::new(()),
        })
    }

    /// Append one event. Serialization failures are returned to the
    /// caller; partial writes are not retried (the file is append-only
    /// and a partial line will be ignored by the reader).
    pub fn append(&self, event: &AuditEvent) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        line.push(b'\n');
        let _g = self.write_lock.lock().unwrap();
        let mut f = OpenOptions::new().append(true).open(&self.path)?;
        f.write_all(&line)?;
        Ok(())
    }

    /// Read all events from the log (bounded by `limit`, when supplied).
    pub fn read_all(&self, limit: Option<usize>) -> std::io::Result<Vec<AuditEvent>> {
        let f = match OpenOptions::new().read(true).open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let reader = BufReader::new(f);
        let mut out = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AuditEvent>(&line) {
                Ok(ev) => out.push(ev),
                Err(_) => continue,
            }
            if let Some(n) = limit {
                if out.len() >= n {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn append_and_read() {
        let mut p = temp_dir();
        p.push(format!("audit-test-{}.ndjson", uuid::Uuid::new_v4()));
        let log = AuditLog::open(&p).unwrap();
        log.append(
            &AuditEvent::new("dataset.create", "org/x").with_principal("https://alice/#me"),
        )
        .unwrap();
        log.append(&AuditEvent::new("token.issue", "org/x").with_tier("evaluation"))
            .unwrap();
        let events = log.read_all(None).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, "dataset.create");
        assert_eq!(events[1].tier.as_deref(), Some("evaluation"));
        std::fs::remove_file(&p).ok();
    }
}
