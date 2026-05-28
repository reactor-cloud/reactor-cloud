//! Functions configuration.

use figment::{providers::Env, Figment};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Deployment mode.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Monolith mode: functions runs with in-process auth.
    #[default]
    Monolith,
    /// Microservices mode: functions talks to remote auth service.
    Microservices,
}

/// Functions service configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionsConfig {
    /// Database connection URL.
    pub database_url: String,

    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Deployment mode.
    #[serde(default)]
    pub deployment: Deployment,

    /// Working directory for function bundles and runtime artifacts.
    #[serde(default = "default_workdir")]
    pub workdir: String,

    /// Storage service URL.
    pub storage_url: String,

    /// Storage API key with access to _reactor_functions bucket.
    pub storage_api_key: String,

    /// Data encryption key for env secrets (column encryption).
    pub data_key: String,

    /// Auth service URL (for remote auth client in microservices mode).
    #[serde(default)]
    pub auth_url: Option<String>,

    /// Auth database URL (for in-process auth in monolith mode).
    #[serde(default)]
    pub auth_database_url: Option<String>,

    /// Data encryption key for auth (monolith mode).
    #[serde(default)]
    pub auth_data_key: Option<String>,

    /// Internal secret for service-to-service auth (microservices mode).
    #[serde(default)]
    pub internal_secret: Option<String>,

    /// Default invoke timeout in milliseconds.
    #[serde(default = "default_invoke_timeout_ms")]
    pub invoke_default_timeout_ms: u64,

    /// Maximum invoke timeout in milliseconds (server-level cap).
    #[serde(default = "default_max_timeout_ms")]
    pub invoke_max_timeout_ms: u64,

    /// Maximum bundle size in bytes.
    #[serde(default = "default_bundle_max_bytes")]
    pub bundle_max_bytes: u64,

    // Bun runtime config
    /// Path to bun binary.
    #[serde(default = "default_bun_bin")]
    pub bun_bin: String,

    /// Bun idle TTL in seconds (warm pool eviction).
    #[serde(default = "default_bun_idle_ttl_secs")]
    pub bun_idle_ttl_secs: u64,

    /// Maximum warm instances per function for Bun.
    #[serde(default = "default_bun_max_instances")]
    pub bun_max_instances_per_fn: u32,

    // Lambda runtime config
    /// AWS region for Lambda.
    #[serde(default)]
    pub lambda_region: Option<String>,

    /// Lambda execution role ARN.
    #[serde(default)]
    pub lambda_role_arn: Option<String>,

    /// S3 bucket for Lambda bundles.
    #[serde(default)]
    pub lambda_bundle_s3_bucket: Option<String>,

    /// Lambda Web Adapter layer ARN.
    #[serde(default)]
    pub lambda_lwa_layer_arn: Option<String>,

    /// CloudWatch log group prefix.
    #[serde(default = "default_lambda_log_group_prefix")]
    pub lambda_log_group_prefix: String,

    /// Enable /metrics endpoint.
    #[serde(default)]
    pub metrics: bool,

    /// Log filter.
    #[serde(default = "default_log")]
    pub log: String,
}

fn default_bind() -> SocketAddr {
    "127.0.0.1:8083".parse().unwrap()
}

fn default_workdir() -> String {
    "/var/lib/reactor-functions".to_string()
}

fn default_invoke_timeout_ms() -> u64 {
    30_000 // 30 seconds
}

fn default_max_timeout_ms() -> u64 {
    300_000 // 5 minutes
}

fn default_bundle_max_bytes() -> u64 {
    50 * 1024 * 1024 // 50 MiB
}

fn default_bun_bin() -> String {
    "bun".to_string()
}

fn default_bun_idle_ttl_secs() -> u64 {
    300 // 5 minutes
}

fn default_bun_max_instances() -> u32 {
    8
}

fn default_lambda_log_group_prefix() -> String {
    "/reactor/functions/".to_string()
}

fn default_log() -> String {
    "info".to_string()
}

impl FunctionsConfig {
    /// Load configuration from environment.
    pub fn from_env() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::prefixed("REACTOR_FUNCTIONS_"))
            .extract()
    }
}

impl Default for FunctionsConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            bind: default_bind(),
            deployment: Deployment::default(),
            workdir: default_workdir(),
            storage_url: String::new(),
            storage_api_key: String::new(),
            data_key: String::new(),
            auth_url: None,
            auth_database_url: None,
            auth_data_key: None,
            internal_secret: None,
            invoke_default_timeout_ms: default_invoke_timeout_ms(),
            invoke_max_timeout_ms: default_max_timeout_ms(),
            bundle_max_bytes: default_bundle_max_bytes(),
            bun_bin: default_bun_bin(),
            bun_idle_ttl_secs: default_bun_idle_ttl_secs(),
            bun_max_instances_per_fn: default_bun_max_instances(),
            lambda_region: None,
            lambda_role_arn: None,
            lambda_bundle_s3_bucket: None,
            lambda_lwa_layer_arn: None,
            lambda_log_group_prefix: default_lambda_log_group_prefix(),
            metrics: false,
            log: default_log(),
        }
    }
}
