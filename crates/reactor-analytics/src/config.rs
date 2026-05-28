//! Analytics configuration.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Deployment topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Single binary with all capabilities.
    #[default]
    Monolith,
    /// Separate services communicating over HTTP.
    Microservices,
}

/// Analytics service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    /// Database connection URL.
    pub database_url: String,

    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Deployment topology.
    #[serde(default)]
    pub deployment: Deployment,

    /// Admin token for internal endpoints.
    pub admin_token: Option<String>,

    /// Auth service URL (microservices mode).
    pub auth_url: Option<String>,

    /// Auth database URL (monolith mode).
    pub auth_database_url: Option<String>,

    /// Auth column encryption key (monolith mode).
    pub auth_data_key: Option<String>,

    /// Internal secret for inter-service communication.
    pub internal_secret: Option<String>,

    /// Path to MaxMind GeoLite2 country database.
    pub geo_db_path: Option<PathBuf>,

    /// Honor DNT (Do Not Track) headers.
    #[serde(default = "default_honor_dnt")]
    pub honor_dnt: bool,

    /// Maximum properties + context size per event (bytes).
    #[serde(default = "default_max_properties_bytes")]
    pub max_properties_bytes: usize,

    /// Default per-org monthly event quota.
    #[serde(default = "default_quota_per_org_monthly")]
    pub quota_per_org_monthly: u64,

    /// Default retention days for events.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    /// Batch interval in milliseconds.
    #[serde(default = "default_batch_interval_ms")]
    pub batch_interval_ms: u64,

    /// Maximum rows per batch.
    #[serde(default = "default_batch_max_rows")]
    pub batch_max_rows: usize,

    /// Batch queue depth.
    #[serde(default = "default_batch_queue_depth")]
    pub batch_queue_depth: usize,

    /// Query timeout in milliseconds.
    #[serde(default = "default_query_timeout_ms")]
    pub query_timeout_ms: u64,

    /// Maximum rows scanned per query.
    #[serde(default = "default_query_max_rows")]
    pub query_max_rows: u64,

    /// Maximum days for raw event queries.
    #[serde(default = "default_query_raw_range_days")]
    pub query_raw_range_days: u32,

    /// Maximum days for aggregate queries.
    #[serde(default = "default_query_agg_range_days")]
    pub query_agg_range_days: u32,

    /// Rate limit requests per second per key.
    #[serde(default = "default_rate_limit_rps")]
    pub rate_limit_rps: u32,

    /// Rate limit burst size per key.
    #[serde(default = "default_rate_limit_burst")]
    pub rate_limit_burst: u32,

    /// Rate limit tokens refilled per second.
    #[serde(default = "default_rate_limit_per_second")]
    pub rate_limit_per_second: u32,

    /// Quota cache TTL in seconds.
    #[serde(default = "default_quota_cache_ttl_secs")]
    pub quota_cache_ttl_secs: u64,

    /// Global event sample rate (0.0 to 1.0).
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,

    /// Enable Prometheus metrics endpoint.
    #[serde(default)]
    pub metrics: bool,

    /// Logging level.
    #[serde(default = "default_log")]
    pub log: String,
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8006".parse().unwrap()
}

fn default_honor_dnt() -> bool {
    true
}

fn default_max_properties_bytes() -> usize {
    32768
}

fn default_quota_per_org_monthly() -> u64 {
    1_000_000
}

fn default_retention_days() -> u32 {
    90
}

fn default_batch_interval_ms() -> u64 {
    200
}

fn default_batch_max_rows() -> usize {
    500
}

fn default_batch_queue_depth() -> usize {
    50000
}

fn default_query_timeout_ms() -> u64 {
    30000
}

fn default_query_max_rows() -> u64 {
    100000
}

fn default_query_raw_range_days() -> u32 {
    90
}

fn default_query_agg_range_days() -> u32 {
    730
}

fn default_rate_limit_rps() -> u32 {
    1000
}

fn default_rate_limit_burst() -> u32 {
    100
}

fn default_rate_limit_per_second() -> u32 {
    50
}

fn default_quota_cache_ttl_secs() -> u64 {
    60
}

fn default_sample_rate() -> f64 {
    1.0
}

fn default_log() -> String {
    "info".to_string()
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            bind: default_bind(),
            deployment: Deployment::default(),
            admin_token: None,
            auth_url: None,
            auth_database_url: None,
            auth_data_key: None,
            internal_secret: None,
            geo_db_path: None,
            honor_dnt: default_honor_dnt(),
            max_properties_bytes: default_max_properties_bytes(),
            quota_per_org_monthly: default_quota_per_org_monthly(),
            retention_days: default_retention_days(),
            batch_interval_ms: default_batch_interval_ms(),
            batch_max_rows: default_batch_max_rows(),
            batch_queue_depth: default_batch_queue_depth(),
            query_timeout_ms: default_query_timeout_ms(),
            query_max_rows: default_query_max_rows(),
            query_raw_range_days: default_query_raw_range_days(),
            query_agg_range_days: default_query_agg_range_days(),
            rate_limit_rps: default_rate_limit_rps(),
            rate_limit_burst: default_rate_limit_burst(),
            rate_limit_per_second: default_rate_limit_per_second(),
            quota_cache_ttl_secs: default_quota_cache_ttl_secs(),
            sample_rate: default_sample_rate(),
            metrics: false,
            log: default_log(),
        }
    }
}
