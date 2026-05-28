//! OpenBao/HashiCorp Vault client adapter.
//!
//! This adapter implements the `Vault` trait using an OpenBao (or HashiCorp Vault)
//! backend. It supports:
//!
//! - Transit secrets engine for encryption/decryption
//! - KV v2 secrets engine for secret storage
//! - Token and AppRole authentication
//!
//! For production deployments (G3), this is the recommended backend.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use vaultrs::api::transit::KeyType;
use vaultrs::client::{Client, VaultClient, VaultClientSettingsBuilder};
use vaultrs::error::ClientError;
use vaultrs::kv2;
use vaultrs::transit;

use reactor_core::primitives::vault::{
    Ciphertext, SecretMetadata, SecretValue, Vault, VaultError,
};
use reactor_core::ProjectId;

use crate::{OpenBaoAuth, OpenBaoConfig};

/// OpenBao/Vault client adapter.
pub struct OpenBaoVault {
    /// The Vault client.
    client: Arc<RwLock<VaultClient>>,
    /// KV v2 mount path.
    kv_mount: String,
    /// Transit mount path.
    transit_mount: String,
    /// Configuration for reconnection.
    config: OpenBaoConfig,
}

impl OpenBaoVault {
    /// Create a new OpenBao vault client.
    ///
    /// # Arguments
    ///
    /// * `config` - OpenBao configuration
    ///
    /// # Errors
    ///
    /// Returns an error if connection or authentication fails.
    pub async fn new(config: &OpenBaoConfig) -> Result<Self, VaultError> {
        let client = Self::create_client(config).await?;

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            kv_mount: config.kv_mount.clone(),
            transit_mount: config.transit_mount.clone(),
            config: config.clone(),
        })
    }

    /// Create an authenticated Vault client.
    async fn create_client(config: &OpenBaoConfig) -> Result<VaultClient, VaultError> {
        let mut settings = VaultClientSettingsBuilder::default();
        settings.address(config.address.as_str());

        if let Some(ref ns) = config.namespace {
            settings.namespace(Some(ns.clone()));
        }

        // Handle custom CA cert
        if let Some(ref ca_cert_path) = config.ca_cert {
            let ca_cert = fs::read_to_string(ca_cert_path)
                .await
                .map_err(|e| VaultError::Configuration(format!("Failed to read CA cert: {}", e)))?;

            settings.ca_certs(vec![ca_cert]);
        }

        let settings = settings
            .build()
            .map_err(|e| VaultError::Configuration(format!("Invalid client settings: {}", e)))?;

        let mut client = VaultClient::new(settings)
            .map_err(|e| VaultError::Connection(format!("Failed to create client: {}", e)))?;

        // Authenticate
        match &config.auth {
            OpenBaoAuth::Token { token } => {
                client.set_token(token);
            }
            OpenBaoAuth::AppRole {
                role_id,
                secret_id_file,
                mount_path,
            } => {
                let secret_id = fs::read_to_string(secret_id_file).await.map_err(|e| {
                    VaultError::Configuration(format!("Failed to read secret ID file: {}", e))
                })?;

                let auth_info = vaultrs::auth::approle::login(
                    &client,
                    mount_path,
                    role_id,
                    secret_id.trim(),
                )
                .await
                .map_err(|e| VaultError::AuthenticationFailed(format!("AppRole login failed: {}", e)))?;

                client.set_token(&auth_info.client_token);
            }
        }

        // Verify connection
        let health = vaultrs::sys::health(&client)
            .await
            .map_err(|e| VaultError::Connection(format!("Health check failed: {}", e)))?;

        if health.sealed {
            return Err(VaultError::Sealed);
        }

        debug!(
            address = %config.address,
            version = %health.version,
            "Connected to OpenBao/Vault"
        );

        Ok(client)
    }

    /// Get the tenant-specific KV path prefix.
    fn tenant_kv_path(tenant: &ProjectId) -> String {
        format!("tenants/{}", tenant)
    }

    /// Get the full KV path for a secret.
    fn secret_path(tenant: &ProjectId, name: &str) -> String {
        format!("{}/{}", Self::tenant_kv_path(tenant), name)
    }

    /// Get the tenant-specific transit key name.
    fn transit_key_name(tenant: &ProjectId, key: &str) -> String {
        format!("tenant-{}-{}", tenant, key)
    }

    /// Ensure a transit key exists.
    async fn ensure_transit_key(
        &self,
        tenant: &ProjectId,
        key: &str,
    ) -> Result<(), VaultError> {
        let key_name = Self::transit_key_name(tenant, key);
        let client = self.client.read().await;

        // Try to read the key first
        match transit::key::read(&*client, &self.transit_mount, &key_name).await {
            Ok(_) => return Ok(()),
            Err(ClientError::APIError { code: 404, .. }) => {
                // Key doesn't exist, create it
            }
            Err(e) => {
                return Err(VaultError::Connection(format!(
                    "Failed to check transit key: {}",
                    e
                )));
            }
        }

        // Create the key
        transit::key::create(
            &*client,
            &self.transit_mount,
            &key_name,
            Some(
                vaultrs::api::transit::requests::CreateKeyRequest::builder()
                    .key_type(KeyType::Aes256Gcm96)
                    .exportable(false)
                    .allow_plaintext_backup(false),
            ),
        )
        .await
        .map_err(|e| VaultError::Connection(format!("Failed to create transit key: {}", e)))?;

        debug!(
            tenant = %tenant,
            key = %key,
            "Created transit key"
        );

        Ok(())
    }
}

