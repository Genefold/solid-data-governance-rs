//! In-memory LDP resource store.
//!
//! A simple `Arc<RwLock<HashMap>>` that is shared across all request handlers.
//! Suitable for integration-test runs; swap in `solid-storage` backends for
//! production persistence.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// One stored resource entry.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Raw body bytes.
    pub body: Vec<u8>,
    /// `Content-Type` string as supplied by the PUT/POST caller.
    pub content_type: String,
    /// True when the path ends with `/` (container semantics).
    pub is_container: bool,
}

/// The shared, thread-safe in-memory store.
#[derive(Debug, Default)]
pub struct LdpStore {
    inner: RwLock<HashMap<String, Entry>>,
}

impl LdpStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Normalise a URL path key (always starts with `/`).
    fn key(path: &str) -> String {
        if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        }
    }

    /// Retrieve a resource.
    pub fn get(&self, path: &str) -> Option<Entry> {
        let k = Self::key(path);
        self.inner.read().unwrap().get(&k).cloned()
    }

    /// Check existence without cloning the body.
    pub fn exists(&self, path: &str) -> bool {
        let k = Self::key(path);
        self.inner.read().unwrap().contains_key(&k)
    }

    /// Store a resource.  Returns `true` if this was a create (did not exist).
    /// Auto-creates every ancestor container.
    pub fn put(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: String,
    ) -> bool {
        let k = Self::key(path);
        let is_container = k.ends_with('/');

        // Auto-create ancestor containers.
        self.ensure_ancestors(&k);

        let mut map = self.inner.write().unwrap();
        let created = !map.contains_key(&k);
        map.insert(
            k,
            Entry { body, content_type, is_container },
        );
        created
    }

    /// Delete a resource.  Returns `true` if the resource existed.
    pub fn delete(&self, path: &str) -> bool {
        let k = Self::key(path);
        self.inner.write().unwrap().remove(&k).is_some()
    }

    pub fn is_container(&self, path: &str) -> bool {
        Self::key(path).ends_with('/')
    }

    /// Walk up the path hierarchy and ensure every intermediate container
    /// exists (mirrors CSS auto-container-creation behaviour).
    fn ensure_ancestors(&self, key: &str) {
        // Strip trailing slash for segment splitting.
        let s = key.trim_end_matches('/');
        let mut parts: Vec<&str> = s.splitn(100, '/').collect();
        // Drop the leaf segment.
        if parts.len() > 1 {
            parts.pop();
        }

        let mut map = self.inner.write().unwrap();
        let mut current = String::new();
        for part in &parts {
            if part.is_empty() {
                current.push('/');
            } else {
                if !current.ends_with('/') {
                    current.push('/');
                }
                current.push_str(part);
                current.push('/');
            }
            // Only the root `/` and proper container paths.
            let k = if current == "//" {
                "/".to_owned()
            } else {
                current.clone()
            };
            map.entry(k).or_insert_with(|| Entry {
                body: Vec::new(),
                content_type: "text/turtle".to_owned(),
                is_container: true,
            });
        }
    }
}
