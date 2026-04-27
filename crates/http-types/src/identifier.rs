//! Resource identifier types.
//!
//! A [`ResourceIdentifier`] is the canonical URL string that uniquely addresses
//! a resource on the server.  It is intentionally a thin newtype — all URL
//! parsing concerns live at the boundary (see [`TryFrom<url::Url>`]).

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use url::Url;

/// Uniquely identifies a resource on the server by its absolute URL.
///
/// The inner `path` is always the **full URL string** (e.g.
/// `"http://localhost:3000/alice/profile/card#me"`).
/// Use [`ResourceIdentifier::path_only`] when you need just the URL path
/// component without scheme/host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceIdentifier {
    /// The full, normalised URL string.
    pub path: String,
}

impl ResourceIdentifier {
    /// Construct from any string.  No validation is performed here;
    /// prefer [`TryFrom<Url>`] for validated construction at call sites
    /// that already hold a parsed URL.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Returns `true` if this identifier refers to an LDP container
    /// (i.e. the URL ends with `/`).
    pub fn is_container(&self) -> bool {
        self.path.ends_with('/')
    }

    /// Returns a container version of this identifier by appending `/`
    /// if it is not already present.
    pub fn as_container(&self) -> Self {
        if self.is_container() {
            self.clone()
        } else {
            Self::new(format!("{}/", self.path))
        }
    }

    /// Returns a document version of this identifier by stripping a
    /// trailing `/` if present.
    pub fn as_document(&self) -> Self {
        Self::new(self.path.trim_end_matches('/').to_owned())
    }

    /// Returns the URL path component only (strips scheme + authority).
    ///
    /// Returns the full string as-is when the value is already a bare path.
    pub fn path_only(&self) -> &str {
        // Fast path: already a bare path (starts with /).
        if self.path.starts_with('/') {
            return &self.path;
        }
        // Try to parse as URL and return just the path.
        if let Ok(url) = Url::parse(&self.path) {
            let start = url.path();
            // Find where the path component starts within the full string.
            if let Some(pos) = self.path.find(start) {
                return &self.path[pos..];
            }
        }
        &self.path
    }

    /// Returns the parent container identifier, or `None` for the root.
    ///
    /// ```
    /// # use http_types::ResourceIdentifier;
    /// let id = ResourceIdentifier::new("http://localhost/alice/profile/");
    /// assert_eq!(id.parent().unwrap().path, "http://localhost/alice/");
    /// ```
    pub fn parent(&self) -> Option<Self> {
        let trimmed = self.path.trim_end_matches('/');
        let pos = trimmed.rfind('/')?;
        Some(Self::new(format!("{}/", &trimmed[..=pos])))
    }

    /// Append a relative segment, returning a new identifier.
    ///
    /// A `/` is inserted between `self` and `segment` only when `self` does
    /// not already end with one.
    pub fn join(&self, segment: &str) -> Self {
        let base = self.path.trim_end_matches('/');
        let seg = segment.trim_start_matches('/');
        Self::new(format!("{base}/{seg}"))
    }
}

// ── trait impls ────────────────────────────────────────────────────────────

impl fmt::Display for ResourceIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

impl AsRef<str> for ResourceIdentifier {
    fn as_ref(&self) -> &str {
        &self.path
    }
}

impl PartialOrd for ResourceIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResourceIdentifier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl FromStr for ResourceIdentifier {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl From<&str> for ResourceIdentifier {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ResourceIdentifier {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl TryFrom<Url> for ResourceIdentifier {
    type Error = std::convert::Infallible;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        Ok(Self::new(url.as_str().to_owned()))
    }
}

impl From<ResourceIdentifier> for String {
    fn from(id: ResourceIdentifier) -> Self {
        id.path
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_detection() {
        assert!(ResourceIdentifier::new("http://localhost/").is_container());
        assert!(!ResourceIdentifier::new("http://localhost/file").is_container());
    }

    #[test]
    fn parent() {
        let id = ResourceIdentifier::new("http://localhost/a/b/c");
        assert_eq!(id.parent().unwrap().path, "http://localhost/a/b/");
    }

    #[test]
    fn join() {
        let root = ResourceIdentifier::new("http://localhost/alice/");
        assert_eq!(root.join("profile").path, "http://localhost/alice/profile");
    }

    #[test]
    fn ordering() {
        let a = ResourceIdentifier::new("http://localhost/a");
        let b = ResourceIdentifier::new("http://localhost/b");
        assert!(a < b);
    }

    #[test]
    fn as_document_and_container() {
        let doc = ResourceIdentifier::new("http://localhost/foo");
        assert_eq!(doc.as_container().path, "http://localhost/foo/");
        assert_eq!(doc.as_container().as_document().path, "http://localhost/foo");
    }
}
