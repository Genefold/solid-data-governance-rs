//! HTTP operation and access mode types.

use crate::ResourceIdentifier;

/// Parsed HTTP method.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        let s = match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        };
        write!(f, "{s}")
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

/// Access modes used in WAC/ACP permission evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccessMode {
    Read,
    Write,
    Append,
    Control,
}

/// A parsed, validated HTTP operation ready to be dispatched.
#[derive(Debug)]
pub struct Operation {
    pub method: HttpMethod,
    pub target: ResourceIdentifier,
    pub preferences: ContentPreferences,
}

/// Negotiated content-type preferences from Accept headers.
#[derive(Debug, Default)]
pub struct ContentPreferences {
    pub type_preferences: Vec<String>,
}
