//! Static asset request handler.

use crate::entry::StaticAssetEntry;
use http_types::SolidError;
use mime_guess::MimeGuess;
use std::path::PathBuf;
use tokio::fs;

/// Serves static assets by mapping URL paths to filesystem paths.
///
/// Picks the longest matching prefix. Mirrors `StaticAssetHandler` in TypeScript.
pub struct StaticAssetHandler {
    entries: Vec<StaticAssetEntry>,
    /// Cache max-age in seconds. `None` disables cache headers.
    expires: Option<u64>,
}

impl StaticAssetHandler {
    pub fn new(entries: Vec<StaticAssetEntry>, expires: Option<u64>) -> Self {
        Self { entries, expires }
    }

    /// Resolve a request URL to a filesystem path and MIME type.
    ///
    /// Returns `Err(SolidError::NotFound)` if no entry matches.
    pub fn resolve(&self, url_path: &str) -> Result<(PathBuf, String), SolidError> {
        // Strip query string.
        let clean = url_path.split('?').next().unwrap_or(url_path);

        // Longest-prefix match.
        let best = self
            .entries
            .iter()
            .filter(|e| clean.starts_with(&e.url_path))
            .max_by_key(|e| e.url_path.len())
            .ok_or_else(|| SolidError::NotFound(format!("No static asset at {url_path}")))?;

        let suffix = &clean[best.url_path.len()..];
        // Prevent path traversal.
        if suffix.contains("..") {
            return Err(SolidError::NotFound(format!(
                "No static asset at {url_path}"
            )));
        }

        let fs_path = best.fs_path.join(suffix.trim_start_matches('/'));
        let mime = MimeGuess::from_path(&fs_path)
            .first_or_octet_stream()
            .to_string();

        Ok((fs_path, mime))
    }

    /// Read an asset from disk, returning its bytes and content-type.
    pub async fn serve(
        &self,
        url_path: &str,
    ) -> Result<(Vec<u8>, String), SolidError> {
        let (fs_path, mime) = self.resolve(url_path)?;
        let data = fs::read(&fs_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SolidError::NotFound(format!("{}", fs_path.display()))
            } else {
                SolidError::Internal(e.to_string())
            }
        })?;
        Ok((data, mime))
    }

    /// Optional cache max-age in seconds.
    pub fn cache_max_age(&self) -> Option<u64> {
        self.expires
    }
}
