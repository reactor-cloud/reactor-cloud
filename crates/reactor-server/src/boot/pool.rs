//! Shared resources for the unified server.
//!
//! `SharedResources` holds the connection pool, HTTP client, cache backend,
//! leader election, and clock that are shared across all capabilities.

use crate::boot::vault::build_vault;
use crate::config::{DatabaseConfig, ReactorConfig};
use crate::error::ServerError;
use reactor_cache::{CacheBackend, LeaderElect, PostgresBackend};
use reactor_core::primitives::vault::Vault;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// A clock trait for time operations (supports testing with mock clocks).
pub trait Clock: Send + Sync + 'static {
    /// Get the current UTC timestamp.
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}

/// System clock implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}

/// Shared resources used by all capabilities.
#[derive(Clone)]
pub struct SharedResources {
    /// PostgreSQL connection pool (shared by all capabilities).
    pub pg: PgPool,

    /// HTTP client for outbound requests.
    pub http: reqwest::Client,

    /// Cache backend (queue + KV operations).
    pub cache: Arc<dyn CacheBackend>,

    /// Leader election for background task coordination.
    pub leader: Arc<dyn LeaderElect>,

    /// Vault for secrets management.
    pub vault: Arc<dyn Vault>,

    /// Clock for time operations.
    pub clock: Arc<dyn Clock>,

    /// Shutdown receiver for coordinating graceful shutdown.
    pub shutdown: watch::Receiver<bool>,
}

impl SharedResources {
    /// Build shared resources from configuration.
    ///
    /// This establishes the database connection pool, creates the HTTP client,
    /// initializes the cache backend, leader election, vault, and sets up the
    /// shutdown channel.
    pub async fn build(
        config: &ReactorConfig,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<Self, ServerError> {
        let pg = build_pool(&config.database).await?;
        let http = build_http_client()?;
        let cache = build_cache_backend(pg.clone()).await?;
        let leader = build_leader_elect(pg.clone());
        let vault = build_vault(config).await?;
        let clock = Arc::new(SystemClock);

        Ok(Self {
            pg,
            http,
            cache,
            leader,
            vault,
            clock,
            shutdown: shutdown_rx,
        })
    }

    /// Ping the database to verify connectivity.
    pub async fn ping_db(&self) -> Result<(), ServerError> {
        sqlx::query("SELECT 1")
            .execute(&self.pg)
            .await
            .map_err(|e| ServerError::Database(e))?;
        Ok(())
    }
}

/// Build the PostgreSQL connection pool.
async fn build_pool(config: &DatabaseConfig) -> Result<PgPool, ServerError> {
    let pool = PgPoolOptions::new()
        .max_connections(config.pool_max)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
        .connect(&config.url)
        .await
        .map_err(ServerError::Database)?;

    tracing::info!(
        pool_max = config.pool_max,
        acquire_timeout_secs = config.acquire_timeout_secs,
        "database pool established"
    );

    Ok(pool)
}

/// Build the HTTP client for outbound requests.
fn build_http_client() -> Result<reqwest::Client, ServerError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| ServerError::Boot(format!("failed to create HTTP client: {}", e)))
}

/// Build the cache backend (Postgres-backed at v0).
async fn build_cache_backend(pool: PgPool) -> Result<Arc<dyn CacheBackend>, ServerError> {
    let backend = PostgresBackend::new(pool);

    // Run cache migrations
    backend
        .migrate()
        .await
        .map_err(|e| ServerError::Migration(format!("cache migration failed: {}", e)))?;

    tracing::info!("cache backend initialized (postgres)");

    Ok(Arc::new(backend))
}

/// Build the leader election backend.
///
/// Uses PostgreSQL advisory locks for distributed leader election.
/// In single-node deployments this is effectively a no-op (always leader),
/// but having the abstraction lets us scale to multi-node without code changes.
fn build_leader_elect(pool: PgPool) -> Arc<dyn LeaderElect> {
    tracing::info!("leader election initialized (pg_advisory)");
    reactor_cache::pg_advisory_leader(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_system_clock() {
        let clock = SystemClock;
        let now = clock.now();
        // Should be a reasonable time (after 2020)
        assert!(now.year() >= 2020);
    }
}
