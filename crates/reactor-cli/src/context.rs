//! Context management (~/.reactor/config.toml).
//!
//! Contexts represent connections to Reactor servers. Each context has:
//! - An endpoint URL
//! - Optional organization
//! - Authentication configuration (token env, keychain, file, or none)

use crate::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use url::Url;

/// Token storage filename.
pub const TOKENS_FILENAME: &str = "tokens.toml";

/// Global configuration filename.
pub const CONFIG_FILENAME: &str = "config.toml";

/// Global configuration directory name.
pub const CONFIG_DIR: &str = ".reactor";

/// Global configuration for the CLI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default context name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// Active context name (alias for default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_context: Option<String>,

    /// Named contexts.
    #[serde(default)]
    pub contexts: HashMap<String, ContextConfig>,
}

/// Configuration for a single context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Server endpoint URL.
    pub endpoint: String,

    /// Organization slug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,

    /// Authentication configuration.
    #[serde(default)]
    pub auth: AuthConfig,
}

/// Token storage backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TokenStorage {
    /// Use OS keychain (may prompt on macOS with unsigned binaries).
    #[default]
    Keychain,
    /// Use file-based storage (~/.reactor/tokens.toml).
    File,
}

/// Authentication configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuthConfig {
    /// No authentication.
    #[default]
    None,

    /// Token from environment variable.
    TokenEnv {
        /// Environment variable name.
        env: String,
    },

    /// Token stored in OS keychain.
    Keychain {
        /// Keychain service name.
        #[serde(default = "default_keychain_service")]
        service: String,
        /// Keychain account name.
        account: String,
    },

    /// Session-based authentication (PKCE login flow).
    ///
    /// Access and refresh tokens are stored with automatic refresh when
    /// the access token expires. Storage backend is configurable.
    Session {
        /// Storage backend (keychain or file).
        #[serde(default)]
        storage: TokenStorage,
        /// Keychain service name (used when storage is Keychain).
        #[serde(default = "default_keychain_service")]
        service: String,
        /// Account name for access token.
        access_account: String,
        /// Account name for refresh token.
        refresh_account: String,
        /// Access token expiration time (Unix timestamp).
        expires_at: i64,
        /// Granted scopes.
        #[serde(default)]
        scopes: Vec<String>,
    },
}

fn default_keychain_service() -> String {
    "reactor".to_string()
}

/// File-based token storage (~/.reactor/tokens.toml).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenFile {
    /// Tokens indexed by account name.
    #[serde(default)]
    pub tokens: HashMap<String, String>,
}

impl TokenFile {
    /// Get the token file path.
    pub fn path() -> CliResult<PathBuf> {
        Ok(GlobalConfig::config_dir()?.join(TOKENS_FILENAME))
    }

    /// Load tokens from disk.
    pub fn load() -> CliResult<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        toml_edit::de::from_str(&content)
            .map_err(|e| CliError::Config(format!("invalid tokens file: {}", e)))
    }

    /// Save tokens to disk with restricted permissions.
    pub fn save(&self) -> CliResult<()> {
        let dir = GlobalConfig::config_dir()?;
        std::fs::create_dir_all(&dir)?;

        let path = Self::path()?;
        let content = toml_edit::ser::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("failed to serialize tokens: {}", e)))?;
        
        // Write file
        std::fs::write(&path, &content)?;
        
        // Set restrictive permissions (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&path, perms)?;
        }
        
        Ok(())
    }

    /// Get a token by account name.
    pub fn get(&self, account: &str) -> Option<&str> {
        self.tokens.get(account).map(|s| s.as_str())
    }

    /// Set a token for an account.
    pub fn set(&mut self, account: impl Into<String>, token: impl Into<String>) {
        self.tokens.insert(account.into(), token.into());
    }

    /// Remove a token by account name.
    pub fn remove(&mut self, account: &str) -> Option<String> {
        self.tokens.remove(account)
    }
}

/// A resolved context ready for use.
#[derive(Debug, Clone)]
pub struct ResolvedContext {
    /// Context name.
    pub name: String,
    /// Server endpoint URL.
    pub endpoint: Url,
    /// Organization slug.
    pub org: Option<String>,
    /// Resolved authentication token (if any).
    pub token: Option<String>,
}

