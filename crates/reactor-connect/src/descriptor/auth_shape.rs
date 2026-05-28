//! Authentication shape utilities.

use super::{AuthDescriptor, AuthKind};

impl AuthDescriptor {
    /// Check if this auth requires OAuth2 flow.
    pub fn requires_oauth(&self) -> bool {
        matches!(self.kind, AuthKind::OAuth2 { .. })
    }

    /// Get the OAuth2 configuration if applicable.
    pub fn oauth_config(&self) -> Option<OAuth2Config> {
        match &self.kind {
            AuthKind::OAuth2 {
                authorize_url,
                token_url,
                scopes,
                pkce,
                ..
            } => Some(OAuth2Config {
                authorize_url: authorize_url.clone(),
                token_url: token_url.clone(),
                scopes: scopes.clone(),
                pkce: *pkce,
            }),
            _ => None,
        }
    }
}

/// OAuth2 configuration extracted from AuthDescriptor.
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    /// Authorization URL.
    pub authorize_url: String,
    /// Token URL.
    pub token_url: String,
    /// Required scopes.
    pub scopes: Vec<String>,
    /// Whether to use PKCE.
    pub pkce: bool,
}
