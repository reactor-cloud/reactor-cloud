//! Jobs configuration.

use figment::{
    providers::{Env, Serialized},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Deployment mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Monolith mode — runs auth in-process.
    Monolith,
    /// Microservices mode — calls remote auth server.
    Microservices,
}

impl Default for Deployment {
    fn default() -> Self {
        Self::Monolith
    }
}

/// Jobs server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobsConfig {
    /// PostgreSQL connection URL.
    pub database_url: String,

    /// Server bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Deployment mode.
    #[serde(default)]
    pub deployment: Deployment,

    /// reactor-functions server URL.
    pub functions_url: String,

    /// Internal API key for reactor-functions.
    pub functions_api_key: String,

    /// reactor-data server URL (optional, for ctx.data).
    pub data_url: Option<String>,

    /// Internal API key for reactor-data.
    pub data_api_key: Option<String>,

    /// Number of worker tasks.
    #[serde(default = "default_worker_count")]
    pub worker_count: usize,

    /// Scheduler poll interval in milliseconds.
    #[serde(default = "default_scheduler_interval_ms")]
    pub scheduler_interval_ms: u64,

    /// Default job timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub default_timeout_ms: u64,

    /// Maximum job timeout in milliseconds.
    #[serde(default = "default_max_timeout_ms")]
    pub max_timeout_ms: u64,

    /// Secret for webhook token encryption.
    pub webhook_secret: String,

    /// Maximum concurrent runs per org.
    #[serde(default = "default_max_org_concurrent_runs")]
    pub max_org_concurrent_runs: u32,

    /// Maximum payload size in bytes.
    #[serde(default = "default_max_payload_bytes")]
    pub max_payload_bytes: u64,

    // Auth config for microservices mode
    /// Auth server URL.
    pub auth_url: Option<String>,

    /// Internal shared secret.
    pub internal_secret: Option<String>,

    // Auth config for monolith mode
    /// Auth database URL.
    pub auth_database_url: Option<String>,

    /// Auth column encryption key.
    pub auth_data_key: Option<String>,

    /// Enable Prometheus metrics.
    #[serde(default)]
    pub metrics: bool,

    /// Log filter.
    #[serde(default = "default_log")]
    pub log: String,
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8005".parse().unwrap()
}

fn default_worker_count() -> usize {
    4
}

fn default_scheduler_interval_ms() -> u64 {
    1000
}

fn default_timeout_ms() -> u64 {
    600_000 // 10 minutes
}

fn default_max_timeout_ms() -> u64 {
    3_600_000 // 1 hour
}

fn default_max_org_concurrent_runs() -> u32 {
    50
}

fn default_max_payload_bytes() -> u64 {
    1_048_576 // 1 MiB
}

fn default_log() -> String {
    "info".to_string()
}

impl JobsConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self, figment::Error> {
        Figment::from(Serialized::defaults(JobsConfig::default()))
            .merge(Env::prefixed("REACTOR_JOBS_").split("_"))
            .extract()
    }
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            bind: default_bind(),
            deployment: Deployment::default(),
            functions_url: String::new(),
            functions_api_key: String::new(),
            data_url: None,
            data_api_key: None,
            worker_count: default_worker_count(),
            scheduler_interval_ms: default_scheduler_interval_ms(),
            default_timeout_ms: default_timeout_ms(),
            max_timeout_ms: default_max_timeout_ms(),
            webhook_secret: String::new(),
            max_org_concurrent_runs: default_max_org_concurrent_runs(),
            max_payload_bytes: default_max_payload_bytes(),
            auth_url: None,
            internal_secret: None,
            auth_database_url: None,
            auth_data_key: None,
            metrics: false,
            log: default_log(),
        }
    }
}
