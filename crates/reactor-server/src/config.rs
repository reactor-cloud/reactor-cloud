//! Configuration types for reactor-server.
//!
//! The unified server uses a single `ReactorConfig` that combines:
//! - Project metadata
//! - Server settings (single bind address for all capabilities)
//! - Database settings (single PgPool shared by all capabilities)
//! - Per-capability config slices (simplified versions without auth/db duplication)

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Unified configuration for the reactor-server.
///
/// Each capability slice is optional — omitting a capability removes it from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorConfig {
    /// Project metadata.
    #[serde(default)]
    pub project: ProjectConfig,

    /// Server settings.
    #[serde(default)]
    pub server: ServerConfig,

    /// Database settings (shared by all capabilities).
    pub database: DatabaseConfig,

    /// Tracing/logging settings.
    #[serde(default)]
    pub tracing: TracingConfig,

    /// Admin endpoint settings.
    pub admin: AdminConfig,

    /// Vault settings (optional — if not set, uses embedded vault).
    #[serde(default)]
    pub vault: Option<VaultConfigSlice>,

    /// AI capability configuration.
    #[cfg(feature = "cap-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfigSlice>,

    /// Auth capability configuration.
    #[cfg(feature = "cap-auth")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfigSlice>,

    /// Data capability configuration.
    #[cfg(feature = "cap-data")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<DataConfigSlice>,

    /// Storage capability configuration.
    #[cfg(feature = "cap-storage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfigSlice>,

    /// Functions capability configuration.
    #[cfg(feature = "cap-functions")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functions: Option<FunctionsConfigSlice>,

    /// Jobs capability configuration.
    #[cfg(feature = "cap-jobs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<JobsConfigSlice>,

    /// Connect capability configuration.
    #[cfg(feature = "cap-connect")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect: Option<ConnectConfigSlice>,

    /// Sites capability configuration.
    #[cfg(feature = "cap-sites")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sites: Option<SitesConfigSlice>,

    /// Analytics capability configuration.
    #[cfg(feature = "cap-analytics")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analytics: Option<AnalyticsConfigSlice>,

    /// Cloud control plane configuration.
    #[cfg(feature = "cap-cloud")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud: Option<CloudConfigSlice>,
}

/// Project metadata.
///
/// This defines the project identity for this server instance:
/// - `id` — Immutable UUID (if empty, a nil UUID is used for dev mode)
/// - `ref` — URL-safe 20-char identifier used in subdomains (derived from id if not specified)
/// - `name` — Human-readable project name
/// - `env` — Deployment environment (production/preview/dev)
///
/// # Subdomain format
///
/// ```text
/// {project_ref}.reactor.cloud
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name (human-readable).
    #[serde(default)]
    pub name: String,

    /// Project ID (UUID string, immutable).
    ///
    /// If empty, a nil UUID is used for development. In production,
    /// this should be a stable UUIDv7 assigned during project creation.
    #[serde(default)]
    pub id: String,

    /// Project ref (URL-safe subdomain identifier).
    ///
    /// If not specified, derived deterministically from the project ID
    /// using blake3 + base32 (20 lowercase alphanumeric characters).
    #[serde(default, rename = "ref")]
    pub ref_: Option<String>,

    /// Environment: "production", "preview", or "dev".
    ///
    /// Affects default behaviors like logging verbosity and
    /// which external services are used.
    #[serde(default)]
    pub env: Option<String>,
}

impl ProjectConfig {
    /// Get the project ID as a typed `ProjectId`.
    ///
    /// Returns `ProjectId::nil()` if the ID is empty or invalid.
    pub fn project_id(&self) -> reactor_core::ProjectId {
        if self.id.is_empty() {
            reactor_core::ProjectId::nil()
        } else {
            reactor_core::ProjectId::parse(&self.id)
                .unwrap_or_else(|_| reactor_core::ProjectId::nil())
        }
    }

    /// Get the project ref, deriving from ID if not specified.
    ///
    /// This is deterministic: the same ID always produces the same ref.
    pub fn project_ref(&self) -> reactor_core::ProjectRef {
        if let Some(ref ref_str) = self.ref_ {
            reactor_core::ProjectRef::parse(ref_str)
                .unwrap_or_else(|_| self.project_id().to_ref())
        } else {
            self.project_id().to_ref()
        }
    }

    /// Get the project name, with fallback to "reactor".
    pub fn project_name(&self) -> &str {
        if self.name.is_empty() {
            "reactor"
        } else {
            &self.name
        }
    }

    /// Get the tenant environment.
    ///
    /// Defaults to `Production` if not specified.
    pub fn tenant_env(&self) -> reactor_core::TenantEnv {
        self.env
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(reactor_core::TenantEnv::Production)
    }

    /// Build a `TenantCtx` from this config.
    ///
    /// This is the canonical way to get the tenant context for single-tenant mode.
    pub fn to_tenant_ctx(&self) -> reactor_core::TenantCtx {
        reactor_core::TenantCtx::new(
            self.project_id(),
            self.project_ref(),
            self.project_name(),
            self.tenant_env(),
        )
    }
}

/// Server settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Request timeout in seconds.
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            request_timeout_secs: default_request_timeout(),
        }
    }
}

/// Database settings (shared by all capabilities).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL.
    pub url: String,

    /// Maximum pool connections.
    #[serde(default = "default_pool_max")]
    pub pool_max: u32,

    /// Connection acquire timeout in seconds.
    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout_secs: u64,
}

