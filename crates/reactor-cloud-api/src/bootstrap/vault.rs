//! Vault bootstrapping for tenant secrets.
//!
//! Creates transit encryption keys and KV secrets for each tenant.

use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

use reactor_core::{ProjectId, SecretValue, Vault, VaultError};

/// Error type for vault bootstrap operations.
#[derive(Debug, Error)]
pub enum VaultBootstrapError {
    #[error("transit key creation failed: {0}")]
    TransitKeyCreation(String),

    #[error("secret creation failed: {0}")]
    SecretCreation(String),

    #[error("JWT generation failed: {0}")]
    JwtGeneration(String),

    #[error("vault error: {0}")]
    Vault(#[from] VaultError),
}

/// Result of vault bootstrap.
#[derive(Debug)]
pub struct VaultBootstrapResult {
    /// Number of transit keys created.
    pub transit_keys_created: usize,
    /// Number of KV secrets created.
    pub secrets_created: usize,
}

/// Vault bootstrap operations.
pub struct VaultBootstrap;

impl VaultBootstrap {
    /// Transit key names for a tenant.
    const TRANSIT_KEYS: &'static [&'static str] = &[
        "auth/data",       // Encrypt auth tokens/data
        "storage/signing", // Sign storage URLs
        "functions/env",   // Encrypt function env vars
        "jobs/webhook",    // Sign webhook payloads
    ];

    /// KV secret names for a tenant.
    const KV_SECRETS: &'static [&'static str] = &[
        "keys/anon",        // Anonymous API key material
        "keys/service",     // Service API key material
        "keys/jwt-signing", // JWT signing key
    ];

    /// Bootstrap vault transit keys and secrets for a tenant.
    pub async fn bootstrap(
        vault: &Arc<dyn Vault>,
        project_id: &ProjectId,
    ) -> Result<VaultBootstrapResult, VaultBootstrapError> {
        info!(project_id = %project_id, "bootstrapping vault for tenant");

        let mut transit_keys_created = 0;
        let mut secrets_created = 0;

        // Create transit keys by encrypting a test value
        // This ensures the key is created in backends that support lazy key creation
        for key_name in Self::TRANSIT_KEYS {
            debug!(key = %key_name, "initializing transit key");
            match vault.encrypt(project_id, key_name, b"init").await {
                Ok(_) => {
                    transit_keys_created += 1;
                    debug!(key = %key_name, "transit key initialized");
                }
                Err(e) => {
                    warn!(key = %key_name, error = %e, "transit key initialization failed (may already exist)");
                }
            }
        }

        // Create KV secrets with random values
        for secret_name in Self::KV_SECRETS {
            debug!(secret = %secret_name, "creating KV secret");

            // Generate random 32-byte key material
            let key_material = Self::generate_random_bytes(32);
            let secret = SecretValue::new(key_material);

            match vault.put_secret(project_id, secret_name, secret).await {
                Ok(_) => {
                    secrets_created += 1;
                    debug!(secret = %secret_name, "KV secret created");
                }
                Err(e) => {
                    warn!(secret = %secret_name, error = %e, "KV secret creation failed (may already exist)");
                }
            }
        }

        info!(
            project_id = %project_id,
            transit_keys = transit_keys_created,
            secrets = secrets_created,
            "vault bootstrap complete"
        );

        Ok(VaultBootstrapResult {
            transit_keys_created,
            secrets_created,
        })
    }

    /// Generate an anonymous JWT for the tenant.
    pub async fn generate_anon_jwt(
        vault: &Arc<dyn Vault>,
        project_id: &ProjectId,
        project_ref: &str,
    ) -> Result<String, VaultBootstrapError> {
        Self::generate_jwt(vault, project_id, project_ref, "anon").await
    }

    /// Generate a service JWT for the tenant.
    pub async fn generate_service_jwt(
        vault: &Arc<dyn Vault>,
        project_id: &ProjectId,
        project_ref: &str,
    ) -> Result<String, VaultBootstrapError> {
        Self::generate_jwt(vault, project_id, project_ref, "service").await
    }

    /// Generate a JWT with the given role.
    async fn generate_jwt(
        vault: &Arc<dyn Vault>,
        project_id: &ProjectId,
        project_ref: &str,
        role: &str,
    ) -> Result<String, VaultBootstrapError> {
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
        use serde::{Deserialize, Serialize};

        // Get the JWT signing key from vault
        let signing_key = vault
            .get_secret(project_id, "keys/jwt-signing")
            .await
            .map_err(VaultBootstrapError::Vault)?
            .ok_or_else(|| {
                VaultBootstrapError::JwtGeneration("jwt-signing key not found".to_string())
            })?;

        // JWT claims
        #[derive(Serialize, Deserialize)]
        struct Claims {
            role: String,
            project_id: String,
            project_ref: String,
            iat: i64,
        }

        let claims = Claims {
            role: role.to_string(),
            project_id: project_id.to_string(),
            project_ref: project_ref.to_string(),
            iat: chrono::Utc::now().timestamp(),
        };

        // Create JWT header with kid for key rotation support
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(format!("{}:{}", project_ref, "v1"));

        // Encode JWT
        let key = EncodingKey::from_secret(signing_key.as_bytes());
        let token = encode(&header, &claims, &key)
            .map_err(|e| VaultBootstrapError::JwtGeneration(e.to_string()))?;

        Ok(token)
    }

    /// Cleanup vault resources for a tenant.
    pub async fn cleanup(
        vault: &Arc<dyn Vault>,
        project_id: &ProjectId,
    ) -> Result<(), VaultBootstrapError> {
        info!(project_id = %project_id, "cleaning up vault for tenant");

        // Delete KV secrets (transit keys don't have a delete API in most vaults)
        for secret_name in Self::KV_SECRETS {
            debug!(secret = %secret_name, "deleting KV secret");
            if let Err(e) = vault.delete_secret(project_id, secret_name).await {
                warn!(secret = %secret_name, error = %e, "failed to delete KV secret");
            }
        }

        info!(project_id = %project_id, "vault cleanup complete");
        Ok(())
    }

    /// Generate random bytes using a secure RNG.
    fn generate_random_bytes(len: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut bytes = vec![0u8; len];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_bytes() {
        let bytes1 = VaultBootstrap::generate_random_bytes(32);
        let bytes2 = VaultBootstrap::generate_random_bytes(32);

        assert_eq!(bytes1.len(), 32);
        assert_eq!(bytes2.len(), 32);
        assert_ne!(bytes1, bytes2); // Very unlikely to be equal
    }
}