impl GlobalConfig {
    /// Get the config directory path (~/.reactor).
    pub fn config_dir() -> CliResult<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| CliError::Config("could not determine home directory".into()))?;
        Ok(home.join(CONFIG_DIR))
    }

    /// Get the config file path (~/.reactor/config.toml).
    pub fn config_path() -> CliResult<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILENAME))
    }

    /// Load the global configuration from disk.
    pub fn load() -> CliResult<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        toml_edit::de::from_str(&content)
            .map_err(|e| CliError::Config(format!("invalid config: {}", e)))
    }

    /// Save the global configuration to disk.
    pub fn save(&self) -> CliResult<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;

        let path = Self::config_path()?;
        let content = toml_edit::ser::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("failed to serialize config: {}", e)))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get a context by name.
    pub fn get_context(&self, name: &str) -> Option<&ContextConfig> {
        self.contexts.get(name)
    }

    /// Add or update a context.
    pub fn set_context(&mut self, name: String, config: ContextConfig) {
        self.contexts.insert(name, config);
    }

    /// Remove a context.
    pub fn remove_context(&mut self, name: &str) -> Option<ContextConfig> {
        self.contexts.remove(name)
    }

    /// Set the default context.
    pub fn set_default(&mut self, name: Option<String>) {
        self.default = name.clone();
        self.active_context = name;
    }

    /// Get the default context name.
    pub fn get_default(&self) -> Option<&str> {
        self.active_context.as_deref().or(self.default.as_deref())
    }
}

/// Save the context configuration (no-op for now, config is managed via GlobalConfig).
pub fn save_context_config(_config: &GlobalConfig, _context_name: &str) -> CliResult<()> {
    // Context config is part of the global config, which is saved via GlobalConfig::save
    Ok(())
}

impl ContextConfig {
    /// Create a new context config.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            org: None,
            auth: AuthConfig::None,
        }
    }

    /// Set the organization.
    pub fn with_org(mut self, org: impl Into<String>) -> Self {
        self.org = Some(org.into());
        self
    }

    /// Set auth to use an environment variable.
    pub fn with_token_env(mut self, env: impl Into<String>) -> Self {
        self.auth = AuthConfig::TokenEnv { env: env.into() };
        self
    }

    /// Set auth to use the OS keychain.
    pub fn with_keychain(mut self, account: impl Into<String>) -> Self {
        self.auth = AuthConfig::Keychain {
            service: default_keychain_service(),
            account: account.into(),
        };
        self
    }
}

/// Resolve a context by name, applying token precedence.
///
/// Token precedence (highest to lowest):
/// 1. `--token` flag (passed as `cli_token`)
/// 2. `REACTOR_TOKEN` environment variable
/// 3. Context-specific env var (from config)
/// 4. OS keychain (from config)
/// 5. None
pub fn resolve_context(
    config: &GlobalConfig,
    context_name: Option<&str>,
    project_default: Option<&str>,
    cli_token: Option<&str>,
) -> CliResult<ResolvedContext> {
    // Determine context name
    let name = context_name
        .or(project_default)
        .or(config.get_default())
        .ok_or_else(|| {
            CliError::ContextNotFound("no context specified and no default set".into())
        })?;

    // Get context config
    let ctx = config.get_context(name).ok_or_else(|| {
        CliError::ContextNotFound(name.to_string())
    })?;

    // Parse endpoint
    let endpoint = Url::parse(&ctx.endpoint)?;

    // Resolve token with precedence
    let token = resolve_token(cli_token, &ctx.auth)?;

    Ok(ResolvedContext {
        name: name.to_string(),
        endpoint,
        org: ctx.org.clone(),
        token,
    })
}

