//! JSON Web Key Set (JWKS) types.

use serde::{Deserialize, Serialize};

/// A JSON Web Key Set containing public keys for JWT verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwks {
    /// The set of keys.
    pub keys: Vec<JsonWebKey>,
}

impl Jwks {
    /// Create a new empty JWKS.
    #[must_use]
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Find a key by its key ID.
    #[must_use]
    pub fn find_key(&self, kid: &str) -> Option<&JsonWebKey> {
        self.keys.iter().find(|k| k.kid.as_deref() == Some(kid))
    }

    /// Add a key to the set.
    pub fn add_key(&mut self, key: JsonWebKey) {
        self.keys.push(key);
    }
}

impl Default for Jwks {
    fn default() -> Self {
        Self::new()
    }
}

/// A single JSON Web Key.
///
/// Currently only RSA keys are supported (for RS256).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonWebKey {
    /// Key type — always "RSA" for our use case.
    pub kty: String,

    /// Key use — "sig" for signing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "use")]
    pub key_use: Option<String>,

    /// Key ID — unique identifier for key rotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,

    /// Algorithm — "RS256" for our use case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alg: Option<String>,

    /// RSA modulus (base64url encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<String>,

    /// RSA public exponent (base64url encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e: Option<String>,
}

impl JsonWebKey {
    /// Create a new RSA public key for RS256.
    #[must_use]
    pub fn rsa(kid: String, n: String, e: String) -> Self {
        Self {
            kty: "RSA".to_string(),
            key_use: Some("sig".to_string()),
            kid: Some(kid),
            alg: Some("RS256".to_string()),
            n: Some(n),
            e: Some(e),
        }
    }
}
