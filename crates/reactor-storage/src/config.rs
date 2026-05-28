//! Storage configuration.

use figment::{providers::Env, Figment};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Deployment mode.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Monolith mode: storage runs with in-process auth.
    #[default]
    Monolith,
    /// Microservices mode: storage talks to remote auth service.
    Microservices,
}

/// Storage service configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// Database connection URL.
    pub database_url: String,

    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Deployment mode.
    #[serde(default)]
    pub deployment: Deployment,

    /// Admin token for internal/system operations.
    /// When provided, requests with this token bypass normal auth.
    #[serde(default)]
    pub admin_token: Option<String>,

    /// Base path for local filesystem storage (only used with fs feature).
    #[serde(default)]
    pub fs_base_path: Option<String>,

    /// S3 bucket name (only used with s3 feature).
    #[serde(default)]
    pub s3_bucket: Option<String>,

    /// S3 region (only used with s3 feature).
    #[serde(default)]
    pub s3_region: Option<String>,

    /// S3 endpoint override (for MinIO or localstack).
    #[serde(default)]
    pub s3_endpoint: Option<String>,

    /// Auth service URL (for remote auth client in microservices mode).
    #[serde(default)]
    pub auth_url: Option<String>,

    /// Auth database URL (for in-process auth in monolith mode).
    #[serde(default)]
    pub auth_database_url: Option<String>,

    /// Data encryption key for auth (monolith mode).
    #[serde(default)]
    pub auth_data_key: Option<String>,

    /// Secret key for HMAC-signed URLs.
    #[serde(default)]
    pub signing_secret: Option<String>,

    /// Signed URL expiration in seconds.
    #[serde(default = "default_signed_url_expiry")]
    pub signed_url_expiry_secs: u64,

    /// Maximum upload size in bytes.
    #[serde(default = "default_max_upload_size")]
    pub max_upload_size: u64,

    /// Enable /metrics endpoint.
    #[serde(default)]
    pub metrics: bool,

    /// Log filter.
    #[serde(default = "default_log")]
    pub log: String,
}

fn default_bind() -> SocketAddr {
    "127.0.0.1:8082".parse().unwrap()
}

fn default_signed_url_expiry() -> u64 {
    3600 // 1 hour
}

fn default_max_upload_size() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

fn default_log() -> String {
    "info".to_string()
}

impl StorageConfig {
    /// Load configuration from environment.
    pub fn from_env() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::prefixed("REACTOR_STORAGE_"))
            .extract()
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            bind: default_bind(),
            deployment: Deployment::default(),
            admin_token: None,
            fs_base_path: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            auth_url: None,
            auth_database_url: None,
            auth_data_key: None,
            signing_secret: None,
            signed_url_expiry_secs: default_signed_url_expiry(),
            max_upload_size: default_max_upload_size(),
            metrics: false,
            log: default_log(),
        }
    }
}
