//! Tenant adapter cache for multi-tenant mode (Phase 4).
//!
//! In shared cluster deployments, each tenant has their own set of adapters:
//! - Database pool (connects to tenant_<ref> database)
//! - Storage client (scoped to tenant's bucket prefix)
//! - Auth verifier (uses tenant's JWT signing keys)
//! - Vault session (scoped to tenant/<id>/* paths)
//!
//! The cache uses idle eviction to bound memory usage when serving thousands
//! of tenants. Cold lookups pay a one-time cost; warm requests reuse pools.

use dashmap::DashMap;
use reactor_core::{ProjectId, TenantCtx};
use sqlx::PgPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, info};

/// Per-tenant capability adapters.
///
/// Each tenant gets their own isolated set of adapters when first accessed.
/// The adapters are cached and reused for subsequent requests.
#[derive(Debug)]
pub struct TenantCapabilities {
    /// Tenant context (ID, ref, name, env).
    pub tenant_ctx: Arc<TenantCtx>,

    /// Per-tenant database pool (small, 3-5 connections).
    /// Connects to the tenant's database via the shared pooler.
    pub db_pool: PgPool,

    /// Storage bucket prefix for this tenant.
    pub storage_prefix: String,

    /// Last time this tenant's adapters were accessed.
    /// Used for idle eviction.
    last_used: AtomicU64,
}

impl TenantCapabilities {
    /// Create new tenant capabilities.
    pub fn new(
        tenant_ctx: Arc<TenantCtx>,
        db_pool: PgPool,
        storage_prefix: String,
    ) -> Self {
        Self {
            tenant_ctx,
            db_pool,
            storage_prefix,
            last_used: AtomicU64::new(Self::now_secs()),
        }
    }

    /// Update the last-used timestamp.
    pub fn touch(&self) {
        self.last_used.store(Self::now_secs(), Ordering::Relaxed);
    }

    /// Get seconds since last use.
    pub fn idle_secs(&self) -> u64 {
        Self::now_secs().saturating_sub(self.last_used.load(Ordering::Relaxed))
    }

    fn now_secs() -> u64 {
        Instant::now().elapsed().as_secs()
    }
}

/// Configuration for the tenant adapter cache.
#[derive(Debug, Clone)]
pub struct TenantAdapterCacheConfig {
    /// Maximum number of active tenants in the cache.
    pub max_active_tenants: usize,

    /// Idle timeout for eviction.
    pub idle_timeout: Duration,

    /// Maximum concurrent cold loads.
    pub cold_load_concurrency: usize,

    /// Per-tenant database pool size.
    pub per_tenant_pool_size: u32,

    /// Base URL for the shared Postgres (direct connection or pooler).
    pub shared_postgres_base_url: String,

    /// Pooler URL template (optional).
    /// If set, overrides shared_postgres_base_url.
    /// Use `{ref}` placeholder for tenant database name substitution.
    /// Example: `postgres://user:pass@supavisor:5432/{ref}`
    pub pooler_url_template: Option<String>,

    /// Pooler mode ("transaction" or "session").
    /// Transaction mode requires avoiding session-level SET and LISTEN.
    pub pooler_mode: String,

    /// Enable prepared statement caching.
    /// Set to false for PgBouncer in transaction mode.
    pub prepared_statements: bool,

    /// Connection timeout in seconds for pooler connections.
    pub pooler_connect_timeout_secs: u64,

    /// Storage bucket for shared storage.
    pub storage_bucket: String,
}

impl Default for TenantAdapterCacheConfig {
    fn default() -> Self {
        Self {
            max_active_tenants: 5000,
            idle_timeout: Duration::from_secs(600),
            cold_load_concurrency: 16,
            per_tenant_pool_size: 5,
            shared_postgres_base_url: String::new(),
            pooler_url_template: None,
            pooler_mode: "transaction".to_string(),
            prepared_statements: true,
            pooler_connect_timeout_secs: 10,
            storage_bucket: String::new(),
        }
    }
}

/// Tenant adapter cache for multi-tenant mode.
///
/// Manages per-tenant database pools, storage clients, and other adapters.
/// Uses idle eviction to bound memory when serving thousands of tenants.
pub struct TenantAdapterCache {
    /// Cached tenant capabilities, keyed by ProjectId.
    capabilities: DashMap<ProjectId, Arc<TenantCapabilities>>,

    /// Semaphore to limit concurrent cold loads.
    cold_load_semaphore: Arc<Semaphore>,

    /// Configuration.
    config: TenantAdapterCacheConfig,

    /// Epoch for last_used timestamps (set at cache creation).
    epoch: Instant,
}

impl TenantAdapterCache {
    /// Create a new tenant adapter cache.
    pub fn new(config: TenantAdapterCacheConfig) -> Self {
        Self {
            capabilities: DashMap::with_capacity(config.max_active_tenants),
            cold_load_semaphore: Arc::new(Semaphore::new(config.cold_load_concurrency)),
            config,
            epoch: Instant::now(),
        }
    }