/// Tracing/logging settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Log filter (e.g., "info,reactor_auth=debug").
    #[serde(default = "default_filter")]
    pub filter: String,

    /// Log format: "json" or "pretty".
    #[serde(default = "default_fmt")]
    pub fmt: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            filter: default_filter(),
            fmt: default_fmt(),
        }
    }
}

/// Admin endpoint settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    /// Bearer token for /_admin/* endpoints.
    pub token: String,

    /// Allow admin access from non-loopback addresses.
    #[serde(default)]
    pub allow_remote: bool,
}

// =============================================================================
// Auth config slice (for unified server)
// =============================================================================

/// Auth capability configuration (unified server slice).
///
/// Omits database_url (uses shared) and bind (single server port).
#[cfg(feature = "cap-auth")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfigSlice {
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

    /// Public URL for email links.
    pub public_url: String,

    /// SMTP configuration (optional).
    #[serde(default)]
    pub smtp: Option<SmtpConfig>,
}

/// SMTP configuration for sending emails.
#[cfg(feature = "cap-auth")]
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

// =============================================================================
// Data config slice (for unified server)
// =============================================================================

/// Data capability configuration (unified server slice).
#[cfg(feature = "cap-data")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfigSlice {
    /// Directory containing user migrations.
    #[serde(default)]
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
}

// =============================================================================
// AI config slice (for unified server)
// =============================================================================

/// AI capability configuration (unified server slice).
#[cfg(feature = "cap-ai")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfigSlice {
    /// OpenRouter API key (can use `vault:` prefix).
    #[serde(default)]
    pub openrouter_api_key: Option<String>,

    /// AWS Access Key ID for Bedrock (can use `vault:` prefix).
    #[serde(default)]
    pub aws_access_key_id: Option<String>,

    /// AWS Secret Access Key for Bedrock (can use `vault:` prefix).
    #[serde(default)]
    pub aws_secret_access_key: Option<String>,

    /// AWS Session Token for Bedrock (optional, for STS credentials).
    #[serde(default)]
    pub aws_session_token: Option<String>,

    /// AWS Bedrock region (defaults to us-east-1).
    #[serde(default)]
    pub aws_bedrock_region: Option<String>,

    /// Azure Foundry endpoint.
    #[serde(default)]
    pub azure_foundry_endpoint: Option<String>,

    /// Azure Foundry API key (can use `vault:` prefix).
    #[serde(default)]
    pub azure_foundry_api_key: Option<String>,

    /// Path to registry overlay TOML file (optional).
    #[serde(default)]
    pub registry_overlay: Option<std::path::PathBuf>,

    /// URL to fetch registry overlay from (optional).
    #[serde(default)]
    pub registry_url: Option<String>,

    /// Default alias to use when model is not specified.
    #[serde(default)]
    pub default_alias: Option<String>,
}

// =============================================================================
// Storage config slice (for unified server)
// =============================================================================

/// Storage capability configuration (unified server slice).
#[cfg(feature = "cap-storage")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfigSlice {
    /// Storage backend: "fs" or "s3".
    #[serde(default = "default_storage_backend")]
    pub backend: String,

    /// Base path for local filesystem storage.
    #[serde(default)]
    pub fs_base_path: Option<String>,

    /// S3 bucket name.
    #[serde(default)]
    pub s3_bucket: Option<String>,

    /// S3 region.
    #[serde(default)]
    pub s3_region: Option<String>,

    /// S3 endpoint override (for MinIO or localstack).
    #[serde(default)]
    pub s3_endpoint: Option<String>,

    /// Secret key for HMAC-signed URLs.
    pub signing_secret: String,

    /// Signed URL expiration in seconds.
    #[serde(default = "default_signed_url_expiry")]
    pub signed_url_expiry_secs: u64,

    /// Maximum upload size in bytes.
    #[serde(default = "default_max_upload_size")]
    pub max_upload_size: u64,
}

// =============================================================================
// Functions config slice (for unified server)
// =============================================================================

/// Functions capability configuration (unified server slice).
#[cfg(feature = "cap-functions")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionsConfigSlice {
    /// Working directory for function bundles and runtime artifacts.
    #[serde(default = "default_functions_workdir")]
    pub workdir: String,

    /// Data encryption key for env secrets.
    pub data_key: String,

    /// Available runtimes (subset of ["wasm", "bun", "lambda"]).
    #[serde(default = "default_runtimes")]
    pub runtimes: Vec<String>,

    /// Default invoke timeout in milliseconds.
    #[serde(default = "default_invoke_timeout_ms")]
    pub invoke_default_timeout_ms: u64,

    /// Maximum invoke timeout in milliseconds.
    #[serde(default = "default_invoke_max_timeout_ms")]
    pub invoke_max_timeout_ms: u64,

    /// Maximum bundle size in bytes.
    #[serde(default = "default_bundle_max_bytes")]
    pub bundle_max_bytes: u64,

    // Bun runtime config
    /// Path to bun binary.
    #[serde(default = "default_bun_bin")]
    pub bun_bin: String,

    /// Bun idle TTL in seconds.
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
}

// =============================================================================
// Jobs config slice (for unified server)
// =============================================================================

/// Jobs capability configuration (unified server slice).
#[cfg(feature = "cap-jobs")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobsConfigSlice {
    /// Number of worker tasks.
    #[serde(default = "default_worker_count")]
    pub worker_count: usize,

    /// Scheduler poll interval in milliseconds.
    #[serde(default = "default_scheduler_interval_ms")]
    pub scheduler_interval_ms: u64,

    /// Default job timeout in milliseconds.
    #[serde(default = "default_job_timeout_ms")]
    pub default_timeout_ms: u64,

    /// Maximum job timeout in milliseconds.
    #[serde(default = "default_job_max_timeout_ms")]
    pub max_timeout_ms: u64,

    /// Secret for webhook token encryption.
    pub webhook_secret: String,

    /// Maximum concurrent runs per org.
    #[serde(default = "default_max_org_concurrent_runs")]
    pub max_org_concurrent_runs: u32,

    /// Maximum payload size in bytes.
    #[serde(default = "default_max_payload_bytes")]
    pub max_payload_bytes: u64,
}

