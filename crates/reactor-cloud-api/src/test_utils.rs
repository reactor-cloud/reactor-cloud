//! Test utilities for reactor-cloud-api.
//!
//! Provides mock implementations and test helpers.

use async_trait::async_trait;
use base64::Engine;
use reactor_core::primitives::vault::{Ciphertext, SecretMetadata, SecretValue, Vault, VaultError};
use reactor_core::ProjectId;
use std::collections::HashMap;
use std::sync::RwLock;

/// A mock vault implementation for testing.
///
/// Stores secrets in memory. Not suitable for production use.
pub struct MockVault {
    secrets: RwLock<HashMap<String, SecretValue>>,
    transit_keys: RwLock<HashMap<String, u32>>,
}

impl MockVault {
    /// Create a new mock vault.
    pub fn new() -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
            transit_keys: RwLock::new(HashMap::new()),
        }
    }

    fn secret_key(tenant: &ProjectId, name: &str) -> String {
        format!("{}:{}", tenant, name)
    }

    fn transit_key(tenant: &ProjectId, key: &str) -> String {
        format!("transit:{}:{}", tenant, key)
    }
}

impl Default for MockVault {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Vault for MockVault {
    async fn encrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        plaintext: &[u8],
    ) -> Result<Ciphertext, VaultError> {
        let transit_key = Self::transit_key(tenant, key);
        let keys = self.transit_keys.read().unwrap();

        let version = keys.get(&transit_key).copied().unwrap_or(0);
        if version == 0 {
            return Err(VaultError::NotFound(format!("transit key: {}", key)));
        }

        // Simple "encryption" for testing - just base64 encode with version prefix
        let encoded = base64::engine::general_purpose::STANDARD.encode(plaintext);
        Ok(Ciphertext::with_version(
            format!("mock:v{}:{}", version, encoded),
            version,
        ))
    }

    async fn decrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, VaultError> {
        let transit_key = Self::transit_key(tenant, key);
        let keys = self.transit_keys.read().unwrap();

        if !keys.contains_key(&transit_key) {
            return Err(VaultError::NotFound(format!("transit key: {}", key)));
        }

        // Simple "decryption" for testing
        let data = ciphertext
            .data
            .strip_prefix("mock:")
            .ok_or_else(|| VaultError::DecryptionFailed("invalid ciphertext format".to_string()))?;

        // Skip version prefix (vN:)
        let encoded = data
            .split(':')
            .nth(1)
            .ok_or_else(|| VaultError::DecryptionFailed("invalid ciphertext format".to_string()))?;

        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| VaultError::DecryptionFailed(e.to_string()))
    }

    async fn rotate_key(&self, tenant: &ProjectId, key: &str) -> Result<u32, VaultError> {
        let transit_key = Self::transit_key(tenant, key);
        let mut keys = self.transit_keys.write().unwrap();

        let version = keys.entry(transit_key).or_insert(0);
        *version += 1;
        Ok(*version)
    }

    async fn get_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
    ) -> Result<Option<SecretValue>, VaultError> {
        let key = Self::secret_key(tenant, name);
        let secrets = self.secrets.read().unwrap();
        Ok(secrets.get(&key).cloned())
    }

    async fn put_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
        value: SecretValue,
    ) -> Result<(), VaultError> {
        let key = Self::secret_key(tenant, name);
        let mut secrets = self.secrets.write().unwrap();
        secrets.insert(key, value);
        Ok(())
    }

    async fn list_secrets(&self, tenant: &ProjectId) -> Result<Vec<SecretMetadata>, VaultError> {
        let prefix = format!("{}:", tenant);
        let secrets = self.secrets.read().unwrap();

        Ok(secrets
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| SecretMetadata {
                name: k.strip_prefix(&prefix).unwrap_or(k).to_string(),
                version: v.version,
                created_at: v.created_at,
                updated_at: v.updated_at,
            })
            .collect())
    }

    async fn delete_secret(&self, tenant: &ProjectId, name: &str) -> Result<(), VaultError> {
        let key = Self::secret_key(tenant, name);
        let mut secrets = self.secrets.write().unwrap();
        secrets.remove(&key);
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_vault_secrets() {
        let vault = MockVault::new();
        let tenant = ProjectId::nil();

        // Put and get secret
        let secret = SecretValue::from_str("test-secret");
        vault.put_secret(&tenant, "my-key", secret).await.unwrap();

        let retrieved = vault.get_secret(&tenant, "my-key").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().as_str(), Some("test-secret"));

        // List secrets
        let list = vault.list_secrets(&tenant).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "my-key");

        // Delete secret
        vault.delete_secret(&tenant, "my-key").await.unwrap();
        let retrieved = vault.get_secret(&tenant, "my-key").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_mock_vault_transit() {
        let vault = MockVault::new();
        let tenant = ProjectId::nil();

        // Create key by rotating (version 0 -> 1)
        let version = vault.rotate_key(&tenant, "test-key").await.unwrap();
        assert_eq!(version, 1);

        // Encrypt
        let plaintext = b"hello world";
        let ciphertext = vault
            .encrypt(&tenant, "test-key", plaintext)
            .await
            .unwrap();
        assert!(ciphertext.data.starts_with("mock:v1:"));

        // Decrypt
        let decrypted = vault
            .decrypt(&tenant, "test-key", &ciphertext)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext);

        // Rotate and encrypt again
        let version = vault.rotate_key(&tenant, "test-key").await.unwrap();
        assert_eq!(version, 2);

        let ciphertext2 = vault
            .encrypt(&tenant, "test-key", plaintext)
            .await
            .unwrap();
        assert!(ciphertext2.data.starts_with("mock:v2:"));

        // Old ciphertext still decrypts
        let decrypted = vault
            .decrypt(&tenant, "test-key", &ciphertext)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
