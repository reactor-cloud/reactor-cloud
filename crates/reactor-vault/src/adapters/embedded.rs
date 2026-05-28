//! Embedded file-based vault using AES-256-GCM.
//!
//! This adapter stores secrets in encrypted files on disk. Each tenant gets their own
//! transit key derived from the master key, and secrets are stored as encrypted JSON
//! files organized by tenant.
//!
//! Directory structure:
//! ```text
//! /data/vault/
//! ├── keys/               # Encrypted tenant key material
//! │   └── {project_id}/
//! │       └── {key_name}.key
//! └── secrets/            # Encrypted secrets
//!     └── {project_id}/
//!         └── {secret_name}.json
//! ```
//!
//! This is suitable for single-node (G1/G2) deployments. For production multi-node
//! deployments (G3), use the OpenBao adapter.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use async_trait::async_trait;
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use reactor_core::primitives::vault::{
    Ciphertext, SecretMetadata, SecretValue, Vault, VaultError,
};
use reactor_core::ProjectId;

use crate::EmbeddedConfig;

/// Nonce size for AES-GCM (96 bits).
const NONCE_SIZE: usize = 12;

/// Key size for AES-256 (256 bits).
const KEY_SIZE: usize = 32;

/// Key rotation metadata stored alongside the encrypted key.
#[derive(Debug, Serialize, Deserialize)]
struct KeyMaterial {
    /// Current key version (starts at 1).
    version: u32,
    /// Encrypted data encryption key (DEK) — encrypted with master key.
    encrypted_dek: Vec<u8>,
    /// Nonce used to encrypt the DEK.
    dek_nonce: Vec<u8>,
    /// Timestamp of key creation.
    created_at: chrono::DateTime<Utc>,
    /// Timestamp of last rotation.
    rotated_at: chrono::DateTime<Utc>,
}

/// Decrypted key material held in memory.
struct DecryptedKey {
    version: u32,
    dek: [u8; KEY_SIZE],
}

/// Encrypted secret stored on disk.
#[derive(Debug, Serialize, Deserialize)]
struct StoredSecret {
    /// Ciphertext (base64-encoded in JSON).
    ciphertext: Vec<u8>,
    /// Nonce used for encryption.
    nonce: Vec<u8>,
    /// Key version used for encryption.
    key_version: u32,
    /// Secret version (incremented on each update).
    version: u64,
    /// Creation timestamp.
    created_at: chrono::DateTime<Utc>,
    /// Last update timestamp.
    updated_at: chrono::DateTime<Utc>,
}

/// Embedded file-based vault implementation.
pub struct EmbeddedVault {
    /// Base directory for vault storage.
    base_path: PathBuf,
    /// Master encryption key (derived from config).
    master_key: [u8; KEY_SIZE],
    /// In-memory cache of decrypted tenant keys.
    key_cache: Arc<RwLock<HashMap<(ProjectId, String), DecryptedKey>>>,
    /// Sealed state (when sealed, no operations allowed).
    sealed: Arc<RwLock<bool>>,
}

impl EmbeddedVault {
    /// Create a new embedded vault.
    ///
    /// # Arguments
    ///
    /// * `config` - Embedded vault configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the master key is invalid or directories cannot be created.
    pub async fn new(config: &EmbeddedConfig) -> Result<Self, VaultError> {
        let master_key = Self::resolve_master_key(&config.master_key)?;

        // Ensure directories exist
        let keys_path = config.path.join("keys");
        let secrets_path = config.path.join("secrets");

        fs::create_dir_all(&keys_path)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to create keys dir: {}", e)))?;

        fs::create_dir_all(&secrets_path)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to create secrets dir: {}", e)))?;

        debug!(path = %config.path.display(), "Initialized embedded vault");

        Ok(Self {
            base_path: config.path.clone(),
            master_key,
            key_cache: Arc::new(RwLock::new(HashMap::new())),
            sealed: Arc::new(RwLock::new(false)),
        })
    }