// =============================================================================
// Connect config slice (for unified server)
// =============================================================================

/// Connect capability configuration (unified server slice).
#[cfg(feature = "cap-connect")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectConfigSlice {
    /// Data encryption key for credentials (base64 encoded, 32 bytes).
    pub data_key: String,

    /// Jobs URL for delegated sync scheduling.
    #[serde(default = "default_connect_jobs_url")]
    pub jobs_url: String,

    /// Data URL for ReactorDataSink.
    #[serde(default = "default_connect_data_url")]
    pub data_url: String,

    /// Storage URL for ReactorStorageSink.
    #[serde(default = "default_connect_storage_url")]
    pub storage_url: String,

    /// Token refresh check interval in seconds.
    #[serde(default = "default_connect_refresh_interval_secs")]
    pub refresh_interval_secs: u64,

    /// Sandbox schema TTL in seconds (cleanup after this period).
    #[serde(default = "default_connect_sandbox_ttl_secs")]
    pub sandbox_ttl_secs: u64,

    /// Maximum concurrent sync runs per org.
    #[serde(default = "default_connect_max_concurrent_syncs")]
    pub max_concurrent_syncs: u32,
}

// =============================================================================
// Sites config slice (for unified server)
// =============================================================================

/// Sites capability configuration (unified server slice).
#[cfg(feature = "cap-sites")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitesConfigSlice {
    /// Working directory for site bundles.
    #[serde(default = "default_sites_workdir")]
    pub workdir: String,

    /// Maximum bundle size in bytes.
    #[serde(default = "default_sites_bundle_max_bytes")]
    pub bundle_max_bytes: u64,

    /// Enable ISR (Incremental Static Regeneration).
    #[serde(default = "default_isr_enabled")]
    pub isr_enabled: bool,

    /// Default ISR revalidation period in seconds.
    #[serde(default = "default_isr_revalidate_secs")]
    pub isr_default_revalidate_secs: u64,

    /// Preview subdomain prefix for preview deployments.
    #[serde(default = "default_sites_preview_subdomain")]
    pub preview_subdomain: String,

    /// Internal revalidation secret for function-driven ISR invalidation.
    #[serde(default)]
    pub revalidation_secret: Option<String>,

    /// Default org slug for sites deployment (used when deploying bundles).
    #[serde(default = "default_sites_default_org")]
    pub default_org_slug: String,
}

// =============================================================================
// Analytics config slice (for unified server)
// =============================================================================

/// Analytics capability configuration (unified server slice).
#[cfg(feature = "cap-analytics")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfigSlice {
    /// Internal secret for service-to-service communication.
    #[serde(default)]
    pub internal_secret: Option<String>,

    /// Path to MaxMind GeoLite2 country database.
    #[serde(default)]
    pub geo_db_path: Option<PathBuf>,

    /// Honor DNT (Do Not Track) headers.
    #[serde(default)]
    pub honor_dnt: Option<bool>,

    /// Maximum properties + context size per event (bytes).
    #[serde(default)]
    pub max_properties_bytes: Option<usize>,

    /// Default per-org monthly event quota.
    #[serde(default)]
    pub quota_per_org_monthly: Option<u64>,

    /// Default retention days for events.
    #[serde(default)]
    pub retention_days: Option<u32>,

    /// Batch interval in milliseconds.
    #[serde(default)]
    pub batch_interval_ms: Option<u64>,

    /// Maximum rows per batch.
    #[serde(default)]
    pub batch_max_rows: Option<usize>,

    /// Batch queue depth.
    #[serde(default)]
    pub batch_queue_depth: Option<usize>,

    /// Query timeout in milliseconds.
    #[serde(default)]
    pub query_timeout_ms: Option<u64>,

    /// Maximum rows scanned per query.
    #[serde(default)]
    pub query_max_rows: Option<u64>,

    /// Maximum days for raw event queries.
    #[serde(default)]
    pub query_raw_range_days: Option<u32>,

    /// Maximum days for aggregate queries.
    #[serde(default)]
    pub query_agg_range_days: Option<u32>,

    /// Rate limit requests per second per key.
    #[serde(default)]
    pub rate_limit_rps: Option<u32>,

    /// Rate limit burst size per key.
    #[serde(default)]
    pub rate_limit_burst: Option<u32>,

    /// Rate limit tokens refilled per second.
    #[serde(default)]
    pub rate_limit_per_second: Option<u32>,

    /// Quota cache TTL in seconds.
    #[serde(default)]
    pub quota_cache_ttl_secs: Option<u64>,

    /// Global event sample rate (0.0 to 1.0).
    #[serde(default)]
    pub sample_rate: Option<f64>,
}

// =============================================================================
// Cloud control plane config slice (for unified server)
// =============================================================================