/// Resolve a token using the precedence rules.
fn resolve_token(cli_token: Option<&str>, auth: &AuthConfig) -> CliResult<Option<String>> {
    // 1. CLI token flag
    if let Some(t) = cli_token {
        return Ok(Some(t.to_string()));
    }

    // 2. REACTOR_TOKEN env var
    if let Ok(t) = std::env::var("REACTOR_TOKEN") {
        if !t.is_empty() {
            return Ok(Some(t));
        }
    }

    // 3. Context-specific resolution
    match auth {
        AuthConfig::None => Ok(None),

        AuthConfig::TokenEnv { env } => {
            match std::env::var(env) {
                Ok(t) if !t.is_empty() => Ok(Some(t)),
                _ => Ok(None),
            }
        }

        AuthConfig::Keychain { service, account } => {
            // Try to get token from keychain
            #[cfg(feature = "keyring")]
            {
                let entry = keyring::Entry::new(service, account)
                    .map_err(|e| CliError::AuthFailed(format!("keychain error: {}", e)))?;
                match entry.get_password() {
                    Ok(t) => Ok(Some(t)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(e) => Err(CliError::AuthFailed(format!("keychain error: {}", e))),
                }
            }

            #[cfg(not(feature = "keyring"))]
            {
                // Keyring feature not enabled
                let _ = (service, account);
                Ok(None)
            }
        }

        AuthConfig::Session { storage, service, access_account, refresh_account: _, expires_at, scopes: _ } => {
            // Check if access token is close to expiring (within 5 minutes)
            let now = chrono::Utc::now().timestamp();
            let buffer = 300; // 5 minute buffer
            if now >= (*expires_at - buffer) {
                // Token expired or expiring soon - need to refresh
                // Note: Actual refresh happens in resolve_context_async
                tracing::debug!("Session access token expired or expiring, needs refresh");
            }

            match storage {
                TokenStorage::File => {
                    tracing::debug!(
                        access_account = %access_account,
                        "Looking up session token in file storage"
                    );
                    
                    let token_file = TokenFile::load()?;
                    match token_file.get(access_account) {
                        Some(t) => {
                            tracing::debug!("Token found in file storage");
                            Ok(Some(t.to_string()))
                        }
                        None => {
                            tracing::warn!(access_account = %access_account, "No token found in file storage");
                            Err(CliError::AuthFailed("Session token not found in token file".to_string()))
                        }
                    }
                }
                TokenStorage::Keychain => {
                    tracing::debug!(
                        service = %service,
                        access_account = %access_account,
                        "Looking up session token in keychain"
                    );

                    // Get access token from keychain
                    #[cfg(feature = "keyring")]
                    {
                        let entry = keyring::Entry::new(service, access_account)
                            .map_err(|e| {
                                tracing::error!(error = %e, service = %service, access_account = %access_account, "Failed to create keyring entry");
                                CliError::AuthFailed(format!("keychain error: {}", e))
                            })?;
                        match entry.get_password() {
                            Ok(t) => {
                                tracing::debug!("Token found in keychain");
                                Ok(Some(t))
                            }
                            Err(keyring::Error::NoEntry) => {
                                tracing::warn!(service = %service, access_account = %access_account, "No entry found in keychain");
                                Err(CliError::AuthFailed("Session token not found in keychain".to_string()))
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "Keychain error");
                                Err(CliError::AuthFailed(format!("keychain error: {}", e)))
                            }
                        }
                    }

                    #[cfg(not(feature = "keyring"))]
                    {
                        let _ = (service, access_account);
                        Err(CliError::AuthFailed("Keyring feature required for keychain session auth".to_string()))
                    }
                }
            }
        }
    }
}

/// Resolve a context with async token refresh capability.
pub async fn resolve_context_async(
    config: &mut GlobalConfig,
    context_name: Option<&str>,
    project_default: Option<&str>,
    cli_token: Option<&str>,
) -> CliResult<ResolvedContext> {
    // Determine context name
    let name = context_name
        .or(project_default)
        .or(config.get_default())
        .ok_or_else(|| {
            CliError::ContextNotFound("no context specified and no default set".into())
        })?
        .to_string();

    // Get context config
    let ctx = config.get_context(&name).ok_or_else(|| {
        CliError::ContextNotFound(name.clone())
    })?.clone();

    // Parse endpoint
    let endpoint = Url::parse(&ctx.endpoint)?;

    // Check if we need to refresh the session
    if let AuthConfig::Session { ref storage, ref service, ref access_account, ref refresh_account, expires_at, ref scopes } = ctx.auth {
        let now = chrono::Utc::now().timestamp();
        let buffer = 300; // 5 minute buffer

        if now >= (expires_at - buffer) && cli_token.is_none() {
            tracing::info!("Session token expired or expiring, attempting refresh...");

            // Try to refresh the token
            match refresh_session_token(&endpoint, storage, service, refresh_account).await {
                Ok((new_access, new_refresh, new_expires_in)) => {
                    // Store new tokens based on storage backend
                    match storage {
                        TokenStorage::File => {
                            let mut token_file = TokenFile::load()?;
                            token_file.set(access_account, &new_access);
                            token_file.set(refresh_account, &new_refresh);
                            token_file.save()?;
                        }
                        TokenStorage::Keychain => {
                            #[cfg(feature = "keyring")]
                            {
                                let access_entry = keyring::Entry::new(service, access_account)
                                    .map_err(|e| CliError::Keychain(e.to_string()))?;
                                access_entry.set_password(&new_access)
                                    .map_err(|e| CliError::Keychain(e.to_string()))?;

                                let refresh_entry = keyring::Entry::new(service, refresh_account)
                                    .map_err(|e| CliError::Keychain(e.to_string()))?;
                                refresh_entry.set_password(&new_refresh)
                                    .map_err(|e| CliError::Keychain(e.to_string()))?;
                            }
                        }
                    }

                    // Update config with new expiry
                    let new_expires_at = chrono::Utc::now().timestamp() + new_expires_in;
                    let mut updated_ctx = ctx.clone();
                    updated_ctx.auth = AuthConfig::Session {
                        storage: storage.clone(),
                        service: service.clone(),
                        access_account: access_account.clone(),
                        refresh_account: refresh_account.clone(),
                        expires_at: new_expires_at,
                        scopes: scopes.clone(),
                    };
                    config.set_context(name.clone(), updated_ctx);
                    let _ = config.save(); // Best effort

                    tracing::info!("Session token refreshed successfully");

                    return Ok(ResolvedContext {
                        name,
                        endpoint,
                        org: ctx.org,
                        token: Some(new_access),
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to refresh token: {}", e);
                    return Err(CliError::AuthFailed(
                        format!("Session expired and refresh failed: {}. Please run 'reactor login' to re-authenticate.", e)
                    ));
                }
            }
        }
    }

    // Normal token resolution
    let token = resolve_token(cli_token, &ctx.auth)?;

    Ok(ResolvedContext {
        name,
        endpoint,
        org: ctx.org,
        token,
    })
}

/// Refresh session tokens using the refresh token.
async fn refresh_session_token(
    endpoint: &Url,
    storage: &TokenStorage,
    service: &str,
    refresh_account: &str,
) -> Result<(String, String, i64), String> {
    // Get refresh token based on storage backend
    let refresh_token = match storage {
        TokenStorage::File => {
            let token_file = TokenFile::load()
                .map_err(|e| format!("failed to load token file: {}", e))?;
            token_file.get(refresh_account)
                .map(|s| s.to_string())
                .ok_or_else(|| "refresh token not found in token file".to_string())?
        }
        TokenStorage::Keychain => {
            #[cfg(feature = "keyring")]
            {
                let entry = keyring::Entry::new(service, refresh_account)
                    .map_err(|e| format!("keychain error: {}", e))?;
                entry.get_password()
                    .map_err(|e| format!("failed to get refresh token: {}", e))?
            }
            #[cfg(not(feature = "keyring"))]
            {
                let _ = service;
                return Err("Keyring feature required for keychain storage".to_string());
            }
        }
    };

    // Make refresh request
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}auth/v1/token", endpoint))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "reactor-cli"),
            ("refresh_token", &refresh_token),
        ])
        .send()
        .await
        .map_err(|e| format!("refresh request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("refresh failed ({}): {}", status, text));
    }

    #[derive(serde::Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    }

    let tokens: RefreshResponse = response.json().await
        .map_err(|e| format!("failed to parse refresh response: {}", e))?;

    Ok((tokens.access_token, tokens.refresh_token, tokens.expires_in))
}