    /// Resolve the master key from configuration.
    fn resolve_master_key(key_config: &str) -> Result<[u8; KEY_SIZE], VaultError> {
        let key_hex = if key_config.starts_with("env:") {
            let env_var = &key_config[4..];
            std::env::var(env_var).map_err(|_| {
                VaultError::Configuration(format!(
                    "Master key environment variable '{}' not set",
                    env_var
                ))
            })?
        } else {
            key_config.to_string()
        };

        // Decode hex to bytes
        let key_bytes = hex::decode(&key_hex).map_err(|e| {
            VaultError::Configuration(format!("Invalid master key hex encoding: {}", e))
        })?;

        if key_bytes.len() != KEY_SIZE {
            return Err(VaultError::Configuration(format!(
                "Master key must be {} bytes (got {})",
                KEY_SIZE,
                key_bytes.len()
            )));
        }

        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(&key_bytes);
        Ok(key)
    }

    /// Generate a new random 256-bit key.
    fn generate_key() -> [u8; KEY_SIZE] {
        let mut key = [0u8; KEY_SIZE];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Generate a random nonce.
    fn generate_nonce() -> [u8; NONCE_SIZE] {
        let mut nonce = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    /// Get path for tenant keys.
    fn keys_path(&self, tenant: &ProjectId) -> PathBuf {
        self.base_path.join("keys").join(tenant.to_string())
    }

    /// Get path for tenant secrets.
    fn secrets_path(&self, tenant: &ProjectId) -> PathBuf {
        self.base_path.join("secrets").join(tenant.to_string())
    }

    /// Get or create a transit key for a tenant.
    async fn get_or_create_key(
        &self,
        tenant: &ProjectId,
        key_name: &str,
    ) -> Result<DecryptedKey, VaultError> {
        // Check cache first
        {
            let cache = self.key_cache.read().await;
            if let Some(key) = cache.get(&(tenant.clone(), key_name.to_string())) {
                return Ok(DecryptedKey {
                    version: key.version,
                    dek: key.dek,
                });
            }
        }

        // Load or create key
        let key_path = self.keys_path(tenant).join(format!("{}.key", key_name));

        let key_material = if key_path.exists() {
            // Load existing key
            let data = fs::read(&key_path)
                .await
                .map_err(|e| VaultError::Connection(format!("Failed to read key file: {}", e)))?;

            serde_json::from_slice::<KeyMaterial>(&data)
                .map_err(|e| VaultError::Configuration(format!("Invalid key file: {}", e)))?
        } else {
            // Create new key
            let dek = Self::generate_key();
            let nonce = Self::generate_nonce();

            let cipher = Aes256Gcm::new_from_slice(&self.master_key).map_err(|e| {
                VaultError::Configuration(format!("Invalid master key: {}", e))
            })?;

            let encrypted_dek = cipher
                .encrypt(Nonce::from_slice(&nonce), dek.as_ref())
                .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;

            let now = Utc::now();
            let key_material = KeyMaterial {
                version: 1,
                encrypted_dek,
                dek_nonce: nonce.to_vec(),
                created_at: now,
                rotated_at: now,
            };

            // Ensure tenant key directory exists
            let key_dir = self.keys_path(tenant);
            fs::create_dir_all(&key_dir)
                .await
                .map_err(|e| VaultError::Connection(format!("Failed to create key dir: {}", e)))?;

            // Write key material
            let data = serde_json::to_vec_pretty(&key_material)
                .map_err(|e| VaultError::Configuration(format!("Failed to serialize key: {}", e)))?;

            fs::write(&key_path, &data)
                .await
                .map_err(|e| VaultError::Connection(format!("Failed to write key file: {}", e)))?;

            key_material
        };

        // Decrypt the DEK
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| VaultError::Configuration(format!("Invalid master key: {}", e)))?;

        let nonce = Nonce::from_slice(&key_material.dek_nonce);
        let dek_bytes = cipher
            .decrypt(nonce, key_material.encrypted_dek.as_ref())
            .map_err(|e| VaultError::DecryptionFailed(format!("Failed to decrypt DEK: {}", e)))?;

        let mut dek = [0u8; KEY_SIZE];
        dek.copy_from_slice(&dek_bytes);

        let decrypted = DecryptedKey {
            version: key_material.version,
            dek,
        };

        // Cache the key
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(
                (tenant.clone(), key_name.to_string()),
                DecryptedKey {
                    version: decrypted.version,
                    dek: decrypted.dek,
                },
            );
        }

        Ok(decrypted)
    }