/// Cloud control plane configuration (unified server slice).
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfigSlice {
    /// Base domain for tenant subdomains (e.g., "reactor.cloud").
    #[serde(default = "default_cloud_base_domain")]
    pub base_domain: String,

    /// Backend target for gateway routing (e.g., "localhost:8000").
    #[serde(default = "default_cloud_backend_target")]
    pub backend_target: String,

    /// TLS mode for routes (e.g., "none", "tls", "wildcard").
    #[serde(default = "default_cloud_tls_mode_opt")]
    pub tls_mode: Option<String>,

    /// Cloud provider type: "single_node" (default) or "shared_cluster".
    #[serde(default)]
    pub provider: Option<String>,

    /// Enable multi-tenant mode with host-based routing.
    #[serde(default)]
    pub multi_tenant: bool,

    /// Cache TTL for tenant resolution in seconds.
    #[serde(default = "default_tenant_cache_ttl")]
    pub tenant_cache_ttl_secs: u64,

    /// Shared pool configuration (Phase 4 multi-tenant).
    #[serde(default)]
    pub shared_pool: Option<SharedPoolConfig>,

    /// Realtime backend configuration (Phase 4 NATS).
    #[serde(default)]
    pub realtime: Option<RealtimeConfig>,

    /// PubSub backend configuration (Phase 4 NATS).
    #[serde(default)]
    pub pubsub: Option<PubSubConfig>,

    /// Per-tier quota configuration.
    #[serde(default)]
    pub quotas: Option<QuotasConfig>,
}

#[cfg(feature = "cap-cloud")]
impl Default for CloudConfigSlice {
    fn default() -> Self {
        Self {
            base_domain: default_cloud_base_domain(),
            backend_target: default_cloud_backend_target(),
            tls_mode: Some(default_cloud_tls_mode()),
            provider: None,
            multi_tenant: false,
            tenant_cache_ttl_secs: default_tenant_cache_ttl(),
            shared_pool: None,
            realtime: None,
            pubsub: None,
            quotas: None,
        }
    }
}

/// Shared pool configuration for multi-tenant mode (Phase 4).
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedPoolConfig {
    /// Maximum number of active tenants in the adapter cache.
    /// Eviction occurs when this threshold is exceeded.
    #[serde(default = "default_max_active_tenants")]
    pub max_active_tenants: usize,

    /// Idle timeout for tenant adapters.
    /// Tenants not accessed within this duration are evicted.
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,

    /// Maximum concurrent cold loads (parallel first-time tenant fetches).
    #[serde(default = "default_cold_load_concurrency")]
    pub cold_load_concurrency: usize,

    /// Per-tenant database pool size (connections).
    #[serde(default = "default_per_tenant_pool_size")]
    pub per_tenant_pool_size: u32,

    /// Shared Postgres URL for tenant databases.
    /// Each tenant gets their own database: tenant_<ref>.
    #[serde(default)]
    pub shared_postgres_url: Option<String>,

    /// Backend target for shared cluster routing.
    #[serde(default = "default_shared_backend_target")]
    pub shared_backend_target: String,

    /// Connection pooler configuration.
    /// When using an external pooler (Supavisor, PgCat, PgBouncer),
    /// configure these settings for compatibility.
    #[serde(default)]
    pub pooler: Option<PoolerConfig>,
}

/// Configuration for external connection pooler (Supavisor, PgCat, PgBouncer).
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolerConfig {
    /// Pooler mode: "transaction" or "session".
    /// Transaction mode is recommended for shared clusters.
    /// Constraints: SET statements are only allowed inside transactions,
    /// LISTEN/NOTIFY must use NATS instead.
    #[serde(default = "default_pooler_mode")]
    pub mode: String,

    /// Pooler URL (overrides per-tenant direct connection when set).
    /// Format: postgres://user:pass@supavisor:5432/tenant_{ref}
    /// Use {ref} placeholder for tenant database name substitution.
    #[serde(default)]
    pub url_template: Option<String>,

    /// Connection timeout for pooler connections in seconds.
    #[serde(default = "default_pooler_connect_timeout")]
    pub connect_timeout_secs: u64,

    /// Maximum connections through the pooler per tenant.
    /// This should be lower than direct pool size to account for multiplexing.
    #[serde(default = "default_pooler_max_connections")]
    pub max_connections_per_tenant: u32,

    /// Enable prepared statement caching.
    /// Set to false for PgBouncer in transaction mode (doesn't support prepared statements).
    /// Supavisor and PgCat support prepared statements in transaction mode.
    #[serde(default = "default_pooler_prepared_statements")]
    pub prepared_statements: bool,
}

#[cfg(feature = "cap-cloud")]
fn default_pooler_mode() -> String {
    "transaction".to_string()
}

#[cfg(feature = "cap-cloud")]
fn default_pooler_connect_timeout() -> u64 {
    10
}

#[cfg(feature = "cap-cloud")]
fn default_pooler_max_connections() -> u32 {
    5
}

#[cfg(feature = "cap-cloud")]
fn default_pooler_prepared_statements() -> bool {
    true
}

#[cfg(feature = "cap-cloud")]
impl Default for PoolerConfig {
    fn default() -> Self {
        Self {
            mode: default_pooler_mode(),
            url_template: None,
            connect_timeout_secs: default_pooler_connect_timeout(),
            max_connections_per_tenant: default_pooler_max_connections(),
            prepared_statements: default_pooler_prepared_statements(),
        }
    }
}

#[cfg(feature = "cap-cloud")]
impl Default for SharedPoolConfig {
    fn default() -> Self {
        Self {
            max_active_tenants: default_max_active_tenants(),
            idle_timeout_secs: default_idle_timeout_secs(),
            cold_load_concurrency: default_cold_load_concurrency(),
            per_tenant_pool_size: default_per_tenant_pool_size(),
            shared_postgres_url: None,
            shared_backend_target: default_shared_backend_target(),
            pooler: None,
        }
    }
}

