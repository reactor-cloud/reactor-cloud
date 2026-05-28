//! Configuration for reactor-data.

use crate::error::DataError;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Deployment topology for reactor-data.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Auth service embedded in the same process.
    #[default]
    Monolith,
    /// Auth service is a separate microservice.
    Microservices,
}

/// Configuration for reactor-data.
#[derive(Debug, Clone, Deserialize)]
pub struct DataConfig {
    /// Postgres connection string for user data.
    pub database_url: String,

    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Directory containing user migrations.
    pub migrations_dir: Option<PathBuf>,

    /// Whether to run user migrations on startup.
    #[serde(default = "default_run_migrations")]
    pub run_migrations: bool,

    /// User schema name.
    #[serde(default = "default_user_schema")]
    pub user_schema: String,

    /// Maximum embed depth for ?select queries.
    #[serde(default = "default_max_embed_depth")]
    pub max_embed_depth: u8,

    /// Maximum limit for pagination.
    #[serde(default = "default_max_limit")]
    pub max_limit: u32,

    /// Default limit for pagination.
    #[serde(default = "default_default_limit")]
    pub default_limit: u32,

    /// Deployment topology.
    #[serde(default)]
    pub deployment: Deployment,

    /// URL of reactor-auth-server (required for microservices mode).
    pub auth_url: Option<String>,

    /// Internal secret shared with reactor-auth (required for microservices mode).
    pub internal_secret: Option<String>,

    /// Postgres connection string for auth database (required for monolith mode).
    pub auth_database_url: Option<String>,

    /// Column encryption key for auth (required for monolith mode).
    pub auth_data_key: Option<String>,

    /// Log level filter.
    #[serde(default = "default_log")]
    pub log: String,

    /// Enable Prometheus metrics.
    #[serde(default)]
    pub metrics: bool,
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8002".parse().unwrap()
}

fn default_run_migrations() -> bool {
    true
}

fn default_user_schema() -> String {
    "public".to_string()
}

fn default_max_embed_depth() -> u8 {
    5
}

fn default_max_limit() -> u32 {
    1000
}

fn default_default_limit() -> u32 {
    100
}

fn default_log() -> String {
    "info".to_string()
}

impl DataConfig {
    /// Load configuration from environment variables.
    ///
    /// Variables are prefixed with `REACTOR_DATA_`.
    pub fn from_env() -> Result<Self, DataError> {
        let database_url = std::env::var("REACTOR_DATA_DATABASE_URL")
            .map_err(|_| DataError::Config("REACTOR_DATA_DATABASE_URL is required".to_string()))?;

        let bind = std::env::var("REACTOR_DATA_BIND")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_bind);

        let migrations_dir = std::env::var("REACTOR_DATA_MIGRATIONS_DIR")
            .ok()
            .map(PathBuf::from);

        let run_migrations = std::env::var("REACTOR_DATA_RUN_MIGRATIONS")
            .ok()
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .unwrap_or_else(default_run_migrations);

        let user_schema = std::env::var("REACTOR_DATA_USER_SCHEMA")
            .ok()
            .unwrap_or_else(default_user_schema);

        let max_embed_depth = std::env::var("REACTOR_DATA_MAX_EMBED_DEPTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_embed_depth);

        let max_limit = std::env::var("REACTOR_DATA_MAX_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_limit);

        let default_limit = std::env::var("REACTOR_DATA_DEFAULT_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_default_limit);

        let deployment = match std::env::var("REACTOR_DATA_DEPLOYMENT").ok().as_deref() {
            Some("microservices") => Deployment::Microservices,
            _ => Deployment::Monolith,
        };

        let auth_url = std::env::var("REACTOR_DATA_AUTH_URL").ok();
        let internal_secret = std::env::var("REACTOR_DATA_INTERNAL_SECRET").ok();
        let auth_database_url = std::env::var("REACTOR_DATA_AUTH_DATABASE_URL").ok();
        let auth_data_key = std::env::var("REACTOR_DATA_AUTH_DATA_KEY").ok();

        let log = std::env::var("REACTOR_LOG")
            .ok()
            .unwrap_or_else(default_log);

        let metrics = std::env::var("REACTOR_DATA_METRICS")
            .ok()
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .unwrap_or(false);

        let config = Self {
            database_url,
            bind,
            migrations_dir,
            run_migrations,
            user_schema,
            max_embed_depth,
            max_limit,
            default_limit,
            deployment,
            auth_url,
            internal_secret,
            auth_database_url,
            auth_data_key,
            log,
            metrics,
        };

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration based on deployment mode.
    pub fn validate(&self) -> Result<(), DataError> {
        match self.deployment {
            Deployment::Monolith => {
                if self.auth_database_url.is_none() {
                    return Err(DataError::Config(
                        "REACTOR_DATA_AUTH_DATABASE_URL required for monolith mode".to_string(),
                    ));
                }
                if self.auth_data_key.is_none() {
                    return Err(DataError::Config(
                        "REACTOR_DATA_AUTH_DATA_KEY required for monolith mode".to_string(),
                    ));
                }
            }
            Deployment::Microservices => {
                if self.auth_url.is_none() {
                    return Err(DataError::Config(
                        "REACTOR_DATA_AUTH_URL required for microservices mode".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}
