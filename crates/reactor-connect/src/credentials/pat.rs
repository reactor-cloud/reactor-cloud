//! Personal access token handling.

use crate::credentials::Credentials;

impl Credentials {
    /// Get the authorization header value for this credential.
    pub fn auth_header(&self, format: &str) -> Option<String> {
        match self {
            Credentials::Pat { token } => {
                Some(format.replace("{token}", token))
            }
            Credentials::OAuth2 { access_token, .. } => {
                Some(format.replace("{token}", access_token))
            }
            Credentials::Basic { username, password } => {
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("{}:{}", username, password),
                );
                Some(format!("Basic {}", encoded))
            }
            Credentials::Custom { .. } => None,
        }
    }
}