/// Realtime backend configuration (Phase 4).
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeConfig {
    /// Backend type: "in_process" or "nats".
    #[serde(default = "default_realtime_backend")]
    pub backend: String,

    /// NATS configuration (when backend = "nats").
    #[serde(default)]
    pub nats: Option<NatsConfig>,
}

#[cfg(feature = "cap-cloud")]
impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            backend: default_realtime_backend(),
            nats: None,
        }
    }
}

/// PubSub backend configuration (Phase 4).
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubConfig {
    /// Backend type: "in_process" or "nats".
    #[serde(default = "default_pubsub_backend")]
    pub backend: String,

    /// NATS configuration (when backend = "nats").
    #[serde(default)]
    pub nats: Option<NatsConfig>,
}

#[cfg(feature = "cap-cloud")]
impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            backend: default_pubsub_backend(),
            nats: None,
        }
    }
}

/// NATS connection configuration.
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    /// NATS server URLs.
    pub servers: Vec<String>,

    /// Path to NATS credentials file.
    #[serde(default)]
    pub credentials_file: Option<String>,

    /// Connection name (for debugging).
    #[serde(default = "default_nats_connection_name")]
    pub connection_name: String,

    /// Reconnect buffer size in bytes.
    #[serde(default = "default_nats_reconnect_buffer")]
    pub reconnect_buffer_size: usize,

    /// Maximum reconnect attempts (0 = unlimited).
    #[serde(default)]
    pub max_reconnects: Option<usize>,
}

#[cfg(feature = "cap-cloud")]
impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            servers: vec!["nats://localhost:4222".to_string()],
            credentials_file: None,
            connection_name: default_nats_connection_name(),
            reconnect_buffer_size: default_nats_reconnect_buffer(),
            max_reconnects: None,
        }
    }
}

/// Per-tier quota configuration.
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotasConfig {
    /// Free tier quotas.
    #[serde(default)]
    pub free: TierQuotas,

    /// Dedicated tier quotas (None = unlimited).
    #[serde(default)]
    pub dedicated: Option<TierQuotas>,
}

#[cfg(feature = "cap-cloud")]
impl Default for QuotasConfig {
    fn default() -> Self {
        Self {
            free: TierQuotas::default(),
            dedicated: None,
        }
    }
}

/// Quota limits for a tier.
#[cfg(feature = "cap-cloud")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierQuotas {
    /// Requests per minute.
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,

    /// Maximum concurrent function invocations.
    #[serde(default = "default_concurrent_functions")]
    pub concurrent_functions: u32,

    /// Maximum database connections per tenant.
    #[serde(default = "default_db_connections")]
    pub db_connections: u32,

    /// Storage quota in GB.
    #[serde(default = "default_storage_gb")]
    pub storage_gb: u32,

    /// Bandwidth quota in GB per month.
    #[serde(default = "default_bandwidth_gb_per_month")]
    pub bandwidth_gb_per_month: u32,
}

#[cfg(feature = "cap-cloud")]
impl Default for TierQuotas {
    fn default() -> Self {
        Self {
            requests_per_minute: default_requests_per_minute(),
            concurrent_functions: default_concurrent_functions(),
            db_connections: default_db_connections(),
            storage_gb: default_storage_gb(),
            bandwidth_gb_per_month: default_bandwidth_gb_per_month(),
        }
    }
}

// =============================================================================
// Vault config slice
// =============================================================================

/// Vault configuration for secrets management.
///
/// If omitted, uses embedded (file-based) vault with a master key from
/// the `REACTOR_VAULT_MASTER_KEY` environment variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfigSlice {
    /// Vault backend: "embedded" or "openbao".
    #[serde(default = "default_vault_backend")]
    pub backend: String,

    // Embedded vault settings
    /// Path for embedded vault storage.
    #[serde(default = "default_vault_path")]
    pub path: PathBuf,

    /// Master key for embedded vault.
    ///
    /// Can be:
    /// - Raw hex-encoded 32-byte key
    /// - `env:VAR_NAME` to read from environment variable
    #[serde(default)]
    pub master_key: Option<String>,

    // OpenBao settings
    /// OpenBao server address.
    #[serde(default)]
    pub address: Option<String>,

    /// OpenBao namespace.
    #[serde(default)]
    pub namespace: Option<String>,

    /// OpenBao KV mount path.
    #[serde(default = "default_vault_kv_mount")]
    pub kv_mount: String,

    /// OpenBao transit mount path.
    #[serde(default = "default_vault_transit_mount")]
    pub transit_mount: String,

    /// Authentication method: "token" or "approle".
    #[serde(default = "default_vault_auth_method")]
    pub auth_method: String,

    /// OpenBao token (for token auth).
    #[serde(default)]
    pub token: Option<String>,

    /// AppRole role ID (for approle auth).
    #[serde(default)]
    pub role_id: Option<String>,

    /// AppRole secret ID file path (for approle auth).
    #[serde(default)]
    pub secret_id_file: Option<PathBuf>,

    /// AppRole mount path.
    #[serde(default = "default_vault_approle_mount")]
    pub approle_mount: String,

    /// Path to CA cert for TLS verification.
    #[serde(default)]
    pub ca_cert: Option<PathBuf>,

    /// Secret cache TTL in seconds.
    #[serde(default = "default_vault_cache_ttl")]
    pub cache_ttl_secs: u64,
}

