//! HTTP operation types.
//!
//! An [`Operation`] is the fully-parsed, validated representation of an
//! incoming HTTP request after initial middleware processing.  It carries the
//! method, target identifier, body (for write operations), content negotiation
//! preferences, and a credential placeholder that the identity layer fills in.

use crate::{Representation, ResourceIdentifier};
use serde::{Deserialize, Serialize};

// ── HttpMethod ─────────────────────────────────────────────────────────────

/// Parsed HTTP method supported by the Solid protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        })
    }
}

impl TryFrom<&str> for HttpMethod {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "HEAD" => Ok(Self::Head),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "OPTIONS" => Ok(Self::Options),
            other => Err(format!("Unknown HTTP method: {other}")),
        }
    }
}

// ── AccessMode ────────────────────────────────────────────────────────────

/// Access modes used by the WAC / ACP authorization layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    Read,
    Write,
    Append,
    Control,
}

// ── ContentPreferences ───────────────────────────────────────────────────

/// A single media-range entry parsed from an `Accept` header.
///
/// Carries the media type string and a quality value (`q`) in the range
/// `0.0..=1.0`.
///
/// ```
/// # use http_types::MediaRange;
/// let mr = MediaRange::new("text/turtle", 1.0);
/// assert_eq!(mr.media_type, "text/turtle");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRange {
    /// The media type string, e.g. `"text/turtle"` or `"*/*"`.
    pub media_type: String,
    /// Quality value (`0.0` = not acceptable, `1.0` = most preferred).
    pub q: f32,
}

impl MediaRange {
    pub fn new(media_type: impl Into<String>, q: f32) -> Self {
        Self {
            media_type: media_type.into(),
            q: q.clamp(0.0, 1.0),
        }
    }

    /// Returns a wildcard `*/*` range with quality 1.0 (accept anything).
    pub fn wildcard() -> Self {
        Self::new("*/*", 1.0)
    }

    /// Returns `true` if this range matches any media type.
    pub fn is_wildcard(&self) -> bool {
        self.media_type == "*/*"
    }

    /// Returns `true` if this range is a subtype wildcard (e.g. `text/*`).
    pub fn is_subtype_wildcard(&self) -> bool {
        self.media_type.ends_with("/*") && self.media_type != "*/*"
    }
}

impl PartialOrd for MediaRange {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Higher q-value = higher preference.
        self.q.partial_cmp(&other.q)
    }
}

/// Content negotiation preferences extracted from an HTTP `Accept` header.
///
/// The list is ordered by descending quality value (highest preference first).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentPreferences {
    /// Accepted media ranges, sorted by descending q-value.
    pub type_preferences: Vec<MediaRange>,
}

impl ContentPreferences {
    /// Build `ContentPreferences` from a raw `Accept` header value.
    ///
    /// Unknown or malformed entries are silently skipped.
    pub fn from_accept_header(value: &str) -> Self {
        let mut ranges: Vec<MediaRange> = value
            .split(',')
            .filter_map(|part| {
                let mut iter = part.splitn(2, ';');
                let media_type = iter.next()?.trim().to_ascii_lowercase();
                let q = iter
                    .next()
                    .and_then(|params| {
                        params
                            .split(';')
                            .find_map(|p| p.trim().strip_prefix("q="))
                            .and_then(|v| v.parse::<f32>().ok())
                    })
                    .unwrap_or(1.0);
                if media_type.is_empty() {
                    return None;
                }
                Some(MediaRange::new(media_type, q))
            })
            .collect();

        // Sort by descending q-value, preserving original order for ties
        // (stable sort).
        ranges.sort_by(|a, b| b.q.partial_cmp(&a.q).unwrap_or(std::cmp::Ordering::Equal));
        Self { type_preferences: ranges }
    }

    /// Returns `true` if the preferences list is empty or contains only
    /// a wildcard entry — i.e. the client will accept anything.
    pub fn accepts_any(&self) -> bool {
        self.type_preferences.is_empty()
            || self.type_preferences.iter().all(|r| r.is_wildcard())
    }

