//! Vault primitive for secret storage and transit encryption.
//!
//! This module defines the trait for vault operations. Implementations
//! (Embedded file-based, OpenBao) live in the `reactor-vault` crate.
//!
//! # Operations
//!
//! - **Transit**: Encrypt/decrypt data using tenant-scoped keys (the key never leaves the vault)
//! - **KV**: Store/retrieve arbitrary secrets per tenant
//!
//! # Tenant scoping
//!
//! All operations take a `ProjectId` parameter. In single-tenant mode,
//! this is always the same project ID. In multi-tenant mode, each tenant
//! has isolated keys and secrets.

use crate::project::ProjectId;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for vault operations.
#[derive(Debug, Error)]
pub enum VaultError {
    /// Encryption failed.
    #[error("encryption failed: {0}")]
    EncryptionFailed(String),
    /// Decryption failed.
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),
    /// Secret not found.
    #[error("secret not found: {0}")]
    NotFound(String),
    /// Invalid key name.
    #[error("invalid key name: {0}")]
    InvalidKeyName(String),
    /// Authentication failed.
    #[error("vault authentication failed: {0}")]
    AuthenticationFailed(String),
    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// Connection error.
    #[error("vault connection error: {0}")]
    Connection(String),
    /// Internal vault error.
    #[error("vault internal error: {0}")]
    Internal(String),
    /// Vault is sealed (OpenBao-specific).
    #[error("vault is sealed")]
    Sealed,
    /// Invalid configuration.
    #[error("invalid vault configuration: {0}")]
    Configuration(String),
}

/// Ciphertext returned by transit encryption.
///
/// This is an opaque blob that should be stored as-is and passed back
/// to `decrypt()` to recover the plaintext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ciphertext {
    /// The encrypted data (base64-encoded in most backends).
    pub data: String,
    /// Key version used for encryption (for key rotation).
    pub key_version: Option<u32>,
}

impl Ciphertext {
    /// Create a new ciphertext.
    #[must_use]
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            key_version: None,
        }
    }

    /// Create a ciphertext with key version.
    #[must_use]
    pub fn with_version(data: impl Into<String>, version: u32) -> Self {
        Self {
            data: data.into(),
            key_version: Some(version),
        }
    }
}

/// A secret value stored in the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretValue {
    /// The secret data (base64-encoded for JSON serialization).
    #[serde(with = "base64_serde")]
    pub data: Vec<u8>,
    /// Version of this secret (for optimistic locking).
    pub version: u64,
    /// When this secret was created.
    pub created_at: DateTime<Utc>,
    /// When this secret was last updated.
    pub updated_at: DateTime<Utc>,
}

impl SecretValue {
    /// Create a new secret value from bytes.
    #[must_use]
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        let now = Utc::now();
        Self {
            data: data.into(),
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new secret value from a string.
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
    }

    /// Get the secret data as a string (if valid UTF-8).
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }

    /// Get the secret data as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

mod base64_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        encoded.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use base64::Engine;
        let encoded = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .map_err(serde::de::Error::custom)
    }
}

/// Metadata about a secret (without the actual value).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Secret name.
    pub name: String,
    /// Current version.
    pub version: u64,
    /// When the secret was created.
    pub created_at: DateTime<Utc>,
    /// When the secret was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Vault trait for secret storage and transit encryption.
///
/// # Implementations
///
/// - `EmbeddedVault` — File-based AES-GCM encryption (single-node, no daemon)
/// - `OpenBaoVault` — OpenBao/Vault client (production)
/// - `MockVault` — In-memory for testing
///
/// All implementations live in the `reactor-vault` crate.
#[async_trait]
pub trait Vault: Send + Sync {
    // =========================================================================
    // Transit operations (encryption as a service)
    // =========================================================================

