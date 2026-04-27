//! A single URL-path → filesystem-path mapping entry.

use std::path::PathBuf;

/// Maps a URL path prefix to a filesystem path.
///
/// Mirrors the TypeScript `StaticAssetEntry`.
#[derive(Debug, Clone)]
pub struct StaticAssetEntry {
    /// The URL path that triggers this mapping (e.g., `"/assets/styles"`).
    pub url_path: String,
    /// The filesystem path to serve from.
    pub fs_path: PathBuf,
}

impl StaticAssetEntry {
    pub fn new(url_path: impl Into<String>, fs_path: impl Into<PathBuf>) -> Self {
        Self {
            url_path: url_path.into(),
            fs_path: fs_path.into(),
        }
    }
}