    /// Returns the most preferred media type string, or `None` if the list
    /// is empty.
    pub fn most_preferred(&self) -> Option<&str> {
        self.type_preferences.first().map(|r| r.media_type.as_str())
    }
}

// ── AgentCredentials placeholder ──────────────────────────────────────────

/// Opaque credential placeholder attached to every operation.
///
/// The identity crate fills this in after DPoP/Bearer token validation.
/// Keeping it here avoids a circular dependency between `http-types` and
/// `identity`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCredentials {
    /// The authenticated agent's WebID, if any.
    pub web_id: Option<String>,
    /// Client identifier from a client-credentials token, if any.
    pub client_id: Option<String>,
}

impl AgentCredentials {
    pub fn anonymous() -> Self {
        Self::default()
    }

    pub fn is_authenticated(&self) -> bool {
        self.web_id.is_some()
    }
}

// ── Operation ─────────────────────────────────────────────────────────────

/// A fully-parsed, validated HTTP request ready for pipeline dispatch.
///
/// Constructed by the `OperationParser` in `server-core` after the raw
/// `axum::http::Request` has been decoded.  From this point on all handler
/// code works with `Operation` rather than the raw request.
#[derive(Debug)]
pub struct Operation {
    /// HTTP method of the request.
    pub method: HttpMethod,
    /// The resource being operated on.
    pub target: ResourceIdentifier,
    /// Request body for write operations (`PUT`, `POST`, `PATCH`).
    /// `None` for read-only methods.
    pub body: Option<Representation>,
    /// Content negotiation preferences from the `Accept` header.
    pub preferences: ContentPreferences,
    /// Credentials extracted by the authentication layer.
    pub credentials: AgentCredentials,
}

impl Operation {
    /// Convenience constructor for read operations (no body).
    pub fn read(
        method: HttpMethod,
        target: impl Into<ResourceIdentifier>,
        preferences: ContentPreferences,
    ) -> Self {
        Self {
            method,
            target: target.into(),
            body: None,
            preferences,
            credentials: AgentCredentials::anonymous(),
        }
    }

    /// Convenience constructor for write operations (with body).
    pub fn write(
        method: HttpMethod,
        target: impl Into<ResourceIdentifier>,
        body: Representation,
    ) -> Self {
        Self {
            method,
            target: target.into(),
            body: Some(body),
            preferences: ContentPreferences::default(),
            credentials: AgentCredentials::anonymous(),
        }
    }

    /// Returns `true` for methods that carry a body (`PUT`, `POST`, `PATCH`).
    pub fn is_write(&self) -> bool {
        matches!(self.method, HttpMethod::Put | HttpMethod::Post | HttpMethod::Patch)
    }

    /// Returns the implied [`AccessMode`]s for this operation.
    pub fn required_access_modes(&self) -> Vec<AccessMode> {
        match self.method {
            HttpMethod::Get | HttpMethod::Head => vec![AccessMode::Read],
            HttpMethod::Put | HttpMethod::Patch => vec![AccessMode::Write],
            HttpMethod::Post => vec![AccessMode::Append],
            HttpMethod::Delete => vec![AccessMode::Write],
            HttpMethod::Options => vec![],
        }
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_header_parsing() {
        let prefs = ContentPreferences::from_accept_header(
            "text/turtle;q=0.9, application/ld+json, */*;q=0.1",
        );
        assert_eq!(prefs.type_preferences[0].media_type, "application/ld+json");
        assert_eq!(prefs.type_preferences[1].media_type, "text/turtle");
        assert_eq!(prefs.type_preferences[2].media_type, "*/*");
    }

    #[test]
    fn most_preferred() {
        let prefs = ContentPreferences::from_accept_header("text/turtle");
        assert_eq!(prefs.most_preferred(), Some("text/turtle"));
    }

    #[test]
    fn accepts_any_wildcard() {
        let prefs = ContentPreferences::from_accept_header("*/*");
        assert!(prefs.accepts_any());
    }

    #[test]
    fn required_access_modes() {
        let op = Operation::read(
            HttpMethod::Get,
            "http://localhost/resource",
            ContentPreferences::default(),
        );
        assert_eq!(op.required_access_modes(), vec![AccessMode::Read]);
    }
}
