//! Mock vault adapter for testing.
//!
//! This adapter stores everything in memory and provides no real encryption.
//! It's intended only for tests and should never be used in production.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;

use reactor_core::primitives::vault::{
    Ciphertext, SecretMetadata, SecretValue, Vault, VaultError,
};
use reactor_core::ProjectId;

/// Mock vault for testing.
///
/// This vault provides no real security - it's purely for testing purposes.
/// All "encryption" is just base64 encoding, and secrets are stored in memory.
#[derive(Default)]
pub struct MockVault {
    secrets: Arc<RwLock<HashMap<(String, String), StoredSecret>>>,
    key_versions: Arc<RwLock<HashMap<(String, String), u32>>>,
    sealed: Arc<RwLock<bool>>,
    healthy: Arc<RwLock<bool>>,
}

#[derive(Clone)]
struct StoredSecret {
    value: SecretValue,
}

impl MockVault {
    /// Create a new mock vault.
    pub fn new() -> Self {
        Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
            key_versions: Arc::new(RwLock::new(HashMap::new())),
            sealed: Arc::new(RwLock::new(false)),
            healthy: Arc::new(RwLock::new(true)),
        }
    }

    /// Set the sealed state for testing.
    pub async fn set_sealed(&self, sealed: bool) {
        let mut s = self.sealed.write().await;
        *s = sealed;
    }

    /// Set the healthy state for testing.
    pub async fn set_healthy(&self, healthy: bool) {
        let mut h = self.healthy.write().await;
        *h = healthy;
    }

    fn tenant_key(tenant: &ProjectId) -> String {
        tenant.to_string()
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
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let tenant_key = Self::tenant_key(tenant);
        
        // Get or create key version
        let version = {
            let mut versions = self.key_versions.write().await;
            *versions.entry((tenant_key.clone(), key.to_string())).or_insert(1)
        };

        // "Encrypt" by base64 encoding (this is NOT secure - for testing only!)
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(plaintext);

        Ok(Ciphertext {
            data: encoded,
            key_version: Some(version),
        })
    }

    async fn decrypt(
        &self,
        _tenant: &ProjectId,
        _key: &str,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        // "Decrypt" by base64 decoding
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&ciphertext.data)
            .map_err(|e| VaultError::DecryptionFailed(format!("Decryption failed: {}", e)))
    }

    async fn rotate_key(&self, tenant: &ProjectId, key: &str) -> Result<u32, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let tenant_key = Self::tenant_key(tenant);
        let mut versions = self.key_versions.write().await;

        let entry = versions.entry((tenant_key, key.to_string())).or_insert(1);
        *entry += 1;

        Ok(*entry)
    }

    async fn get_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
    ) -> Result<Option<SecretValue>, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let tenant_key = Self::tenant_key(tenant);
        let secrets = self.secrets.read().await;

        Ok(secrets
            .get(&(tenant_key, name.to_string()))
            .map(|s| s.value.clone()))
    }

    async fn put_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
        value: SecretValue,
    ) -> Result<(), VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let tenant_key = Self::tenant_key(tenant);
        let mut secrets = self.secrets.write().await;

        // Increment version if exists
        let new_version = secrets
            .get(&(tenant_key.clone(), name.to_string()))
            .map(|s| s.value.version + 1)
            .unwrap_or(1);

        let stored = StoredSecret {
            value: SecretValue {
                data: value.data,
                version: new_version,
                created_at: secrets
                    .get(&(tenant_key.clone(), name.to_string()))
                    .map(|s| s.value.created_at)
                    .unwrap_or_else(Utc::now),
                updated_at: Utc::now(),
            },
        };

        secrets.insert((tenant_key, name.to_string()), stored);
        Ok(())
    }

    async fn list_secrets(&self, tenant: &ProjectId) -> Result<Vec<SecretMetadata>, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let tenant_key = Self::tenant_key(tenant);
        let secrets = self.secrets.read().await;

        let list = secrets
            .iter()
            .filter(|((t, _), _)| t == &tenant_key)
            .map(|((_, name), stored)| SecretMetadata {
                name: name.clone(),
                version: stored.value.version,
                created_at: stored.value.created_at,
                updated_at: stored.value.updated_at,
            })
            .collect();

        Ok(list)
    }

    async fn delete_secret(&self, tenant: &ProjectId, name: &str) -> Result<(), VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let tenant_key = Self::tenant_key(tenant);
        let mut secrets = self.secrets.write().await;

        if secrets.remove(&(tenant_key, name.to_string())).is_none() {
            return Err(VaultError::NotFound(name.to_string()));
        }

        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        *self.healthy.read().await && !*self.sealed.read().await
    }

    async fn is_sealed(&self) -> bool {
        *self.sealed.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_project_id() -> ProjectId {
        ProjectId::new()
    }

    #[tokio::test]
    async fn test_mock_encrypt_decrypt() {
        let vault = MockVault::new();
        let tenant = test_project_id();
        let plaintext = b"secret data";

        let ciphertext = vault.encrypt(&tenant, "key", plaintext).await.unwrap();
        let decrypted = vault.decrypt(&tenant, "key", &ciphertext).await.unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_mock_secrets() {
        let vault = MockVault::new();
        let tenant = test_project_id();

        let value = SecretValue {
            data: b"secret".to_vec(),
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        vault.put_secret(&tenant, "test", value.clone()).await.unwrap();

        let retrieved = vault.get_secret(&tenant, "test").await.unwrap().unwrap();
        assert_eq!(retrieved.data, value.data);
        assert_eq!(retrieved.version, 1);
    }

    #[tokio::test]
    async fn test_mock_sealed() {
        let vault = MockVault::new();
        let tenant = test_project_id();

        vault.set_sealed(true).await;

        let result = vault.encrypt(&tenant, "key", b"data").await;
        assert!(matches!(result, Err(VaultError::Sealed)));
    }
}
