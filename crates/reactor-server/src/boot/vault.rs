//! Vault initialization and management for reactor-server.
//!
//! This module provides:
//! - Vault initialization from configuration (Embedded or OpenBao backend)
//! - TTL-based caching of resolved secrets
//! - Helper functions for common secret operations
//!
//! # Configuration
//!
//! ```toml
//! # Embedded (file-based) vault
//! [vault]
//! backend = "embedded"
//! path = ".reactor/vault"
//! master_key = "env:REACTOR_VAULT_MASTER_KEY"
//!
//! # OpenBao vault
//! [vault]
//! backend = "openbao"
//! address = "https://vault.internal:8200"
//! auth_method = "approle"
//! role_id = "xxx"
//! secret_id_file = "/run/secrets/vault-secret-id"
//! ```

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use tracing::{debug, info, warn};

use crate::config::{ReactorConfig, VaultConfigSlice};
use crate::error::ServerError;
use reactor_core::primitives::vault::{SecretValue, Vault, VaultError};
use reactor_core::ProjectId;
use reactor_vault::{EmbeddedConfig, EmbeddedVault, MockVault};

#[cfg(feature = "openbao")]
use reactor_vault::{OpenBaoAuth, OpenBaoConfig, OpenBaoVault};

/// Cached vault wrapper providing TTL-based caching of secrets.
///
/// This wrapper caches `get_secret` results to reduce load on the vault backend.
/// Transit operations (encrypt/decrypt) are not cached as they're typically
/// called less frequently and caching ciphertext would be a security risk.
pub struct CachedVault {
    /// Underlying vault implementation.
    inner: Arc<dyn Vault>,
    /// Secret cache (project_id/name -> SecretValue).
    cache: Cache<String, SecretValue>,
    /// Cache TTL.
    ttl: Duration,
}

impl CachedVault {
    /// Create a new cached vault wrapper.
    pub fn new(vault: Arc<dyn Vault>, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .time_to_live(ttl)
            .max_capacity(1000)
            .build();

        Self {
            inner: vault,
            cache,
            ttl,
        }
    }

    /// Get the cache key for a secret.
    fn cache_key(tenant: &ProjectId, name: &str) -> String {
        format!("{}:{}", tenant, name)
    }

    /// Get the underlying vault.
    pub fn inner(&self) -> &Arc<dyn Vault> {
        &self.inner
    }
}

#[async_trait::async_trait]
impl Vault for CachedVault {
    async fn encrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        plaintext: &[u8],
    ) -> Result<reactor_core::primitives::vault::Ciphertext, VaultError> {
        self.inner.encrypt(tenant, key, plaintext).await
    }

    async fn decrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        ciphertext: &reactor_core::primitives::vault::Ciphertext,
    ) -> Result<Vec<u8>, VaultError> {
        self.inner.decrypt(tenant, key, ciphertext).await
    }

    async fn rotate_key(&self, tenant: &ProjectId, key: &str) -> Result<u32, VaultError> {
        self.inner.rotate_key(tenant, key).await
    }

    async fn get_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
    ) -> Result<Option<SecretValue>, VaultError> {
        let key = Self::cache_key(tenant, name);

        // Check cache first
        if let Some(cached) = self.cache.get(&key).await {
            debug!(tenant = %tenant, name = %name, "secret cache hit");
            return Ok(Some(cached));
        }

        // Fetch from vault
        let result = self.inner.get_secret(tenant, name).await?;

        // Cache if found
        if let Some(ref value) = result {
            self.cache.insert(key, value.clone()).await;
            debug!(tenant = %tenant, name = %name, ttl_secs = %self.ttl.as_secs(), "secret cached");
        }

        Ok(result)
    }

    async fn put_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
        value: SecretValue,
    ) -> Result<(), VaultError> {
        // Invalidate cache
        let key = Self::cache_key(tenant, name);
        self.cache.invalidate(&key).await;

        self.inner.put_secret(tenant, name, value).await
    }

    async fn list_secrets(
        &self,
        tenant: &ProjectId,
    ) -> Result<Vec<reactor_core::primitives::vault::SecretMetadata>, VaultError> {
        self.inner.list_secrets(tenant).await
    }

    async fn delete_secret(&self, tenant: &ProjectId, name: &str) -> Result<(), VaultError> {
        // Invalidate cache
        let key = Self::cache_key(tenant, name);
        self.cache.invalidate(&key).await;

        self.inner.delete_secret(tenant, name).await
    }

    async fn is_healthy(&self) -> bool {
        self.inner.is_healthy().await
    }

    async fn is_sealed(&self) -> bool {
        self.inner.is_sealed().await
    }
}

/// Build the vault from configuration.
///
/// Supports:
/// - `embedded` — File-based AES-GCM vault (default)
/// - `openbao` — OpenBao/HashiCorp Vault client
///
/// If no configuration is provided, uses embedded vault with master key
/// from `REACTOR_VAULT_MASTER_KEY` environment variable.
pub async fn build_vault(config: &ReactorConfig) -> Result<Arc<dyn Vault>, ServerError> {
    let vault_config = config.vault.clone().unwrap_or_default();
    let cache_ttl = Duration::from_secs(vault_config.cache_ttl_secs);

    let vault: Arc<dyn Vault> = match vault_config.backend.as_str() {
        "embedded" => {
            let embedded = build_embedded_vault(&vault_config).await?;
            Arc::new(embedded)
        }
        #[cfg(feature = "openbao")]
        "openbao" => {
            let openbao = build_openbao_vault(&vault_config).await?;
            Arc::new(openbao)
        }
        #[cfg(not(feature = "openbao"))]
        "openbao" => {
            return Err(ServerError::Config(
                "openbao backend requires 'openbao' feature".to_string(),
            ));
        }
        "mock" => {
            warn!("using mock vault - NOT FOR PRODUCTION");
            Arc::new(MockVault::new())
        }
        other => {
            return Err(ServerError::Config(format!(
                "unknown vault backend: {} (expected 'embedded', 'openbao', or 'mock')",
                other
            )));
        }
    };

    // Wrap with caching layer
    let cached = CachedVault::new(vault, cache_ttl);

    info!(
        backend = %vault_config.backend,
        cache_ttl_secs = %cache_ttl.as_secs(),
        "vault initialized"
    );

    Ok(Arc::new(cached))
}

