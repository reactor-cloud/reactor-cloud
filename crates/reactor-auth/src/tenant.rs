//! Tenant-aware URL and OAuth configuration helpers.
//!
//! In multi-tenant mode, callback URLs are derived from the TenantCtx at request
//! time, and OAuth provider configurations are loaded from vault per-tenant.

use reactor_core::TenantCtx;
use serde::{Deserialize, Serialize};
use url::Url;

/// Tenant URL builder for deriving auth-related URLs.
///
/// In single-tenant mode, this uses a fixed base URL from config.
/// In multi-tenant mode, this derives URLs from the tenant's project_ref.
#[derive(Debug, Clone)]
pub struct TenantUrlBuilder {
    /// Base domain for tenant subdomains (e.g., "reactor.cloud").
    base_domain: String,
    /// URL scheme (typically "https").
    scheme: String,
    /// Fallback URL for single-tenant mode.
    fallback_url: Option<Url>,
}

impl TenantUrlBuilder {
    /// Create a new tenant URL builder for multi-tenant mode.
    pub fn new(base_domain: impl Into<String>, scheme: impl Into<String>) -> Self {
        Self {
            base_domain: base_domain.into(),
            scheme: scheme.into(),
            fallback_url: None,
        }
    }

    /// Create a URL builder with a fallback for single-tenant mode.
    pub fn with_fallback(mut self, fallback: Url) -> Self {
        self.fallback_url = Some(fallback);
        self
    }

    /// Create a URL builder for single-tenant mode only.
    pub fn single_tenant(fallback: Url) -> Self {
        Self {
            base_domain: String::new(),
            scheme: String::new(),
            fallback_url: Some(fallback),
        }
    }

    /// Build the base URL for a tenant.
    ///
    /// Returns `https://{project_ref}.{base_domain}` in multi-tenant mode,
    /// or the fallback URL in single-tenant mode.
    pub fn base_url(&self, tenant: &TenantCtx) -> Url {
        if self.base_domain.is_empty() {
            return self
                .fallback_url
                .clone()
                .expect("fallback URL required in single-tenant mode");
        }

        let host = format!("{}.{}", tenant.project_ref(), self.base_domain);
        format!("{}://{}", self.scheme, host)
            .parse()
            .expect("invalid URL constructed")
    }

    /// Build the OAuth callback URL for a provider.
    ///
    /// Returns `{base_url}/auth/callback/{provider}`.
    pub fn oauth_callback_url(&self, tenant: &TenantCtx, provider: &str) -> Url {
        let mut url = self.base_url(tenant);
        url.set_path(&format!("/auth/callback/{}", provider));
        url
    }

    /// Build the email verification URL.
    ///
    /// Returns `{base_url}/auth/verify?token={token}`.
    pub fn verify_email_url(&self, tenant: &TenantCtx, token: &str) -> Url {
        let mut url = self.base_url(tenant);
        url.set_path("/auth/verify");
        url.set_query(Some(&format!("token={}", token)));
        url
    }

    /// Build the password reset URL.
    ///
    /// Returns `{base_url}/auth/reset-password?token={token}`.
    pub fn reset_password_url(&self, tenant: &TenantCtx, token: &str) -> Url {
        let mut url = self.base_url(tenant);
        url.set_path("/auth/reset-password");
        url.set_query(Some(&format!("token={}", token)));
        url
    }

    /// Build the magic link URL.
    ///
    /// Returns `{base_url}/auth/magic?token={token}`.
    pub fn magic_link_url(&self, tenant: &TenantCtx, token: &str) -> Url {
        let mut url = self.base_url(tenant);
        url.set_path("/auth/magic");
        url.set_query(Some(&format!("token={}", token)));
        url
    }
}

/// OAuth provider configuration.
///
/// Stored in vault at `tenant/<project_id>/oauth/<provider>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    /// OAuth client ID.
    pub client_id: String,
    /// OAuth client secret.
    pub client_secret: String,
    /// Authorization endpoint URL.
    pub auth_url: String,
    /// Token endpoint URL.
    pub token_url: String,
    /// Scopes to request.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Whether this provider is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl OAuthProviderConfig {
    /// Create a Google OAuth config.
    pub fn google(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            enabled: true,
        }
    }

    /// Create a GitHub OAuth config.
    pub fn github(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            scopes: vec!["user:email".to_string()],
            enabled: true,
        }
    }

    /// Vault path for storing this provider config.
    ///
    /// Returns `oauth/<provider>` which is stored under `tenant/<project_id>/`.
    pub fn vault_path(provider: &str) -> String {
        format!("oauth/{}", provider)
    }
}

/// Helper for loading OAuth configs from vault.
pub struct OAuthConfigLoader<'a> {
    vault: &'a dyn reactor_core::primitives::vault::Vault,
    project_id: &'a reactor_core::ProjectId,
}