impl Default for VaultConfigSlice {
    fn default() -> Self {
        Self {
            backend: default_vault_backend(),
            path: default_vault_path(),
            master_key: None,
            address: None,
            namespace: None,
            kv_mount: default_vault_kv_mount(),
            transit_mount: default_vault_transit_mount(),
            auth_method: default_vault_auth_method(),
            token: None,
            role_id: None,
            secret_id_file: None,
            approle_mount: default_vault_approle_mount(),
            ca_cert: None,
            cache_ttl_secs: default_vault_cache_ttl(),
        }
    }
}

// =============================================================================
// Default value functions
// =============================================================================

fn default_bind() -> SocketAddr {
    "0.0.0.0:8000".parse().unwrap()
}

fn default_request_timeout() -> u64 {
    30
}

fn default_pool_max() -> u32 {
    20
}

fn default_acquire_timeout() -> u64 {
    5
}

fn default_filter() -> String {
    "info".to_string()
}

fn default_fmt() -> String {
    "json".to_string()
}

// Vault defaults
fn default_vault_backend() -> String {
    "embedded".to_string()
}

fn default_vault_path() -> PathBuf {
    PathBuf::from(".reactor/vault")
}

fn default_vault_kv_mount() -> String {
    "secret".to_string()
}

fn default_vault_transit_mount() -> String {
    "transit".to_string()
}

fn default_vault_auth_method() -> String {
    "token".to_string()
}

fn default_vault_approle_mount() -> String {
    "approle".to_string()
}

fn default_vault_cache_ttl() -> u64 {
    300 // 5 minutes
}

// Auth defaults
#[cfg(feature = "cap-auth")]
fn default_jwt_issuer() -> String {
    "reactor-auth".to_string()
}

#[cfg(feature = "cap-auth")]
fn default_jwt_audience() -> String {
    "reactor".to_string()
}

#[cfg(feature = "cap-auth")]
fn default_access_ttl() -> u64 {
    3600 // 1 hour
}

#[cfg(feature = "cap-auth")]
fn default_refresh_ttl() -> u64 {
    2_592_000 // 30 days
}

#[cfg(feature = "cap-auth")]
fn default_smtp_port() -> u16 {
    587
}

#[cfg(feature = "cap-auth")]
fn default_smtp_tls() -> String {
    "starttls".to_string()
}

// Data defaults
#[cfg(feature = "cap-data")]
fn default_run_migrations() -> bool {
    true
}

#[cfg(feature = "cap-data")]
fn default_user_schema() -> String {
    "public".to_string()
}

#[cfg(feature = "cap-data")]
fn default_max_embed_depth() -> u8 {
    5
}

#[cfg(feature = "cap-data")]
fn default_max_limit() -> u32 {
    1000
}

#[cfg(feature = "cap-data")]
fn default_default_limit() -> u32 {
    100
}

// Storage defaults
#[cfg(feature = "cap-storage")]
fn default_storage_backend() -> String {
    "fs".to_string()
}

#[cfg(feature = "cap-storage")]
fn default_signed_url_expiry() -> u64 {
    3600 // 1 hour
}

#[cfg(feature = "cap-storage")]
fn default_max_upload_size() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

// Functions defaults
#[cfg(feature = "cap-functions")]
fn default_functions_workdir() -> String {
    ".reactor/functions".to_string()
}

#[cfg(feature = "cap-functions")]
fn default_runtimes() -> Vec<String> {
    vec!["wasm".to_string()]
}

#[cfg(feature = "cap-functions")]
fn default_invoke_timeout_ms() -> u64 {
    30_000 // 30 seconds
}

#[cfg(feature = "cap-functions")]
fn default_invoke_max_timeout_ms() -> u64 {
    300_000 // 5 minutes
}

#[cfg(feature = "cap-functions")]
fn default_bundle_max_bytes() -> u64 {
    50 * 1024 * 1024 // 50 MiB
}

#[cfg(feature = "cap-functions")]
fn default_bun_bin() -> String {
    "bun".to_string()
}

#[cfg(feature = "cap-functions")]
fn default_bun_idle_ttl_secs() -> u64 {
    300 // 5 minutes
}

#[cfg(feature = "cap-functions")]
fn default_bun_max_instances() -> u32 {
    8
}

// Jobs defaults
#[cfg(feature = "cap-jobs")]
fn default_worker_count() -> usize {
    4
}

#[cfg(feature = "cap-jobs")]
fn default_scheduler_interval_ms() -> u64 {
    1000
}

#[cfg(feature = "cap-jobs")]
fn default_job_timeout_ms() -> u64 {
    600_000 // 10 minutes
}

#[cfg(feature = "cap-jobs")]
fn default_job_max_timeout_ms() -> u64 {
    3_600_000 // 1 hour
}

#[cfg(feature = "cap-jobs")]
fn default_max_org_concurrent_runs() -> u32 {
    50
}

#[cfg(feature = "cap-jobs")]
fn default_max_payload_bytes() -> u64 {
    1_048_576 // 1 MiB
}

// Connect defaults
#[cfg(feature = "cap-connect")]
fn default_connect_jobs_url() -> String {
    "http://localhost:8000/jobs/v1".to_string()
}

#[cfg(feature = "cap-connect")]
fn default_connect_data_url() -> String {
    "http://localhost:8000/data/v1".to_string()
}

#[cfg(feature = "cap-connect")]
fn default_connect_storage_url() -> String {
    "http://localhost:8000/storage/v1".to_string()
}

#[cfg(feature = "cap-connect")]
fn default_connect_refresh_interval_secs() -> u64 {
    300 // 5 minutes
}

#[cfg(feature = "cap-connect")]
fn default_connect_sandbox_ttl_secs() -> u64 {
    86400 // 24 hours
}

#[cfg(feature = "cap-connect")]
fn default_connect_max_concurrent_syncs() -> u32 {
    10
}