#[async_trait]
impl Vault for OpenBaoVault {
    async fn encrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        plaintext: &[u8],
    ) -> Result<Ciphertext, VaultError> {
        self.ensure_transit_key(tenant, key).await?;

        let key_name = Self::transit_key_name(tenant, key);
        let client = self.client.read().await;

        // Base64 encode plaintext for the API
        use base64::Engine;
        let plaintext_b64 = base64::engine::general_purpose::STANDARD.encode(plaintext);

        let result = transit::data::encrypt(
            &*client,
            &self.transit_mount,
            &key_name,
            &plaintext_b64,
            None,
        )
        .await
        .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;

        // Parse the ciphertext format "vault:v{version}:{base64}"
        let parts: Vec<&str> = result.ciphertext.splitn(3, ':').collect();
        let key_version = if parts.len() >= 2 && parts[1].starts_with('v') {
            parts[1][1..].parse().ok()
        } else {
            None
        };

        Ok(Ciphertext {
            data: result.ciphertext,
            key_version,
        })
    }

    async fn decrypt(
        &self,
        tenant: &ProjectId,
        key: &str,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, VaultError> {
        let key_name = Self::transit_key_name(tenant, key);
        let client = self.client.read().await;

        let result = transit::data::decrypt(
            &*client,
            &self.transit_mount,
            &key_name,
            &ciphertext.data,
            None,
        )
        .await
        .map_err(|e| VaultError::DecryptionFailed(e.to_string()))?;

        // The result is base64-encoded plaintext
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&result.plaintext)
            .map_err(|e| VaultError::DecryptionFailed(format!("Invalid plaintext encoding: {}", e)))
    }

    async fn rotate_key(&self, tenant: &ProjectId, key: &str) -> Result<u32, VaultError> {
        let key_name = Self::transit_key_name(tenant, key);
        let client = self.client.read().await;

        transit::key::rotate(&*client, &self.transit_mount, &key_name)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to rotate key: {}", e)))?;

        // Get the new version by reading the key info
        let key_info = transit::key::read(&*client, &self.transit_mount, &key_name)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to read key info: {}", e)))?;

        // Use min_available_version and min_decryption_version to infer latest
        // After a rotation, the latest version is typically one higher than before
        // The safest approach is to use min_decryption_version as a lower bound
        let latest_version = key_info.min_decryption_version.max(1) as u32;

        debug!(
            tenant = %tenant,
            key = %key,
            version = %latest_version,
            "Rotated transit key"
        );

        Ok(latest_version)
    }

    async fn get_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
    ) -> Result<Option<SecretValue>, VaultError> {
        let path = Self::secret_path(tenant, name);
        let client = self.client.read().await;

        let result: Result<HashMap<String, serde_json::Value>, _> =
            kv2::read(&*client, &self.kv_mount, &path).await;

        match result {
            Ok(data) => {
                // Get the data field
                use base64::Engine;
                let secret_data = data
                    .get("value")
                    .and_then(|v| v.as_str())
                    .map(|s| base64::engine::general_purpose::STANDARD.decode(s))
                    .transpose()
                    .map_err(|e| VaultError::DecryptionFailed(format!("Invalid secret encoding: {}", e)))?
                    .unwrap_or_default();

                // Get metadata separately
                let metadata = kv2::read_metadata(&*client, &self.kv_mount, &path)
                    .await
                    .map_err(|e| VaultError::Connection(format!("Failed to read metadata: {}", e)))?;

                let now = Utc::now();
                Ok(Some(SecretValue {
                    data: secret_data,
                    version: metadata.current_version as u64,
                    created_at: parse_vault_time(&metadata.created_time).unwrap_or(now),
                    updated_at: parse_vault_time(&metadata.updated_time).unwrap_or(now),
                }))
            }
            Err(ClientError::APIError { code: 404, .. }) => Ok(None),
            Err(e) => Err(VaultError::Connection(format!("Failed to read secret: {}", e))),
        }
    }

    async fn put_secret(
        &self,
        tenant: &ProjectId,
        name: &str,
        value: SecretValue,
    ) -> Result<(), VaultError> {
        let path = Self::secret_path(tenant, name);
        let client = self.client.read().await;

        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&value.data);
        let mut data = HashMap::new();
        data.insert("value".to_string(), encoded);

        kv2::set(&*client, &self.kv_mount, &path, &data)
            .await
            .map_err(|e| VaultError::Connection(format!("Failed to write secret: {}", e)))?;

        debug!(
            tenant = %tenant,
            secret = %name,
            "Stored secret"
        );

        Ok(())
    }

    async fn list_secrets(&self, tenant: &ProjectId) -> Result<Vec<SecretMetadata>, VaultError> {
        let path = Self::tenant_kv_path(tenant);
        let client = self.client.read().await;

        let keys = match kv2::list(&*client, &self.kv_mount, &path).await {
            Ok(keys) => keys,
            Err(ClientError::APIError { code: 404, .. }) => return Ok(vec![]),
            Err(e) => return Err(VaultError::Connection(format!("Failed to list secrets: {}", e))),
        };

        let mut secrets = Vec::new();
        let now = Utc::now();
        
        for key in keys {
            // Get metadata for each secret
            let secret_path = format!("{}/{}", path, key);
            if let Ok(metadata) = kv2::read_metadata(&*client, &self.kv_mount, &secret_path).await {
                secrets.push(SecretMetadata {
                    name: key,
                    version: metadata.current_version as u64,
                    created_at: parse_vault_time(&metadata.created_time).unwrap_or(now),
                    updated_at: parse_vault_time(&metadata.updated_time).unwrap_or(now),
                });
            }
        }
        
        Ok(secrets)
    }

    async fn delete_secret(&self, tenant: &ProjectId, name: &str) -> Result<(), VaultError> {
        let path = Self::secret_path(tenant, name);
        let client = self.client.read().await;

        kv2::delete_metadata(&*client, &self.kv_mount, &path)
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
        let client = self.client.read().await;
        match vaultrs::sys::health(&*client).await {
            Ok(health) => !health.sealed,
            Err(e) => {
                warn!(error = %e, "Vault health check failed");
                false
            }
        }
    }

    async fn is_sealed(&self) -> bool {
        let client = self.client.read().await;
        match vaultrs::sys::health(&*client).await {
            Ok(health) => health.sealed,
            Err(_) => true, // Assume sealed if we can't connect
        }
    }
}

/// Parse Vault timestamp string to DateTime.
fn parse_vault_time(s: &str) -> Option<DateTime<Utc>> {
    // Vault returns ISO 8601 timestamps like "2024-01-15T10:30:00.123456789Z"
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

impl OpenBaoVault {
    /// Renew the authentication token.
    ///
    /// Call this periodically for long-running services to prevent token expiration.
    pub async fn renew_token(&self) -> Result<(), VaultError> {
        let client = self.client.read().await;

        vaultrs::token::renew_self(&*client, None)
            .await
            .map_err(|e| VaultError::AuthenticationFailed(format!("Token renewal failed: {}", e)))?;

        debug!("Renewed Vault token");
        Ok(())
    }

    /// Re-authenticate using the configured auth method.
    ///
    /// Call this if the token has expired or become invalid.
    pub async fn reauthenticate(&self) -> Result<(), VaultError> {
        let new_client = Self::create_client(&self.config).await?;
        let mut client = self.client.write().await;
        *client = new_client;
        debug!("Re-authenticated with Vault");
        Ok(())
    }
}