impl<'a> OAuthConfigLoader<'a> {
    /// Create a new OAuth config loader.
    pub fn new(
        vault: &'a dyn reactor_core::primitives::vault::Vault,
        project_id: &'a reactor_core::ProjectId,
    ) -> Self {
        Self { vault, project_id }
    }

    /// Load a provider config from vault.
    ///
    /// Returns `None` if the provider is not configured.
    pub async fn load_provider(
        &self,
        provider: &str,
    ) -> Result<Option<OAuthProviderConfig>, OAuthConfigError> {
        let path = OAuthProviderConfig::vault_path(provider);

        let secret = self
            .vault
            .get_secret(self.project_id, &path)
            .await
            .map_err(|e| OAuthConfigError::Vault(e.to_string()))?;

        match secret {
            Some(secret) => {
                let config: OAuthProviderConfig =
                    serde_json::from_slice(&secret.data).map_err(|e| {
                        OAuthConfigError::InvalidConfig(format!(
                            "failed to parse {} config: {}",
                            provider, e
                        ))
                    })?;

                if config.enabled {
                    Ok(Some(config))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Save a provider config to vault.
    pub async fn save_provider(
        &self,
        provider: &str,
        config: &OAuthProviderConfig,
    ) -> Result<(), OAuthConfigError> {
        let path = OAuthProviderConfig::vault_path(provider);

        let data =
            serde_json::to_vec(config).map_err(|e| OAuthConfigError::InvalidConfig(e.to_string()))?;

        self.vault
            .put_secret(
                self.project_id,
                &path,
                reactor_core::primitives::vault::SecretValue::new(data),
            )
            .await
            .map_err(|e| OAuthConfigError::Vault(e.to_string()))?;

        Ok(())
    }

    /// List configured providers.
    pub async fn list_providers(&self) -> Result<Vec<String>, OAuthConfigError> {
        let secrets = self
            .vault
            .list_secrets(self.project_id)
            .await
            .map_err(|e| OAuthConfigError::Vault(e.to_string()))?;

        Ok(secrets
            .into_iter()
            .filter(|s| s.name.starts_with("oauth/"))
            .map(|s| s.name.strip_prefix("oauth/").unwrap_or(&s.name).to_string())
            .collect())
    }
}

/// Errors when loading OAuth configuration.
#[derive(Debug, thiserror::Error)]
pub enum OAuthConfigError {
    /// Vault operation failed.
    #[error("vault error: {0}")]
    Vault(String),
    /// Invalid configuration format.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_url_builder_multi_tenant() {
        let builder = TenantUrlBuilder::new("reactor.cloud", "https");

        // Create a mock tenant context
        let project_id = reactor_core::ProjectId::nil();
        let project_ref = project_id.to_ref();
        let tenant = TenantCtx::new(
            project_id,
            project_ref,
            "test-project",
            reactor_core::TenantEnv::Production,
        );

        let base_url = builder.base_url(&tenant);
        assert!(base_url.host_str().unwrap().ends_with(".reactor.cloud"));
        assert_eq!(base_url.scheme(), "https");
    }

    #[test]
    fn test_oauth_callback_url() {
        let builder = TenantUrlBuilder::new("reactor.cloud", "https");

        let project_id = reactor_core::ProjectId::nil();
        let project_ref = project_id.to_ref();
        let tenant = TenantCtx::new(
            project_id,
            project_ref,
            "test-project",
            reactor_core::TenantEnv::Production,
        );

        let callback_url = builder.oauth_callback_url(&tenant, "google");
        assert!(callback_url.path().contains("/auth/callback/google"));
    }

    #[test]
    fn test_single_tenant_mode() {
        let fallback = "https://app.example.com".parse().unwrap();
        let builder = TenantUrlBuilder::single_tenant(fallback);

        let project_id = reactor_core::ProjectId::nil();
        let project_ref = project_id.to_ref();
        let tenant = TenantCtx::new(
            project_id,
            project_ref,
            "test-project",
            reactor_core::TenantEnv::Production,
        );

        let base_url = builder.base_url(&tenant);
        assert_eq!(base_url.host_str().unwrap(), "app.example.com");
    }

    #[test]
    fn test_oauth_provider_configs() {
        let google = OAuthProviderConfig::google("client-id", "secret");
        assert!(google.enabled);
        assert!(google.scopes.contains(&"openid".to_string()));

        let github = OAuthProviderConfig::github("client-id", "secret");
        assert!(github.scopes.contains(&"user:email".to_string()));
    }

    #[test]
    fn test_vault_path() {
        assert_eq!(OAuthProviderConfig::vault_path("google"), "oauth/google");
        assert_eq!(OAuthProviderConfig::vault_path("github"), "oauth/github");
    }
}