// Sites defaults
#[cfg(feature = "cap-sites")]
fn default_sites_workdir() -> String {
    ".reactor/sites".to_string()
}

#[cfg(feature = "cap-sites")]
fn default_sites_bundle_max_bytes() -> u64 {
    500 * 1024 * 1024 // 500 MiB
}

#[cfg(feature = "cap-sites")]
fn default_isr_enabled() -> bool {
    true
}

#[cfg(feature = "cap-sites")]
fn default_isr_revalidate_secs() -> u64 {
    60 // 1 minute
}

#[cfg(feature = "cap-sites")]
fn default_sites_preview_subdomain() -> String {
    "preview".to_string()
}

#[cfg(feature = "cap-sites")]
fn default_sites_default_org() -> String {
    "reactor".to_string()
}

// Cloud defaults
#[cfg(feature = "cap-cloud")]
fn default_cloud_base_domain() -> String {
    "reactor.local".to_string()
}

#[cfg(feature = "cap-cloud")]
fn default_cloud_backend_target() -> String {
    "localhost:8000".to_string()
}

#[cfg(feature = "cap-cloud")]
fn default_cloud_tls_mode() -> String {
    "none".to_string()
}

#[cfg(feature = "cap-cloud")]
fn default_cloud_tls_mode_opt() -> Option<String> {
    Some("none".to_string())
}

#[cfg(feature = "cap-cloud")]
fn default_tenant_cache_ttl() -> u64 {
    300 // 5 minutes
}

// Shared pool defaults (Phase 4)
#[cfg(feature = "cap-cloud")]
fn default_max_active_tenants() -> usize {
    5000
}

#[cfg(feature = "cap-cloud")]
fn default_idle_timeout_secs() -> u64 {
    600 // 10 minutes
}

#[cfg(feature = "cap-cloud")]
fn default_cold_load_concurrency() -> usize {
    16
}

#[cfg(feature = "cap-cloud")]
fn default_per_tenant_pool_size() -> u32 {
    5
}

#[cfg(feature = "cap-cloud")]
fn default_shared_backend_target() -> String {
    "rc-shared-1-server.internal:8000".to_string()
}

// Realtime/PubSub defaults
#[cfg(feature = "cap-cloud")]
fn default_realtime_backend() -> String {
    "in_process".to_string()
}

#[cfg(feature = "cap-cloud")]
fn default_pubsub_backend() -> String {
    "in_process".to_string()
}

// NATS defaults
#[cfg(feature = "cap-cloud")]
fn default_nats_connection_name() -> String {
    "reactor-server".to_string()
}

#[cfg(feature = "cap-cloud")]
fn default_nats_reconnect_buffer() -> usize {
    8 * 1024 * 1024 // 8 MiB
}

// Quota defaults (free tier)
#[cfg(feature = "cap-cloud")]
fn default_requests_per_minute() -> u32 {
    1000
}

#[cfg(feature = "cap-cloud")]
fn default_concurrent_functions() -> u32 {
    10
}

#[cfg(feature = "cap-cloud")]
fn default_db_connections() -> u32 {
    5
}

#[cfg(feature = "cap-cloud")]
fn default_storage_gb() -> u32 {
    1
}

#[cfg(feature = "cap-cloud")]
fn default_bandwidth_gb_per_month() -> u32 {
    5
}

// =============================================================================
// Implementation
// =============================================================================

impl ReactorConfig {
    /// Load configuration from Reactor.toml and environment.
    ///
    /// Priority (highest to lowest):
    /// 1. Environment variables (REACTOR_*)
    /// 2. Reactor.toml file
    ///
    /// Nested values use `__` separator in env vars, e.g.:
    /// - `REACTOR_SERVER__BIND` -> `server.bind`
    /// - `REACTOR_AUTH__DATA_KEY` -> `auth.data_key`
    pub fn load() -> Result<Self, figment::Error> {
        use figment::{
            providers::{Env, Format, Toml},
            Figment,
        };

        Figment::new()
            .merge(Toml::file("Reactor.toml"))
            .merge(
                Env::prefixed("REACTOR_")
                    .map(|key| key.as_str().replace("__", ".").into()),
            )
            .extract()
    }

