//! Credential management.

mod oauth2;
mod pat;
mod refresh;

pub use oauth2::*;
pub use pat::*;
pub use refresh::*;

use serde::{Deserialize, Serialize};

/// Credentials stored in vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credentials {
    /// OAuth2 credentials.
    OAuth2 {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        client_id: String,
        client_secret: String,
    },
    /// Personal access token.
    Pat {
        token: String,
    },
    /// HTTP Basic auth.
    Basic {
        username: String,
        password: String,
    },
    /// Custom credentials (JSON blob).
    Custom {
        data: serde_json::Value,
    },
}

impl Credentials {
    /// Check if credentials need refresh.
    pub fn needs_refresh(&self) -> bool {
        match self {
            Credentials::OAuth2 { expires_at, refresh_token, .. } => {
                if refresh_token.is_none() {
                    return false;
                }
                if let Some(exp) = expires_at {
                    let buffer = chrono::Duration::minutes(5);
                    *exp - buffer < chrono::Utc::now()
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