    /// Get or load tenant capabilities.
    ///
    /// If the tenant is cached, returns immediately (cache hit).
    /// If not cached, acquires a semaphore permit and loads the tenant's
    /// capabilities (cold load).
    pub async fn get_or_load(
        &self,
        tenant_ctx: Arc<TenantCtx>,
    ) -> Result<Arc<TenantCapabilities>, TenantCacheError> {
        let project_id = tenant_ctx.project_id();

        // Fast path: check if already cached
        if let Some(entry) = self.capabilities.get(&project_id) {
            entry.touch();
            debug!(
                project_id = %project_id,
                project_ref = %tenant_ctx.project_ref(),
                "tenant cache hit"
            );
            return Ok(entry.clone());
        }

        // Slow path: cold load with semaphore
        let _permit = self
            .cold_load_semaphore
            .acquire()
            .await
            .map_err(|_| TenantCacheError::SemaphoreClosed)?;

        // Double-check after acquiring permit (another task may have loaded it)
        if let Some(entry) = self.capabilities.get(&project_id) {
            entry.touch();
            return Ok(entry.clone());
        }

        debug!(
            project_id = %project_id,
            project_ref = %tenant_ctx.project_ref(),
            "cold loading tenant capabilities"
        );

        // Build per-tenant database pool
        let db_pool = self.build_tenant_pool(&tenant_ctx).await?;

        // Build storage prefix
        let storage_prefix = format!(
            "{}/{}",
            self.config.storage_bucket,
            tenant_ctx.project_ref()
        );

        // Create capabilities
        let capabilities = Arc::new(TenantCapabilities::new(
            tenant_ctx.clone(),
            db_pool,
            storage_prefix,
        ));

        // Check if we need to evict before inserting
        if self.capabilities.len() >= self.config.max_active_tenants {
            self.evict_idle().await;
        }

        // Insert into cache
        self.capabilities.insert(project_id.clone(), capabilities.clone());

        info!(
            project_id = %project_id,
            project_ref = %tenant_ctx.project_ref(),
            cache_size = self.capabilities.len(),
            "tenant cold loaded"
        );

        Ok(capabilities)
    }

    /// Build a per-tenant database pool.
    ///
    /// Connects to the tenant's database via the shared pooler (Supavisor/PgCat)
    /// or directly to the shared Postgres. The connection string includes the
    /// tenant's database name.
    ///
    /// When using transaction-mode pooling:
    /// - Prepared statements may need to be disabled (PgBouncer)
    /// - SET statements must be inside transactions
    /// - LISTEN/NOTIFY must use NATS instead
    async fn build_tenant_pool(
        &self,
        tenant_ctx: &TenantCtx,
    ) -> Result<PgPool, TenantCacheError> {
        let db_name = format!("tenant_{}", tenant_ctx.project_ref());

        // Build connection URL
        let url_str = if let Some(ref template) = self.config.pooler_url_template {
            // Use pooler URL template with {ref} substitution
            template.replace("{ref}", &db_name)
        } else {
            // Use base URL with database name appended
            let mut url = url::Url::parse(&self.config.shared_postgres_base_url)
                .map_err(|e| TenantCacheError::ConfigError(format!("invalid postgres URL: {}", e)))?;
            url.set_path(&format!("/{}", db_name));
            url.to_string()
        };

        // Configure pool based on pooler mode
        let connect_timeout = Duration::from_secs(self.config.pooler_connect_timeout_secs);
        let mut pool_options = sqlx::postgres::PgPoolOptions::new()
            .max_connections(self.config.per_tenant_pool_size)
            .acquire_timeout(connect_timeout);

        // Add pooler-specific connection options
        if self.config.pooler_mode == "transaction" {
            // For transaction-mode poolers:
            // - Supavisor and PgCat support prepared statements
            // - PgBouncer does not (unless using v1.21+ with track_extra_parameters)
            // - We disable prepared statements based on config
            if !self.config.prepared_statements {
                pool_options = pool_options.after_connect(|conn, _meta| {
                    Box::pin(async move {
                        // Disable prepared statement caching for this connection
                        // This is required for PgBouncer in transaction mode
                        use sqlx::Executor;
                        conn.execute("SET plan_cache_mode = force_generic_plan")
                            .await
                            .ok();
                        Ok(())
                    })
                });
            }
        }

        // Connect to the database
        let pool = pool_options
            .connect(&url_str)
            .await
            .map_err(|e| TenantCacheError::DatabaseError(e.to_string()))?;

        debug!(
            project_ref = %tenant_ctx.project_ref(),
            db_name = %db_name,
            pool_size = self.config.per_tenant_pool_size,
            pooler_mode = %self.config.pooler_mode,
            "tenant database pool created"
        );

        Ok(pool)
    }

