//! Configuration for reactor-auth.

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use url::Url;

/// Configuration for the auth service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Database connection URL.
    pub database_url: String,

    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// AES-256-GCM key for column encryption (base64 encoded, 32 bytes).
    pub data_key: String,

    /// JWT issuer claim.
    #[serde(default = "default_jwt_issuer")]
    pub jwt_issuer: String,

    /// JWT audience claim.
    #[serde(default = "default_jwt_audience")]
    pub jwt_audience: String,

    /// Access token TTL in seconds.
    #[serde(default = "default_access_ttl")]
    pub access_ttl_secs: u64,

    /// Refresh token TTL in seconds.
    #[serde(default = "default_refresh_ttl")]
    pub refresh_ttl_secs: u64,

    /// Internal secret for `/_internal/*` endpoints.
    pub internal_secret: Option<String>,

    /// Public URL for email links.
    pub public_url: Url,

    /// SMTP configuration (optional).
    #[serde(default)]
    pub smtp: Option<SmtpConfig>,

    /// Log level filter.
    #[serde(default = "default_log")]
    pub log: String,

    /// Enable metrics endpoint.
    #[serde(default)]
    pub metrics: bool,
}

/// SMTP configuration for sending emails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// SMTP server hostname.
    pub host: String,

    /// SMTP server port.
    #[serde(default = "default_smtp_port")]
    pub port: u16,

    /// SMTP username.
    pub user: Option<String>,

    /// SMTP password.
    pub password: Option<String>,

    /// From address for outgoing emails.
    pub from: String,

    /// TLS mode: "starttls", "tls", or "none".
    #[serde(default = "default_smtp_tls")]
    pub tls: String,
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8001".parse().unwrap()
}

fn default_jwt_issuer() -> String {
    "reactor-auth".to_string()
}

fn default_jwt_audience() -> String {
    "reactor".to_string()
}

fn default_access_ttl() -> u64 {
    3600 // 1 hour
}

fn default_refresh_ttl() -> u64 {
    2_592_000 // 30 days
}

fn default_log() -> String {
    "info".to_string()
}

fn default_smtp_port() -> u16 {
    587
}

fn default_smtp_tls() -> String {
    "starttls".to_string()
}

impl AuthConfig {
    /// Load configuration from environment and optional TOML file.
    ///
    /// Environment variables take precedence over file values.
    /// Env vars are prefixed with `REACTOR_AUTH_` and use `__` for nesting.
    #[allow(clippy::result_large_err)]
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("reactor.toml").nested())
            .merge(
                Env::prefixed("REACTOR_AUTH_")
                    .map(|key| key.as_str().replace("__", ".").into())
                    .split("_"),
            )
            .extract()
    }

    /// Check if SMTP is configured.
    #[must_use]
    pub fn has_smtp(&self) -> bool {
        self.smtp.is_some()
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate data key is valid base64 and 32 bytes
        let key_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &self.data_key)
                .map_err(|_| ConfigError::InvalidDataKey("invalid base64".to_string()))?;

        if key_bytes.len() != 32 {
            return Err(ConfigError::InvalidDataKey(format!(
                "expected 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        // Validate SMTP TLS mode if configured
        if let Some(smtp) = &self.smtp {
            match smtp.tls.as_str() {
                "starttls" | "tls" | "none" => {}
                other => {
                    return Err(ConfigError::InvalidSmtpTls(other.to_string()));
                }
            }
        }

        Ok(())
    }
}

/// Configuration validation errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Invalid data key.
    #[error("invalid data key: {0}")]
    InvalidDataKey(String),

    /// Invalid SMTP TLS mode.
    #[error("invalid SMTP TLS mode: {0} (expected 'starttls', 'tls', or 'none')")]
    InvalidSmtpTls(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(default_bind(), "0.0.0.0:8001".parse().unwrap());
        assert_eq!(default_jwt_issuer(), "reactor-auth");
        assert_eq!(default_access_ttl(), 3600);
        assert_eq!(default_refresh_ttl(), 2_592_000);
    }
}