/// Build embedded (file-based) vault.
async fn build_embedded_vault(config: &VaultConfigSlice) -> Result<EmbeddedVault, ServerError> {
    // Resolve master key
    let master_key = config
        .master_key
        .clone()
        .unwrap_or_else(|| "env:REACTOR_VAULT_MASTER_KEY".to_string());

    let embedded_config = EmbeddedConfig {
        path: config.path.clone(),
        master_key,
    };

    EmbeddedVault::new(&embedded_config)
        .await
        .map_err(|e| ServerError::Boot(format!("failed to initialize embedded vault: {}", e)))
}

/// Build OpenBao/Vault client.
#[cfg(feature = "openbao")]
async fn build_openbao_vault(config: &VaultConfigSlice) -> Result<OpenBaoVault, ServerError> {
    let address = config
        .address
        .clone()
        .ok_or_else(|| ServerError::Config("vault.address is required for openbao backend".to_string()))?;

    let address_url = url::Url::parse(&address)
        .map_err(|e| ServerError::Config(format!("invalid vault address: {}", e)))?;

    let auth = match config.auth_method.as_str() {
        "token" => {
            let token = config
                .token
                .clone()
                .ok_or_else(|| ServerError::Config("vault.token is required for token auth".to_string()))?;
            OpenBaoAuth::Token { token }
        }
        "approle" => {
            let role_id = config
                .role_id
                .clone()
                .ok_or_else(|| ServerError::Config("vault.role_id is required for approle auth".to_string()))?;
            let secret_id_file = config
                .secret_id_file
                .clone()
                .ok_or_else(|| ServerError::Config("vault.secret_id_file is required for approle auth".to_string()))?;

            OpenBaoAuth::AppRole {
                role_id,
                secret_id_file,
                mount_path: config.approle_mount.clone(),
            }
        }
        other => {
            return Err(ServerError::Config(format!(
                "unknown vault auth method: {} (expected 'token' or 'approle')",
                other
            )));
        }
    };

    let openbao_config = OpenBaoConfig {
        address: address_url,
        namespace: config.namespace.clone(),
        kv_mount: config.kv_mount.clone(),
        transit_mount: config.transit_mount.clone(),
        auth,
        ca_cert: config.ca_cert.clone(),
    };

    OpenBaoVault::new(&openbao_config)
        .await
        .map_err(|e| ServerError::Boot(format!("failed to initialize openbao vault: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cached_vault_basic() {
        let mock = Arc::new(MockVault::new());
        let cached = CachedVault::new(mock.clone(), Duration::from_secs(60));

        let tenant = ProjectId::new();

        // Put and get
        let value = SecretValue::from_str("test-secret");
        cached.put_secret(&tenant, "test", value.clone()).await.unwrap();

        let retrieved = cached.get_secret(&tenant, "test").await.unwrap().unwrap();
        assert_eq!(retrieved.as_str(), Some("test-secret"));

        // Should be cached now (second get hits cache)
        let cached_value = cached.get_secret(&tenant, "test").await.unwrap().unwrap();
        assert_eq!(cached_value.as_str(), Some("test-secret"));
    }

    #[tokio::test]
    async fn test_cached_vault_invalidate_on_put() {
        let mock = Arc::new(MockVault::new());
        let cached = CachedVault::new(mock.clone(), Duration::from_secs(60));

        let tenant = ProjectId::new();

        // Put initial value
        let value1 = SecretValue::from_str("value1");
        cached.put_secret(&tenant, "test", value1).await.unwrap();

        // Get (populates cache)
        let _ = cached.get_secret(&tenant, "test").await.unwrap();

        // Put new value (should invalidate cache)
        let value2 = SecretValue::from_str("value2");
        cached.put_secret(&tenant, "test", value2).await.unwrap();

        // Get should return new value
        let retrieved = cached.get_secret(&tenant, "test").await.unwrap().unwrap();
        assert_eq!(retrieved.as_str(), Some("value2"));
    }

    #[tokio::test]
    async fn test_build_embedded_vault() {
        let dir = TempDir::new().unwrap();
        
        // Set up a test master key
        let master_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        
        let config = VaultConfigSlice {
            backend: "embedded".to_string(),
            path: dir.path().to_path_buf(),
            master_key: Some(master_key.to_string()),
            ..Default::default()
        };

        let vault = build_embedded_vault(&config).await.unwrap();

        // Test basic operation
        let tenant = ProjectId::new();
        let value = SecretValue::from_str("test");
        vault.put_secret(&tenant, "test", value).await.unwrap();

        let retrieved = vault.get_secret(&tenant, "test").await.unwrap().unwrap();
        assert_eq!(retrieved.as_str(), Some("test"));
    }
}