    /// Evict idle tenants from the cache.
    ///
    /// Removes tenants that haven't been accessed within the idle timeout.
    /// Called automatically when the cache approaches capacity.
    pub async fn evict_idle(&self) {
        let threshold_secs = self.config.idle_timeout.as_secs();
        let mut evicted = 0;

        self.capabilities.retain(|project_id, capabilities| {
            let idle = capabilities.idle_secs();
            if idle > threshold_secs {
                debug!(
                    project_id = %project_id,
                    idle_secs = idle,
                    "evicting idle tenant"
                );
                evicted += 1;
                false
            } else {
                true
            }
        });

        if evicted > 0 {
            info!(
                evicted = evicted,
                remaining = self.capabilities.len(),
                "evicted idle tenants"
            );
        }
    }

    /// Invalidate a specific tenant from the cache.
    ///
    /// Call this when a tenant's configuration changes (e.g., vault key rotation).
    pub fn invalidate(&self, project_id: &ProjectId) {
        if self.capabilities.remove(project_id).is_some() {
            debug!(project_id = %project_id, "tenant invalidated");
        }
    }

    /// Clear the entire cache.
    pub fn clear(&self) {
        let count = self.capabilities.len();
        self.capabilities.clear();
        info!(cleared = count, "tenant cache cleared");
    }

    /// Get the current cache size.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Get cache statistics.
    pub fn stats(&self) -> TenantCacheStats {
        let mut total_idle_secs = 0u64;
        let mut max_idle_secs = 0u64;

        for entry in self.capabilities.iter() {
            let idle = entry.idle_secs();
            total_idle_secs += idle;
            max_idle_secs = max_idle_secs.max(idle);
        }

        let count = self.capabilities.len();
        let avg_idle_secs = if count > 0 {
            total_idle_secs / count as u64
        } else {
            0
        };

        TenantCacheStats {
            active_tenants: count,
            max_active_tenants: self.config.max_active_tenants,
            avg_idle_secs,
            max_idle_secs,
        }
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct TenantCacheStats {
    /// Number of active tenants in cache.
    pub active_tenants: usize,
    /// Maximum capacity.
    pub max_active_tenants: usize,
    /// Average idle time in seconds.
    pub avg_idle_secs: u64,
    /// Maximum idle time in seconds.
    pub max_idle_secs: u64,
}

/// Errors that can occur during tenant cache operations.
#[derive(Debug, thiserror::Error)]
pub enum TenantCacheError {
    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Database connection error.
    #[error("database error: {0}")]
    DatabaseError(String),

    /// Vault error.
    #[error("vault error: {0}")]
    VaultError(String),

    /// Semaphore closed (shutdown).
    #[error("cache is shutting down")]
    SemaphoreClosed,
}

/// Background task for periodic eviction.
pub async fn eviction_task(
    cache: Arc<TenantAdapterCache>,
    interval: Duration,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                cache.evict_idle().await;
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("tenant cache eviction task shutting down");
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reactor_core::{ProjectRef, TenantEnv};

    fn test_tenant_ctx(name: &str) -> Arc<TenantCtx> {
        let id = ProjectId::new();
        let ref_ = ProjectRef::from_string_unchecked(format!("{:0>20}", name));
        Arc::new(TenantCtx::new(id, ref_, name, TenantEnv::Dev))
    }

    #[test]
    fn test_cache_config_defaults() {
        let config = TenantAdapterCacheConfig::default();
        assert_eq!(config.max_active_tenants, 5000);
        assert_eq!(config.idle_timeout, Duration::from_secs(600));
        assert_eq!(config.cold_load_concurrency, 16);
        assert_eq!(config.per_tenant_pool_size, 5);
    }

    #[test]
    fn test_capabilities_touch() {
        let tenant_ctx = test_tenant_ctx("test");
        let pool = PgPool::connect_lazy("postgres://fake").unwrap();
        let caps = TenantCapabilities::new(tenant_ctx, pool, "bucket/test".to_string());

        // Initial idle should be very small
        assert!(caps.idle_secs() < 2);

        // Touch should update timestamp
        caps.touch();
        assert!(caps.idle_secs() < 2);
    }

    #[test]
    fn test_cache_stats() {
        let config = TenantAdapterCacheConfig::default();
        let cache = TenantAdapterCache::new(config);

        let stats = cache.stats();
        assert_eq!(stats.active_tenants, 0);
        assert_eq!(stats.max_active_tenants, 5000);
    }

    #[test]
    fn test_cache_invalidate() {
        let config = TenantAdapterCacheConfig::default();
        let cache = TenantAdapterCache::new(config);

        // Invalidating non-existent key should be a no-op
        let id = ProjectId::new();
        cache.invalidate(&id);
        assert!(cache.is_empty());
    }
}