    /// Encrypt plaintext using a named transit key.
    ///
    /// The key is scoped to the tenant and never leaves the vault.
    ///
    /// # Arguments
    /// * `tenant` — Project ID for key scoping
    /// * `key` — Transit key name (e.g., "auth/data", "storage/signing")
    /// * `plaintext` — Data to encrypt
    ///
    /// # Returns
    /// Ciphertext that can be stored and later decrypted.
    async fn encrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        plaintext: &[u8],
    ) -> Result<Ciphertext, VaultError>;

    /// Decrypt ciphertext using a named transit key.
    ///
    /// # Arguments
    /// * `tenant` — Project ID for key scoping
    /// * `key` — Transit key name (must match the key used for encryption)
    /// * `ciphertext` — Previously encrypted data
    ///
    /// # Returns
    /// Original plaintext bytes.
    async fn decrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, VaultError>;

    /// Rotate a transit key to a new version.
    ///
    /// Old ciphertext can still be decrypted, but new encryptions use
    /// the new key version.
    async fn rotate_key(&self, tenant: &ProjectId, key: &str) -> Result<u32, VaultError>;

    // =========================================================================
    // KV operations (secret storage)
    // =========================================================================

    /// Get a secret by name.
    ///
    /// # Arguments
    /// * `tenant` — Project ID for namespace scoping
    /// * `name` — Secret name (e.g., "admin/token", "smtp/password")
    ///
    /// # Returns
    /// The secret value, or `None` if not found.
    async fn get_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
    ) -> Result<Option<SecretValue>, VaultError>;

    /// Store a secret.
    ///
    /// Creates or updates the secret at the given name.
    ///
    /// # Arguments
    /// * `tenant` — Project ID for namespace scoping
    /// * `name` — Secret name
    /// * `value` — Secret data
    async fn put_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
        value: SecretValue,
    ) -> Result<(), VaultError>;

    /// List secret names (not values) for a tenant.
    ///
    /// # Arguments
    /// * `tenant` — Project ID for namespace scoping
    ///
    /// # Returns
    /// List of secret metadata (names, versions, timestamps).
    async fn list_secrets(&self, tenant: &ProjectId) -> Result<Vec<SecretMetadata>, VaultError>;

    /// Delete a secret.
    ///
    /// # Arguments
    /// * `tenant` — Project ID for namespace scoping
    /// * `name` — Secret name to delete
    async fn delete_secret(&self, tenant: &ProjectId, name: &str) -> Result<(), VaultError>;

    // =========================================================================
    // Health and lifecycle
    // =========================================================================

    /// Check if the vault is healthy and accessible.
    async fn is_healthy(&self) -> bool;

    /// Check if the vault is sealed (OpenBao-specific).
    ///
    /// Returns `false` for embedded vaults that don't have a seal concept.
    async fn is_sealed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ciphertext_new() {
        let ct = Ciphertext::new("encrypted_data");
        assert_eq!(ct.data, "encrypted_data");
        assert!(ct.key_version.is_none());
    }

    #[test]
    fn test_ciphertext_with_version() {
        let ct = Ciphertext::with_version("encrypted_data", 3);
        assert_eq!(ct.data, "encrypted_data");
        assert_eq!(ct.key_version, Some(3));
    }

    #[test]
    fn test_secret_value_new() {
        let secret = SecretValue::from_str("my_secret");
        assert_eq!(secret.as_str(), Some("my_secret"));
        assert_eq!(secret.version, 1);
    }

    #[test]
    fn test_secret_value_as_bytes() {
        // Use invalid UTF-8 sequence (0xFF is not valid)
        let secret = SecretValue::new(vec![0xFF, 0xFE, 0x00]);
        assert_eq!(secret.as_bytes(), &[0xFF, 0xFE, 0x00]);
        assert!(secret.as_str().is_none()); // Not valid UTF-8
    }

    #[test]
    fn test_secret_value_serde() {
        let secret = SecretValue::from_str("test_secret");
        let json = serde_json::to_string(&secret).unwrap();
        let parsed: SecretValue = serde_json::from_str(&json).unwrap();
        assert_eq!(secret.as_str(), parsed.as_str());
        assert_eq!(secret.version, parsed.version);
    }

    #[test]
    fn test_ciphertext_serde() {
        let ct = Ciphertext::with_version("data", 2);
        let json = serde_json::to_string(&ct).unwrap();
        let parsed: Ciphertext = serde_json::from_str(&json).unwrap();
        assert_eq!(ct.data, parsed.data);
        assert_eq!(ct.key_version, parsed.key_version);
    }
}