    /// Load configuration from a specific file path.
    pub fn load_from(path: &str) -> Result<Self, figment::Error> {
        use figment::{
            providers::{Env, Format, Toml},
            Figment,
        };

        Figment::new()
            .merge(Toml::file(path))
            .merge(
                Env::prefixed("REACTOR_")
                    .map(|key| key.as_str().replace("__", ".").into()),
            )
            .extract()
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), crate::error::ServerError> {
        use crate::error::ServerError;

        // Admin token is required
        if self.admin.token.is_empty() {
            return Err(ServerError::Config("admin.token is required".to_string()));
        }

        // Database URL is required
        if self.database.url.is_empty() {
            return Err(ServerError::Config("database.url is required".to_string()));
        }

        // Auth is required (no anonymous mode)
        #[cfg(feature = "cap-auth")]
        if self.auth.is_none() {
            return Err(ServerError::Config(
                "[auth] section is required (no anonymous mode)".to_string(),
            ));
        }

        // Validate auth config if present
        #[cfg(feature = "cap-auth")]
        if let Some(ref auth) = self.auth {
            if auth.data_key.is_empty() {
                return Err(ServerError::Config("auth.data_key is required".to_string()));
            }
            if auth.public_url.is_empty() {
                return Err(ServerError::Config("auth.public_url is required".to_string()));
            }
        }

        // Validate storage config if present
        #[cfg(feature = "cap-storage")]
        if let Some(ref storage) = self.storage {
            if storage.signing_secret.is_empty() {
                return Err(ServerError::Config(
                    "storage.signing_secret is required".to_string(),
                ));
            }
            match storage.backend.as_str() {
                "fs" => {
                    if storage.fs_base_path.is_none() {
                        return Err(ServerError::Config(
                            "storage.fs_base_path required for fs backend".to_string(),
                        ));
                    }
                }
                "s3" => {
                    if storage.s3_bucket.is_none() {
                        return Err(ServerError::Config(
                            "storage.s3_bucket required for s3 backend".to_string(),
                        ));
                    }
                }
                other => {
                    return Err(ServerError::Config(format!(
                        "unknown storage backend: {} (expected 'fs' or 's3')",
                        other
                    )));
                }
            }
        }

        // Validate functions config if present
        #[cfg(feature = "cap-functions")]
        if let Some(ref functions) = self.functions {
            if functions.data_key.is_empty() {
                return Err(ServerError::Config(
                    "functions.data_key is required".to_string(),
                ));
            }
        }

        // Validate jobs config if present
        #[cfg(feature = "cap-jobs")]
        if let Some(ref jobs) = self.jobs {
            if jobs.webhook_secret.is_empty() {
                return Err(ServerError::Config(
                    "jobs.webhook_secret is required".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Check if auth capability is enabled.
    #[cfg(feature = "cap-auth")]
    pub fn has_auth(&self) -> bool {
        self.auth.is_some()
    }

    /// Check if data capability is enabled.
    #[cfg(feature = "cap-data")]
    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }

    /// Check if storage capability is enabled.
    #[cfg(feature = "cap-storage")]
    pub fn has_storage(&self) -> bool {
        self.storage.is_some()
    }

    /// Check if functions capability is enabled.
    #[cfg(feature = "cap-functions")]
    pub fn has_functions(&self) -> bool {
        self.functions.is_some()
    }

    /// Check if jobs capability is enabled.
    #[cfg(feature = "cap-jobs")]
    pub fn has_jobs(&self) -> bool {
        self.jobs.is_some()
    }

    /// Check if connect capability is enabled.
    #[cfg(feature = "cap-connect")]
    pub fn has_connect(&self) -> bool {
        self.connect.is_some()
    }

    /// Check if sites capability is enabled.
    #[cfg(feature = "cap-sites")]
    pub fn has_sites(&self) -> bool {
        self.sites.is_some()
    }

    /// Check if cloud capability is enabled.
    #[cfg(feature = "cap-cloud")]
    pub fn has_cloud(&self) -> bool {
        self.cloud.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_server_config() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.bind.to_string(), "0.0.0.0:8000");
        assert_eq!(cfg.request_timeout_secs, 30);
    }

    #[test]
    fn test_default_tracing_config() {
        let cfg = TracingConfig::default();
        assert_eq!(cfg.filter, "info");
        assert_eq!(cfg.fmt, "json");
    }

    #[test]
    fn test_project_config_defaults() {
        let cfg = ProjectConfig::default();
        
        // Empty ID should give nil ProjectId
        assert!(cfg.project_id().is_nil());
        
        // Empty name should fallback to "reactor"
        assert_eq!(cfg.project_name(), "reactor");
        
        // Empty env should default to Production
        assert_eq!(cfg.tenant_env(), reactor_core::TenantEnv::Production);
    }

    #[test]
    fn test_project_config_with_id() {
        let cfg = ProjectConfig {
            id: "019213f5-0000-7000-8000-000000000001".to_string(),
            name: "my-project".to_string(),
            ref_: None,
            env: Some("dev".to_string()),
        };
        
        // ID should parse correctly
        let id = cfg.project_id();
        assert!(!id.is_nil());
        assert_eq!(id.to_string(), "019213f5-0000-7000-8000-000000000001");
        
        // Ref should be derived from ID
        let ref_ = cfg.project_ref();
        assert_eq!(ref_.as_str().len(), 20);
        
        // Name should be used
        assert_eq!(cfg.project_name(), "my-project");
        
        // Env should parse
        assert_eq!(cfg.tenant_env(), reactor_core::TenantEnv::Dev);
    }

    #[test]
    fn test_project_ref_deterministic() {
        let cfg = ProjectConfig {
            id: "019213f5-0000-7000-8000-000000000001".to_string(),
            name: String::new(),
            ref_: None,
            env: None,
        };
        
        // Same config should always produce same ref
        let ref1 = cfg.project_ref();
        let ref2 = cfg.project_ref();
        assert_eq!(ref1, ref2);
    }

    #[test]
    fn test_project_ref_override() {
        let cfg = ProjectConfig {
            id: "019213f5-0000-7000-8000-000000000001".to_string(),
            name: String::new(),
            ref_: Some("myprojectref12345678".to_string()),
            env: None,
        };
        
        // Explicit ref should be used
        assert_eq!(cfg.project_ref().as_str(), "myprojectref12345678");
    }

    #[test]
    fn test_to_tenant_ctx() {
        let cfg = ProjectConfig {
            id: "019213f5-0000-7000-8000-000000000001".to_string(),
            name: "test-project".to_string(),
            ref_: None,
            env: Some("production".to_string()),
        };
        
        let ctx = cfg.to_tenant_ctx();
        
        assert_eq!(ctx.project_id().to_string(), "019213f5-0000-7000-8000-000000000001");
        assert_eq!(ctx.project_name(), "test-project");
        assert_eq!(ctx.env(), reactor_core::TenantEnv::Production);
        assert_eq!(ctx.project_ref().as_str().len(), 20);
    }
}
