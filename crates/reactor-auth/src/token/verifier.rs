//! JWT token verification.

use crate::token::keyring::{KeyError, Keyring};
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
use reactor_core::auth::{AuthError, Claims};
use thiserror::Error;

/// Token verification errors.
#[derive(Debug, Error)]
pub enum VerifierError {
    /// Token has expired.
    #[error("token expired")]
    Expired,

    /// Invalid token signature.
    #[error("invalid signature")]
    InvalidSignature,

    /// Invalid token format.
    #[error("invalid token format: {0}")]
    InvalidFormat(String),

    /// Unknown signing key.
    #[error("unknown signing key: {0}")]
    UnknownKey(String),

    /// Invalid audience.
    #[error("invalid audience")]
    InvalidAudience,

    /// Invalid issuer.
    #[error("invalid issuer")]
    InvalidIssuer,

    /// Key error.
    #[error("key error: {0}")]
    Key(#[from] KeyError),
}

impl From<VerifierError> for AuthError {
    fn from(e: VerifierError) -> Self {
        match e {
            VerifierError::Expired => AuthError::TokenExpired,
            _ => AuthError::InvalidToken,
        }
    }
}

/// JWT token verifier.
pub struct TokenVerifier {
    issuer: String,
    audience: String,
}

impl TokenVerifier {
    /// Create a new token verifier.
    pub fn new(issuer: String, audience: String) -> Self {
        Self { issuer, audience }
    }

    /// Verify a token and extract claims.
    pub fn verify(&self, keyring: &Keyring, token: &str) -> Result<Claims, VerifierError> {
        // Decode header to get kid
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| VerifierError::InvalidFormat(e.to_string()))?;

        let kid = header
            .kid
            .ok_or_else(|| VerifierError::InvalidFormat("missing kid".to_string()))?;

        // Find the key
        let key = keyring
            .find_key(&kid)
            .ok_or(VerifierError::UnknownKey(kid))?;

        // Build validation
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        // Decode and verify
        let decoding_key = DecodingKey::from_rsa_pem(key.public_key_pem()?.as_bytes())
            .map_err(|e| VerifierError::InvalidFormat(e.to_string()))?;

        let token_data: TokenData<Claims> =
            decode(token, &decoding_key, &validation).map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => VerifierError::Expired,
                jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                    VerifierError::InvalidSignature
                }
                jsonwebtoken::errors::ErrorKind::InvalidAudience => VerifierError::InvalidAudience,
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => VerifierError::InvalidIssuer,
                _ => VerifierError::InvalidFormat(e.to_string()),
            })?;

        Ok(token_data.claims)
    }
}
