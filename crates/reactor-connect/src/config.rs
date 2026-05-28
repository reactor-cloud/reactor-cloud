//! Connect service configuration.

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// Configuration for the Connect service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectConfig {
    /// Database URL (can be vault: prefixed).
    #[serde(default = "default_database_url")]
    pub database_url: String,

    /// Data key for encrypting sensitive config (vault: prefixed).
    #[serde(default)]
    pub data_key: Option<String>,

    /// Base URL for downstream jobs service.
    #[serde(default = "default_jobs_url")]
    pub jobs_url: String,

    /// Base URL for downstream data service.
    #[serde(default = "default_data_url")]
    pub data_url: String,

    /// Base URL for downstream storage service.
    #[serde(default = "default_storage_url")]
    pub storage_url: String,

    /// Admin token for downstream service calls.
    #[serde(default)]
    pub admin_token: Option<String>,

    /// Maximum concurrent action invocations per org.
    #[serde(default = "default_max_concurrent_actions")]
    pub max_concurrent_actions: u32,

    /// Default sandbox TTL in seconds.
    #[serde(default = "default_sandbox_ttl_seconds")]
    pub sandbox_ttl_seconds: u64,

    /// Token refresh check interval in seconds.
    #[serde(default = "default_token_refresh_interval")]
    pub token_refresh_interval_seconds: u64,
}

fn default_database_url() -> String {
    "postgres://localhost/reactor".to_string()
}

fn default_jobs_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_data_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_storage_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_max_concurrent_actions() -> u32 {
    100
}

fn default_sandbox_ttl_seconds() -> u64 {
    3600 // 1 hour
}

fn default_token_refresh_interval() -> u64 {
    60 // Check every minute
}

impl Default for ConnectConfig {
    fn default() -> Self {
        Self {
            database_url: default_database_url(),
            data_key: None,
            jobs_url: default_jobs_url(),
            data_url: default_data_url(),
            storage_url: default_storage_url(),
            admin_token: None,
            max_concurrent_actions: default_max_concurrent_actions(),
            sandbox_ttl_seconds: default_sandbox_ttl_seconds(),
            token_refresh_interval_seconds: default_token_refresh_interval(),
        }
    }
}

impl ConnectConfig {
    /// Load configuration from environment and optional TOML file.
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("Reactor.toml").nested())
            .merge(Env::prefixed("REACTOR_CONNECT_").split("_"))
            .extract()
    }
}
