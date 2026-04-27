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

    // ── Ported from: test/unit/http/representation/ResourceIdentifier.test.ts
    // (and cross-cutting identifier semantics exercised in other TS suites)

    // --- construction ---

    #[test]
    fn new_from_str_literal() {
        let id = ResourceIdentifier::new("http://test.com/resource");
        assert_eq!(id.path, "http://test.com/resource");
    }

    #[test]
    fn from_str_ref() {
        let id = ResourceIdentifier::from("http://test.com/resource");
        assert_eq!(id.path, "http://test.com/resource");
    }

    #[test]
    fn from_owned_string() {
        let id = ResourceIdentifier::from("http://test.com/container/".to_string());
        assert_eq!(id.path, "http://test.com/container/");
    }

    #[test]
    fn from_url() {
        let url = Url::parse("http://test.com/path").unwrap();
        let id = ResourceIdentifier::try_from(url).unwrap();
        assert_eq!(id.path, "http://test.com/path");
    }

    #[test]
    fn into_string() {
        let id = ResourceIdentifier::new("http://test.com/x");
        let s: String = id.into();
        assert_eq!(s, "http://test.com/x");
    }

    // --- is_container() ---
    // Mirrors: it('should return true if path ends with /')
    //          it('should return false if path does not end with /')

    #[test]
    fn is_container_true_when_trailing_slash() {
        assert!(ResourceIdentifier::new("http://test.com/container/").is_container());
        assert!(ResourceIdentifier::new("http://test.com/").is_container());
    }

    #[test]
    fn is_container_false_without_trailing_slash() {
        assert!(!ResourceIdentifier::new("http://test.com/resource").is_container());
        assert!(!ResourceIdentifier::new("http://test.com/resource.ttl").is_container());
    }

    // --- as_container() / as_document() ---

    #[test]
    fn as_container_appends_slash() {
        let id = ResourceIdentifier::new("http://test.com/foo");
        assert_eq!(id.as_container().path, "http://test.com/foo/");
    }

    #[test]
    fn as_container_idempotent_when_already_container() {
        let id = ResourceIdentifier::new("http://test.com/foo/");
        assert_eq!(id.as_container().path, "http://test.com/foo/");
    }

    #[test]
    fn as_document_strips_trailing_slash() {
        let id = ResourceIdentifier::new("http://test.com/foo/");
        assert_eq!(id.as_document().path, "http://test.com/foo");
    }

    #[test]
    fn as_document_idempotent_when_already_document() {
        let id = ResourceIdentifier::new("http://test.com/foo");
        assert_eq!(id.as_document().path, "http://test.com/foo");
    }

    #[test]
    fn as_container_then_as_document_roundtrips() {
        let doc = ResourceIdentifier::new("http://test.com/foo");
        assert_eq!(doc.as_container().as_document().path, doc.path);
    }

    // --- parent() ---
    // Mirrors: it('should return parent container')
    //          it('should handle containers (trailing slash)')
    //          it('should return None for root')

    #[test]
    fn parent_of_document_is_its_container() {
        let id = ResourceIdentifier::new("http://test.com/a/b/c");
        let parent = id.parent().unwrap();
        assert_eq!(parent.path, "http://test.com/a/b/");
        assert!(parent.is_container());
    }

    #[test]
    fn parent_of_container_is_parent_container() {
        let id = ResourceIdentifier::new("http://test.com/a/b/");
        let parent = id.parent().unwrap();
        assert_eq!(parent.path, "http://test.com/a/");
        assert!(parent.is_container());
    }

    #[test]
    fn parent_of_root_container_is_none() {
        let root = ResourceIdentifier::new("http://test.com/");
        assert!(root.parent().is_none(), "root should have no parent");
    }

    #[test]
    fn parent_of_direct_child_is_root() {
        let id = ResourceIdentifier::new("http://test.com/child");
        let parent = id.parent().unwrap();
        assert_eq!(parent.path, "http://test.com/");
    }

    // --- join() ---

    #[test]
    fn join_appends_segment_to_container() {
        let base = ResourceIdentifier::new("http://test.com/alice/");
        assert_eq!(base.join("profile").path, "http://test.com/alice/profile");
    }

    #[test]
    fn join_no_double_slash_when_segment_starts_with_slash() {
        let base = ResourceIdentifier::new("http://test.com/alice/");
        assert_eq!(base.join("/profile").path, "http://test.com/alice/profile");
    }

    #[test]
    fn join_inserts_slash_on_document_base() {
        let base = ResourceIdentifier::new("http://test.com/alice");
        assert_eq!(base.join("profile").path, "http://test.com/alice/profile");
    }

    // --- path_only() ---
    // Mirrors the URL path-component stripping used in TS identifier helpers

    #[test]
    fn path_only_strips_scheme_and_authority() {
        let id = ResourceIdentifier::new("http://test.com/some/path");
        assert_eq!(id.path_only(), "/some/path");
    }

    #[test]
    fn path_only_preserves_bare_path() {
        let id = ResourceIdentifier::new("/bare/path");
        assert_eq!(id.path_only(), "/bare/path");
    }

    #[test]
    fn path_only_root() {
        let id = ResourceIdentifier::new("http://test.com/");
        assert_eq!(id.path_only(), "/");
    }

    // --- equality, hash, ordering ---
    // Mirrors the IdentifierMap key-equality semantics from TS

    #[test]
    fn equality_same_path() {
        let a = ResourceIdentifier::new("http://test.com/x");
        let b = ResourceIdentifier::new("http://test.com/x");
        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_path() {
        let a = ResourceIdentifier::new("http://test.com/x");
        let b = ResourceIdentifier::new("http://test.com/y");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_deduplication_in_hashset() {
        use std::collections::HashSet;
        let a = ResourceIdentifier::new("http://test.com/x");
        let b = ResourceIdentifier::new("http://test.com/x");
        let c = ResourceIdentifier::new("http://test.com/y");
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b); // duplicate — should not increase size
        set.insert(c);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn ordering_is_lexicographic() {
        let a = ResourceIdentifier::new("http://test.com/a");
        let b = ResourceIdentifier::new("http://test.com/b");
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn sort_vec_of_identifiers() {
        let mut v = vec![
            ResourceIdentifier::new("http://test.com/c"),
            ResourceIdentifier::new("http://test.com/a"),
            ResourceIdentifier::new("http://test.com/b"),
        ];
        v.sort();
        assert_eq!(v[0].path, "http://test.com/a");
        assert_eq!(v[1].path, "http://test.com/b");
        assert_eq!(v[2].path, "http://test.com/c");
    }

    // --- Display / AsRef ---

    #[test]
    fn display_returns_full_path() {
        let id = ResourceIdentifier::new("http://test.com/resource");
        assert_eq!(id.to_string(), "http://test.com/resource");
    }

    #[test]
    fn as_ref_str() {
        let id = ResourceIdentifier::new("http://test.com/resource");
        let s: &str = id.as_ref();
        assert_eq!(s, "http://test.com/resource");
    }

    // --- Clone / Debug (derived) ---

    #[test]
    fn clone_produces_equal_identifier() {
        let original = ResourceIdentifier::new("http://test.com/alice/");
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn debug_format_contains_path() {
        let id = ResourceIdentifier::new("http://test.com/debug");
        let s = format!("{id:?}");
        assert!(s.contains("http://test.com/debug"));
    }
}
