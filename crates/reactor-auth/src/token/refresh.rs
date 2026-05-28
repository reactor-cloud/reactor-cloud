//! Refresh token management.

use crate::crypto::{generate_token_base64url, sha256};
use chrono::{DateTime, Duration, Utc};
use reactor_core::ReactorId;

/// Refresh token prefix.
pub const REFRESH_TOKEN_PREFIX: &str = "rrf_";

/// Generate a new refresh token.
///
/// Returns the raw token (to send to client) and its hash (to store in DB).
pub fn generate_refresh_token() -> (String, [u8; 32]) {
    let random = generate_token_base64url(32);
    let token = format!("{}{}", REFRESH_TOKEN_PREFIX, random);
    let hash = sha256(token.as_bytes());
    (token, hash)
}

/// Hash a refresh token for lookup.
pub fn hash_refresh_token(token: &str) -> [u8; 32] {
    sha256(token.as_bytes())
}

/// Check if a string looks like a refresh token.
pub fn is_refresh_token(s: &str) -> bool {
    s.starts_with(REFRESH_TOKEN_PREFIX)
}

/// Refresh token metadata.
#[derive(Debug, Clone)]
pub struct RefreshTokenData {
    /// Token ID.
    pub id: ReactorId,
    /// Token hash.
    pub token_hash: [u8; 32],
    /// Expiration time.
    pub expires_at: DateTime<Utc>,
}

impl RefreshTokenData {
    /// Create new refresh token data with the given TTL.
    pub fn new(ttl_secs: u64) -> (String, Self) {
        let (token, hash) = generate_refresh_token();
        let data = Self {
            id: ReactorId::new(),
            token_hash: hash,
            expires_at: Utc::now() + Duration::seconds(ttl_secs as i64),
        };
        (token, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_refresh_token() {
        let (token, hash) = generate_refresh_token();

        assert!(token.starts_with(REFRESH_TOKEN_PREFIX));
        assert_eq!(hash.len(), 32);

        // Hash should match
        let computed_hash = hash_refresh_token(&token);
        assert_eq!(hash, computed_hash);
    }

    #[test]
    fn test_is_refresh_token() {
        assert!(is_refresh_token("rrf_abc123"));
        assert!(!is_refresh_token("abc123"));
        assert!(!is_refresh_token("jwt.token.here"));
    }

    #[test]
    fn test_different_tokens_different_hashes() {
        let (token1, hash1) = generate_refresh_token();
        let (token2, hash2) = generate_refresh_token();

        assert_ne!(token1, token2);
        assert_ne!(hash1, hash2);
    }
}