/// Store a token in the OS keychain for a context.
#[cfg(feature = "keyring")]
pub fn store_token_keychain(context_name: &str, token: &str) -> CliResult<()> {
    let entry = keyring::Entry::new("reactor", context_name)
        .map_err(|e| CliError::AuthFailed(format!("keychain error: {}", e)))?;
    entry
        .set_password(token)
        .map_err(|e| CliError::AuthFailed(format!("failed to store token: {}", e)))?;
    Ok(())
}

/// Remove a token from the OS keychain for a context.
#[cfg(feature = "keyring")]
pub fn remove_token_keychain(context_name: &str) -> CliResult<()> {
    let entry = keyring::Entry::new("reactor", context_name)
        .map_err(|e| CliError::AuthFailed(format!("keychain error: {}", e)))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already gone
        Err(e) => Err(CliError::AuthFailed(format!("failed to remove token: {}", e))),
    }
}

/// Check if a token exists in the OS keychain for a context.
#[cfg(feature = "keyring")]
pub fn has_token_keychain(context_name: &str) -> bool {
    keyring::Entry::new("reactor", context_name)
        .and_then(|e| e.get_password())
        .is_ok()
}

// Stub implementations when keyring is not enabled
#[cfg(not(feature = "keyring"))]
pub fn store_token_keychain(_context_name: &str, _token: &str) -> CliResult<()> {
    Err(CliError::Config("keyring support not enabled".into()))
}

#[cfg(not(feature = "keyring"))]
pub fn remove_token_keychain(_context_name: &str) -> CliResult<()> {
    Err(CliError::Config("keyring support not enabled".into()))
}

#[cfg(not(feature = "keyring"))]
pub fn has_token_keychain(_context_name: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GlobalConfig::default();
        assert!(config.default.is_none());
        assert!(config.contexts.is_empty());
    }

    #[test]
    fn test_context_config() {
        let ctx = ContextConfig::new("http://localhost:8080")
            .with_org("acme")
            .with_token_env("MY_TOKEN");

        assert_eq!(ctx.endpoint, "http://localhost:8080");
        assert_eq!(ctx.org, Some("acme".to_string()));
        matches!(ctx.auth, AuthConfig::TokenEnv { env } if env == "MY_TOKEN");
    }

    #[test]
    fn test_serialize_config() {
        let mut config = GlobalConfig::default();
        config.default = Some("local".to_string());
        config.contexts.insert(
            "local".to_string(),
            ContextConfig::new("http://localhost:8080"),
        );

        let toml = toml_edit::ser::to_string_pretty(&config).unwrap();
        assert!(toml.contains("default = \"local\""));
        assert!(toml.contains("[contexts.local]"));
    }
}