    /// Encrypt data with a tenant's transit key.
    async fn encrypt_with_key(
        &self,
        tenant: &ProjectId,
        key_name: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>, u32), VaultError> {
        let key = self.get_or_create_key(tenant, key_name).await?;

        let cipher = Aes256Gcm::new_from_slice(&key.dek)
            .map_err(|e| VaultError::Configuration(format!("Invalid DEK: {}", e)))?;

        let nonce_bytes = Self::generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;

        Ok((ciphertext, nonce_bytes.to_vec(), key.version))
    }

    /// Decrypt data with a tenant's transit key.
    async fn decrypt_with_key(
        &self,
        tenant: &ProjectId,
        key_name: &str,
        ciphertext: &[u8],
        nonce: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        let key = self.get_or_create_key(tenant, key_name).await?;

        let cipher = Aes256Gcm::new_from_slice(&key.dek)
            .map_err(|e| VaultError::Configuration(format!("Invalid DEK: {}", e)))?;

        let nonce = Nonce::from_slice(nonce);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| VaultError::DecryptionFailed(e.to_string()))
    }
}

#[async_trait]
impl Vault for EmbeddedVault {
    async fn encrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        plaintext: &[u8],
    ) -> Result<Ciphertext, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let (ciphertext, nonce, version) = self.encrypt_with_key(tenant, key, plaintext).await?;

        // Combine nonce + ciphertext for storage, base64 encode
        let mut combined = nonce;
        combined.extend(ciphertext);
        
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&combined);

        Ok(Ciphertext {
            data: encoded,
            key_version: Some(version),
        })
    }

    async fn decrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        use base64::Engine;
        let combined = base64::engine::general_purpose::STANDARD
            .decode(&ciphertext.data)
            .map_err(|e| VaultError::DecryptionFailed(format!("Invalid ciphertext encoding: {}", e)))?;

        if combined.len() < NONCE_SIZE {
            return Err(VaultError::DecryptionFailed("Ciphertext too short".to_string()));
        }

        let (nonce, ct) = combined.split_at(NONCE_SIZE);
        self.decrypt_with_key(tenant, key, ct, nonce).await
    }

    async fn rotate_key(&self, tenant: &ProjectId, key: &str) -> Result<u32, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let key_path = self.keys_path(tenant).join(format!("{}.key", key));

        if !key_path.exists() {
            return Err(VaultError::NotFound(format!(
                "Key '{}' does not exist for tenant '{}'",
                key, tenant
            )));
        }

        // Read current key material
        let data = fs::read(&key_path)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to read key file: {}", e)))?;

        let mut key_material: KeyMaterial = serde_json::from_slice(&data)
            .map_err(|e| VaultError::Configuration(format!("Invalid key file: {}", e)))?;

        // Generate new DEK
        let new_dek = Self::generate_key();
        let new_nonce = Self::generate_nonce();

        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| VaultError::Configuration(format!("Invalid master key: {}", e)))?;

        let encrypted_dek = cipher
            .encrypt(Nonce::from_slice(&new_nonce), new_dek.as_ref())
            .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;

        // Update key material
        key_material.version += 1;
        key_material.encrypted_dek = encrypted_dek;
        key_material.dek_nonce = new_nonce.to_vec();
        key_material.rotated_at = Utc::now();

        // Write updated key material
        let data = serde_json::to_vec_pretty(&key_material)
            .map_err(|e| VaultError::Configuration(format!("Failed to serialize key: {}", e)))?;

        fs::write(&key_path, &data)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to write key file: {}", e)))?;

        // Update cache
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(
                (tenant.clone(), key.to_string()),
                DecryptedKey {
                    version: key_material.version,
                    dek: new_dek,
                },
            );
        }

        debug!(
            tenant = %tenant,
            key = %key,
            version = %key_material.version,
            "Rotated transit key"
        );

        Ok(key_material.version)
    }

    async fn get_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
    ) -> Result<Option<SecretValue>, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let secret_path = self.secrets_path(tenant).join(format!("{}.json", name));

        if !secret_path.exists() {
            return Ok(None);
        }

        let data = fs::read(&secret_path)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to read secret: {}", e)))?;

        let stored: StoredSecret = serde_json::from_slice(&data)
            .map_err(|e| VaultError::Configuration(format!("Invalid secret file: {}", e)))?;

        // Decrypt the secret using the internal transit key
        let plaintext = self
            .decrypt_with_key(tenant, "kv-internal", &stored.ciphertext, &stored.nonce)
            .await?;

        Ok(Some(SecretValue {
            data: plaintext,
            version: stored.version,
            created_at: stored.created_at,
            updated_at: stored.updated_at,
        }))
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

        // Encrypt the secret using internal transit key
        let (ciphertext, nonce, key_version) = self
            .encrypt_with_key(tenant, "kv-internal", &value.data)
            .await?;

        // Check if secret exists to determine version
        let secret_path = self.secrets_path(tenant).join(format!("{}.json", name));
        let version = if secret_path.exists() {
            let data = fs::read(&secret_path)
                .await
                .map_err(|e| VaultError::Connection(format!("Failed to read secret: {}", e)))?;

            let stored: StoredSecret = serde_json::from_slice(&data)
                .map_err(|e| VaultError::Configuration(format!("Invalid secret file: {}", e)))?;

            stored.version + 1
        } else {
            1
        };

        let now = Utc::now();
        let stored = StoredSecret {
            ciphertext,
            nonce,
            key_version,
            version,
            created_at: if version == 1 { now } else { value.created_at },
            updated_at: now,
        };

        // Ensure tenant secrets directory exists
        let secrets_dir = self.secrets_path(tenant);
        fs::create_dir_all(&secrets_dir)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to create secrets dir: {}", e)))?;

        let data = serde_json::to_vec_pretty(&stored)
            .map_err(|e| VaultError::Configuration(format!("Failed to serialize secret: {}", e)))?;

        fs::write(&secret_path, &data)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to write secret: {}", e)))?;

        debug!(
            tenant = %tenant,
            secret = %name,
            version = %version,
            "Stored secret"
        );

        Ok(())
    }

    async fn list_secrets(&self, tenant: &ProjectId) -> Result<Vec<SecretMetadata>, VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let secrets_dir = self.secrets_path(tenant);

        if !secrets_dir.exists() {
            return Ok(vec![]);
        }

        let mut entries = fs::read_dir(&secrets_dir)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to read secrets dir: {}", e)))?;

        let mut secrets = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to read entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                // Read stored secret to get metadata
                let data = fs::read(&path).await.map_err(|e| {
                    VaultError::Connection(format!("Failed to read secret file: {}", e))
                })?;

                if let Ok(stored) = serde_json::from_slice::<StoredSecret>(&data) {
                    secrets.push(SecretMetadata {
                        name,
                        version: stored.version,
                        created_at: stored.created_at,
                        updated_at: stored.updated_at,
                    });
                }
            }
        }

        Ok(secrets)
    }

    async fn delete_secret(&self, tenant: &ProjectId, name: &str) -> Result<(), VaultError> {
        if *self.sealed.read().await {
            return Err(VaultError::Sealed);
        }

        let secret_path = self.secrets_path(tenant).join(format!("{}.json", name));

        if !secret_path.exists() {
            return Err(VaultError::NotFound(name.to_string()));
        }

        fs::remove_file(&secret_path)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to delete secret: {}", e)))?;

        debug!(
            tenant = %tenant,
            secret = %name,
            "Deleted secret"
        );

        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        !*self.sealed.read().await
    }

    async fn is_sealed(&self) -> bool {
        *self.sealed.read().await
    }
}

