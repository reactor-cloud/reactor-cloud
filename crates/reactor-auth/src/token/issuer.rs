//! JWT token issuance.

use crate::token::keyring::{KeyError, Keyring};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reactor_core::auth::{AuthMethod, Claims};
use reactor_core::id::{OrgId, SessionId, UserId};
use thiserror::Error;

/// Token issuance errors.
#[derive(Debug, Error)]
pub enum IssuerError {
    /// No active signing key.
    #[error("no active signing key")]
    NoActiveKey,

    /// Key error.
    #[error("key error: {0}")]
    Key(#[from] KeyError),

    /// JWT encoding failed.
    #[error("JWT encoding failed: {0}")]
    Encoding(String),
}

/// JWT token issuer.
pub struct TokenIssuer {
    issuer: String,
    audience: String,
    access_ttl_secs: u64,
}

impl TokenIssuer {
    /// Create a new token issuer.
    pub fn new(issuer: String, audience: String, access_ttl_secs: u64) -> Self {
        Self {
            issuer,
            audience,
            access_ttl_secs,
        }
    }

    /// Issue an access token for a user session.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_access_token(
        &self,
        keyring: &Keyring,
        user_id: UserId,
        email: Option<String>,
        session_id: SessionId,
        amr: Vec<AuthMethod>,
        orgs: Vec<OrgId>,
        default_org: Option<OrgId>,
    ) -> Result<String, IssuerError> {
        self.issue_access_token_with_scopes(
            keyring, user_id, email, session_id, amr, orgs, default_org, vec![], None,
        )
    }

    /// Issue an access token for a user session with scopes and MFA timestamp.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_access_token_with_scopes(
        &self,
        keyring: &Keyring,
        user_id: UserId,
        email: Option<String>,
        session_id: SessionId,
        amr: Vec<AuthMethod>,
        orgs: Vec<OrgId>,
        default_org: Option<OrgId>,
        scopes: Vec<String>,
        mfa_at: Option<i64>,
    ) -> Result<String, IssuerError> {
        let key = keyring.active_key().ok_or(IssuerError::NoActiveKey)?;

        let now = Utc::now();
        let exp = now + Duration::seconds(self.access_ttl_secs as i64);

        let claims = Claims {
            sub: format!("user_{}", user_id),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: Some(now.timestamp()),
            email,
            amr,
            orgs,
            default_org,
            session_id: Some(session_id),
            scopes,
            mfa_at,
        };

        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        let encoding_key = EncodingKey::from_rsa_pem(key.private_key_pem()?.as_bytes())
            .map_err(|e| IssuerError::Encoding(e.to_string()))?;

        encode(&header, &claims, &encoding_key).map_err(|e| IssuerError::Encoding(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issuer_creation() {
        let issuer = TokenIssuer::new("reactor-auth".to_string(), "reactor".to_string(), 3600);
        assert_eq!(issuer.access_ttl_secs, 3600);
    }
}
