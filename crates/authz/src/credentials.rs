//! Agent credential types extracted from a request.

/// Agent credentials extracted from the incoming HTTP request.
///
/// Mirrors the TypeScript `Credentials` interface.
#[derive(Debug, Clone, Default)]
pub struct Credentials {
    /// WebID of the authenticated agent, if any.
    pub web_id: Option<String>,
    /// Client identifier, if using client credentials.
    pub client_id: Option<String>,
}

impl Credentials {
    pub fn anonymous() -> Self {
        Self::default()
    }

    pub fn is_authenticated(&self) -> bool {
        self.web_id.is_some()
    }
}