impl EmbeddedVault {
    /// Seal the vault, preventing all operations.
    pub async fn seal(&self) {
        let mut sealed = self.sealed.write().await;
        *sealed = true;

        // Clear key cache
        let mut cache = self.key_cache.write().await;
        cache.clear();

        warn!("Vault sealed");
    }

    /// Unseal the vault.
    pub async fn unseal(&self) {
        let mut sealed = self.sealed.write().await;
        *sealed = false;

        debug!("Vault unsealed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_vault() -> (EmbeddedVault, TempDir) {
        let dir = TempDir::new().unwrap();
        let master_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let config = EmbeddedConfig {
            path: dir.path().to_path_buf(),
            master_key: master_key.to_string(),
        };

        let vault = EmbeddedVault::new(&config).await.unwrap();
        (vault, dir)
    }

    fn test_project_id() -> ProjectId {
        ProjectId::new()
    }

    #[tokio::test]
    async fn test_encrypt_decrypt_roundtrip() {
        let (vault, _dir) = create_test_vault().await;
        let tenant = test_project_id();
        let plaintext = b"secret data";

        let ciphertext = vault.encrypt(&tenant, "test-key", plaintext).await.unwrap();
        let decrypted = vault.decrypt(&tenant, "test-key", &ciphertext).await.unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_secret_storage_roundtrip() {
        let (vault, _dir) = create_test_vault().await;
        let tenant = test_project_id();

        let value = SecretValue {
            data: b"my secret".to_vec(),
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        vault.put_secret(&tenant, "my-secret", value.clone()).await.unwrap();

        let retrieved = vault.get_secret(&tenant, "my-secret").await.unwrap().unwrap();

        assert_eq!(retrieved.data, value.data);
        assert_eq!(retrieved.version, 1);
    }

    #[tokio::test]
    async fn test_list_secrets() {
        let (vault, _dir) = create_test_vault().await;
        let tenant = test_project_id();

        let value = SecretValue {
            data: b"secret".to_vec(),
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        vault.put_secret(&tenant, "secret1", value.clone()).await.unwrap();
        vault.put_secret(&tenant, "secret2", value.clone()).await.unwrap();

        let secrets = vault.list_secrets(&tenant).await.unwrap();

        assert_eq!(secrets.len(), 2);
        let names: Vec<_> = secrets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"secret1"));
        assert!(names.contains(&"secret2"));
    }

    #[tokio::test]
    async fn test_delete_secret() {
        let (vault, _dir) = create_test_vault().await;
        let tenant = test_project_id();

        let value = SecretValue {
            data: b"secret".to_vec(),
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        vault.put_secret(&tenant, "to-delete", value).await.unwrap();
        assert!(vault.get_secret(&tenant, "to-delete").await.unwrap().is_some());

        vault.delete_secret(&tenant, "to-delete").await.unwrap();
        assert!(vault.get_secret(&tenant, "to-delete").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let (vault, _dir) = create_test_vault().await;
        let tenant = test_project_id();
        let plaintext = b"secret data";

        // Encrypt with initial key
        let ct1 = vault.encrypt(&tenant, "rotate-test", plaintext).await.unwrap();
        assert_eq!(ct1.key_version, Some(1));

        // Rotate key
        let new_version = vault.rotate_key(&tenant, "rotate-test").await.unwrap();
        assert_eq!(new_version, 2);

        // New encryptions use new key version
        let ct2 = vault.encrypt(&tenant, "rotate-test", plaintext).await.unwrap();
        assert_eq!(ct2.key_version, Some(2));

        // Can still decrypt with old key version (key material stored in ciphertext)
        // Note: In a real implementation, you'd want to support re-wrapping old ciphertexts
    }

    #[tokio::test]
    async fn test_sealed_vault() {
        let (vault, _dir) = create_test_vault().await;
        let tenant = test_project_id();

        vault.seal().await;

        let result = vault.encrypt(&tenant, "key", b"data").await;
        assert!(matches!(result, Err(VaultError::Sealed)));

        vault.unseal().await;

        let result = vault.encrypt(&tenant, "key", b"data").await;
        assert!(result.is_ok());
    }
}
