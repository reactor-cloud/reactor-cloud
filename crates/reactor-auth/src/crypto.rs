//! Cryptographic utilities for reactor-auth.
//!
//! This module provides column encryption for sensitive data like private keys.
//! Two implementations are available:
//!
//! - `ColumnEncryptor`: Static key encryption (legacy, for backwards compatibility)
//! - `VaultEncryptor`: Vault-backed transit encryption (recommended for production)
//!
//! # Migration
//!
//! To migrate from static key to vault encryption:
//! 1. Deploy with vault configured but keep the data_key in config
//! 2. Re-encrypt existing data using a migration script
//! 3. Remove data_key from config once all data is migrated

use std::sync::Arc;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use thiserror::Error;

use reactor_core::primitives::vault::Vault;
use reactor_core::ProjectId;

/// Encryption errors.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Invalid key format.
    #[error("invalid key: {0}")]
    InvalidKey(String),

    /// Encryption failed.
    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    /// Decryption failed.
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    /// Invalid ciphertext format.
    #[error("invalid ciphertext format")]
    InvalidCiphertext,

    /// Vault error.
    #[error("vault error: {0}")]
    Vault(String),
}

impl From<reactor_core::primitives::vault::VaultError> for CryptoError {
    fn from(e: reactor_core::primitives::vault::VaultError) -> Self {
        CryptoError::Vault(e.to_string())
    }
}

/// AES-256-GCM encryptor for column encryption.
#[derive(Clone)]
pub struct ColumnEncryptor {
    cipher: Aes256Gcm,
}

impl ColumnEncryptor {
    /// Create a new encryptor from a base64-encoded 32-byte key.
    pub fn new(key_base64: &str) -> Result<Self, CryptoError> {
        let key_bytes = BASE64
            .decode(key_base64)
            .map_err(|e| CryptoError::InvalidKey(format!("invalid base64: {e}")))?;

        if key_bytes.len() != 32 {
            return Err(CryptoError::InvalidKey(format!(
                "expected 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;

        Ok(Self { cipher })
    }

    /// Encrypt plaintext, returning base64-encoded ciphertext with prepended nonce.
    ///
    /// Format: base64(nonce || ciphertext || tag)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String, CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(&result))
    }

    /// Decrypt base64-encoded ciphertext with prepended nonce.
    pub fn decrypt(&self, ciphertext_base64: &str) -> Result<Vec<u8>, CryptoError> {
        let data = BASE64
            .decode(ciphertext_base64)
            .map_err(|_| CryptoError::InvalidCiphertext)?;

        if data.len() < 12 {
            return Err(CryptoError::InvalidCiphertext);
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }

    /// Encrypt a string, returning base64-encoded ciphertext.
    pub fn encrypt_string(&self, plaintext: &str) -> Result<String, CryptoError> {
        self.encrypt(plaintext.as_bytes())
    }

    /// Decrypt to a string.
    pub fn decrypt_string(&self, ciphertext_base64: &str) -> Result<String, CryptoError> {
        let plaintext = self.decrypt(ciphertext_base64)?;
        String::from_utf8(plaintext).map_err(|_| CryptoError::DecryptionFailed("invalid utf-8".to_string()))
    }
}

/// Generate a cryptographically secure random token.
pub fn generate_token(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

/// Generate a token and encode as base64url.
pub fn generate_token_base64url(len: usize) -> String {
    let bytes = generate_token(len);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Compute SHA-256 hash of data.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

// =============================================================================
// Async Data Encryption
// =============================================================================

/// Transit key name for auth data encryption.
const AUTH_TRANSIT_KEY: &str = "auth/data";

/// Async data encryptor trait.
///
/// Abstracts encryption operations for sensitive data, allowing both
/// synchronous (static key) and asynchronous (vault) implementations.
#[async_trait]
pub trait DataEncryptor: Send + Sync {
    /// Encrypt plaintext bytes.
    async fn encrypt(&self, plaintext: &[u8]) -> Result<String, CryptoError>;

    /// Decrypt ciphertext to bytes.
    async fn decrypt(&self, ciphertext: &str) -> Result<Vec<u8>, CryptoError>;

    /// Encrypt a string.
    async fn encrypt_string(&self, plaintext: &str) -> Result<String, CryptoError> {
        self.encrypt(plaintext.as_bytes()).await
    }

    /// Decrypt to a string.
    async fn decrypt_string(&self, ciphertext: &str) -> Result<String, CryptoError> {
        let plaintext = self.decrypt(ciphertext).await?;
        String::from_utf8(plaintext).map_err(|_| CryptoError::DecryptionFailed("invalid utf-8".to_string()))
    }
}

/// Adapter to make `ColumnEncryptor` implement `DataEncryptor`.
///
/// This allows existing deployments using static keys to work with the new
/// async interface without code changes.
#[async_trait]
impl DataEncryptor for ColumnEncryptor {
    async fn encrypt(&self, plaintext: &[u8]) -> Result<String, CryptoError> {
        ColumnEncryptor::encrypt(self, plaintext)
    }

    async fn decrypt(&self, ciphertext: &str) -> Result<Vec<u8>, CryptoError> {
        ColumnEncryptor::decrypt(self, ciphertext)
    }
}

/// Vault-backed transit encryptor.
///
/// Uses the vault's transit encryption engine for data encryption.
/// Each tenant has isolated transit keys.
pub struct VaultEncryptor {
    /// The vault backend.
    vault: Arc<dyn Vault>,
    /// Project ID for tenant scoping.
    tenant: ProjectId,
}

impl VaultEncryptor {
    /// Create a new vault encryptor.
    pub fn new(vault: Arc<dyn Vault>, tenant: ProjectId) -> Self {
        Self { vault, tenant }
    }
}

#[async_trait]
impl DataEncryptor for VaultEncryptor {
    async fn encrypt(&self, plaintext: &[u8]) -> Result<String, CryptoError> {
        let ciphertext = self
            .vault
            .encrypt(&self.tenant, AUTH_TRANSIT_KEY, plaintext)
            .await?;

        // Return the ciphertext data which is already a string
        Ok(ciphertext.data)
    }

    async fn decrypt(&self, ciphertext: &str) -> Result<Vec<u8>, CryptoError> {
        let ct = reactor_core::primitives::vault::Ciphertext::new(ciphertext);

        self.vault
            .decrypt(&self.tenant, AUTH_TRANSIT_KEY, &ct)
            .await
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> String {
        BASE64.encode(&[0u8; 32])
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let encryptor = ColumnEncryptor::new(&test_key()).unwrap();
        let plaintext = b"hello world";

        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_string_roundtrip() {
        let encryptor = ColumnEncryptor::new(&test_key()).unwrap();
        let plaintext = "secret data 🔐";

        let ciphertext = encryptor.encrypt_string(plaintext).unwrap();
        let decrypted = encryptor.decrypt_string(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_ciphertexts() {
        let encryptor = ColumnEncryptor::new(&test_key()).unwrap();
        let plaintext = b"same input";

        let ct1 = encryptor.encrypt(plaintext).unwrap();
        let ct2 = encryptor.encrypt(plaintext).unwrap();

        // Due to random nonce, ciphertexts should differ
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = BASE64.encode(&[0u8; 16]);
        let result = ColumnEncryptor::new(&short_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_token() {
        let token = generate_token(32);
        assert_eq!(token.len(), 32);
    }
}
