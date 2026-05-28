//! Auth bundle construction for the unified server.
//!
//! Builds the `AuthBundle` which contains the `AuthState` and an
//! `InProcessAuthClient` that other capabilities use for authentication.

use crate::config::AuthConfigSlice;
use crate::error::ServerError;
use reactor_auth::config::SmtpConfig as AuthSmtpConfig;
use reactor_auth::{AuthConfig, AuthState, InProcessAuthClient};
use reactor_core::auth::AuthClient;
use reactor_core::primitives::vault::Vault;
use reactor_core::ProjectId;
use sqlx::PgPool;
use std::sync::Arc;

/// Bundle containing auth state and in-process client.
pub struct AuthBundle {
    /// The auth state (for mounting the router).
    pub state: AuthState,

    /// The in-process auth client (for other capabilities).
    pub client: Arc<dyn AuthClient>,
}

impl AuthBundle {
    /// Build the auth bundle from pool, config, and vault.
    ///
    /// This:
    /// 1. Constructs AuthState from the config slice
    /// 2. Resolves secrets from vault if configured
    /// 3. Ensures an active signing key exists
    /// 4. Wraps the service in InProcessAuthClient
    pub async fn build(
        pool: &PgPool,
        config: &AuthConfigSlice,
        vault: &dyn Vault,
        tenant: &ProjectId,
    ) -> Result<Self, ServerError> {
        // Convert config slice to full AuthConfig (with vault resolution)
        let auth_config = build_auth_config(config, vault, tenant).await?;

        // Build AuthState
        let state = AuthState::from_pool(pool.clone(), auth_config)
            .map_err(|e| ServerError::Capability {
                capability: "auth".to_string(),
                message: format!("failed to build AuthState: {:?}", e),
            })?;

        // Ensure we have an active signing key
        state
            .keyring
            .ensure_active_key()
            .await
            .map_err(|e| ServerError::Capability {
                capability: "auth".to_string(),
                message: format!("failed to ensure active signing key: {:?}", e),
            })?;

        tracing::info!("auth bundle ready with active signing key");

        // Create the in-process client
        let client: Arc<dyn AuthClient> =
            Arc::new(InProcessAuthClient::new(state.service.clone()));

        Ok(Self { state, client })
    }
}

/// Resolve a secret value, potentially from vault.
///
/// Supports:
/// - Direct value: `"my-secret-key"` - uses the value directly
/// - Vault reference: `"vault:secret-name"` - fetches from vault KV
async fn resolve_secret(
    config_value: &str,
    vault: &dyn Vault,
    tenant: &ProjectId,
    description: &str,
) -> Result<String, ServerError> {
    if config_value.starts_with("vault:") {
        let vault_key = &config_value[6..]; // Strip "vault:" prefix
        let secret = vault
            .get_secret(tenant, vault_key)
            .await
            .map_err(|e| ServerError::Boot(format!("failed to get {} from vault: {}", description, e)))?
            .ok_or_else(|| ServerError::Config(format!(
                "{} '{}' not found in vault", description, vault_key
            )))?;

        String::from_utf8(secret.data)
            .map_err(|_| ServerError::Config(format!("{} is not valid UTF-8", description)))
    } else {
        Ok(config_value.to_string())
    }
}

/// Convert the server config slice to a full AuthConfig.
async fn build_auth_config(
    slice: &AuthConfigSlice,
    vault: &dyn Vault,
    tenant: &ProjectId,
) -> Result<AuthConfig, ServerError> {
    // Convert SMTP config if present (with optional vault resolution for password)
    let smtp = if let Some(s) = slice.smtp.as_ref() {
        // Resolve SMTP password if it's a vault reference
        let password = if let Some(ref pwd) = s.password {
            if pwd.starts_with("vault:") {
                Some(resolve_secret(pwd, vault, tenant, "SMTP password").await?)
            } else {
                Some(pwd.clone())
            }
        } else {
            None
        };

        Some(AuthSmtpConfig {
            host: s.host.clone(),
            port: s.port,
            user: s.user.clone(),
            password,
            from: s.from.clone(),
            tls: s.tls.clone(),
        })
    } else {
        None
    };

    // Parse public_url as Url
    let public_url: url::Url = slice
        .public_url
        .parse()
        .map_err(|e| ServerError::Config(format!("invalid auth.public_url: {}", e)))?;

    // Resolve data_key (column encryption key) - supports vault: refs
    let data_key = resolve_secret(&slice.data_key, vault, tenant, "auth data key").await?;

    Ok(AuthConfig {
        data_key,
        jwt_issuer: slice.jwt_issuer.clone(),
        jwt_audience: slice.jwt_audience.clone(),
        access_ttl_secs: slice.access_ttl_secs,
        refresh_ttl_secs: slice.refresh_ttl_secs,
        public_url,
        smtp,
        internal_secret: None, // Not needed for unified server
        // Fields that don't exist in slice (not needed for unified server)
        database_url: String::new(), // We use shared pool
        bind: "127.0.0.1:8001".parse().unwrap(),
        log: "info".to_string(),
        metrics: false,
    })
}
