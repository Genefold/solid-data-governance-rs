//! Resource identifier types.

use url::Url;

/// Uniquely identifies a resource on the server by its full URL path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceIdentifier {
    pub path: String,
}

impl ResourceIdentifier {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Returns true if this identifier refers to a container (path ends with `/`).
    pub fn is_container(&self) -> bool {
        self.path.ends_with('/')
    }

    /// Returns the parent container identifier, if any.
    pub fn parent(&self) -> Option<Self> {
        let trimmed = self.path.trim_end_matches('/');
        let pos = trimmed.rfind('/')?;
        Some(Self::new(format!("{}/", &trimmed[..=pos])))
    }
}

impl TryFrom<Url> for ResourceIdentifier {
    type Error = String;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        Ok(Self::new(url.as_str().to_owned()))
    }
}
