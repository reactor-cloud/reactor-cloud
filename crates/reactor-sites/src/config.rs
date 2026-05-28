//! Sites configuration.

use figment::{providers::Env, Figment};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Deployment mode.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Monolith mode: sites runs with in-process auth.
    #[default]
    Monolith,
    /// Microservices mode: sites talks to remote auth/functions/storage services.
    Microservices,
}

/// Sites service configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SitesConfig {
    /// Database connection URL.
    pub database_url: String,

    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Deployment mode.
    #[serde(default)]
    pub deployment: Deployment,

    /// Working directory for bundle processing.
    #[serde(default = "default_workdir")]
    pub workdir: String,

    /// Functions service URL.
    pub functions_url: String,

    /// Functions API key (internal service key).
    pub functions_api_key: String,

    /// Storage service URL.
    pub storage_url: String,

    /// Storage API key with access to the sites bucket.
    pub storage_api_key: String,

    /// Storage bucket for site static assets.
    /// Defaults to the cluster's main storage bucket (STORAGE_S3_BUCKET).
    #[serde(default)]
    pub storage_bucket: Option<String>,

    /// Jobs service URL (for ISR/ACME background jobs).
    #[serde(default)]
    pub jobs_url: Option<String>,

    /// Jobs API key (internal service key).
    #[serde(default)]
    pub jobs_api_key: Option<String>,

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

    /// Internal revalidation secret for function-driven ISR invalidation.
    pub revalidation_secret: String,

    /// Maximum static files per deployment.
    #[serde(default = "default_static_max_files")]
    pub static_max_files: u32,

    /// Maximum total static size per deployment in bytes.
    #[serde(default = "default_static_max_bytes")]
    pub static_max_bytes: u64,

    /// Default ISR revalidate interval in seconds.
    #[serde(default = "default_isr_default_ttl_secs")]
    pub isr_default_ttl_secs: u64,

    /// Preview subdomain prefix.
    #[serde(default = "default_preview_subdomain")]
    pub preview_subdomain: String,

    /// ACME email for Let's Encrypt registration (G2, domain-acme feature).
    #[serde(default)]
    pub acme_email: Option<String>,

    /// ACME directory URL (defaults to Let's Encrypt production).
    #[serde(default)]
    pub acme_directory: Option<String>,

    /// Enable /metrics endpoint.
    #[serde(default)]
    pub metrics: bool,

    /// Invocation sample rate (0.0 to 1.0).
    #[serde(default = "default_invocation_sample_rate")]
    pub invocation_sample_rate: f64,

    /// Log filter.
    #[serde(default = "default_log")]
    pub log: String,
}

fn default_bind() -> SocketAddr {
    "127.0.0.1:8006".parse().unwrap()
}

fn default_workdir() -> String {
    "/var/lib/reactor-sites".to_string()
}

fn default_static_max_files() -> u32 {
    50_000
}

fn default_static_max_bytes() -> u64 {
    512 * 1024 * 1024 // 512 MiB
}

fn default_isr_default_ttl_secs() -> u64 {
    3600 // 1 hour
}

fn default_preview_subdomain() -> String {
    "preview".to_string()
}

fn default_invocation_sample_rate() -> f64 {
    0.01 // 1%
}

fn default_log() -> String {
    "info".to_string()
}

impl SitesConfig {
    /// Load configuration from environment.
    pub fn from_env() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::prefixed("REACTOR_SITES_"))
            .extract()
    }
}

impl Default for SitesConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            bind: default_bind(),
            deployment: Deployment::default(),
            workdir: default_workdir(),
            functions_url: String::new(),
            functions_api_key: String::new(),
            storage_url: String::new(),
            storage_api_key: String::new(),
            storage_bucket: None,
            jobs_url: None,
            jobs_api_key: None,
            auth_url: None,
            auth_database_url: None,
            auth_data_key: None,
            internal_secret: None,
            revalidation_secret: String::new(),
            static_max_files: default_static_max_files(),
            static_max_bytes: default_static_max_bytes(),
            isr_default_ttl_secs: default_isr_default_ttl_secs(),
            preview_subdomain: default_preview_subdomain(),
            acme_email: None,
            acme_directory: None,
            metrics: false,
            invocation_sample_rate: default_invocation_sample_rate(),
            log: default_log(),
        }
    }
}
