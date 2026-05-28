//! Migration fan-out across capabilities.
//!
//! Runs all capability migrations in topological order:
//! 1. Auth (no deps)
//! 2. Data (depends on auth for RLS context)
//! 3. Storage (depends on auth)
//! 4. Functions (depends on auth, storage for bundles)
//! 5. Jobs (depends on auth, functions)
//! 6. Sites (depends on auth, storage, functions)
//!
//! Each capability's `_sqlx_migrations` tracking table is isolated into its
//! own schema via `SET search_path`, so version numbers do not collide across
//! capabilities. The capability's actual tables use fully-qualified schema
//! names in the migration SQL and are unaffected by the search_path.
//!
//! Note: reactor-cache migrations are run during SharedResources::build().

use crate::config::ReactorConfig;
use crate::error::ServerError;
use sqlx::{Executor, PgPool};

/// Run all capability migrations in the correct order.
///
/// This is idempotent — running twice has no effect if migrations are already applied.
pub async fn run_all(pool: &PgPool, config: &ReactorConfig) -> Result<(), ServerError> {
    tracing::info!("running capability migrations");

    // Pre-create shared extensions in public schema so they're available to all capabilities
    create_shared_extensions(pool).await?;

    // 1. Auth migrations
    #[cfg(feature = "cap-auth")]
    if config.auth.is_some() {
        run_auth_migrations(pool).await?;
    }

    // 2. Data migrations (internal schema only - user migrations handled separately)
    #[cfg(feature = "cap-data")]
    if config.data.is_some() {
        run_data_migrations(pool).await?;
    }

    // 3. Storage migrations
    #[cfg(feature = "cap-storage")]
    if config.storage.is_some() {
        run_storage_migrations(pool).await?;
    }

    // 4. Functions migrations
    #[cfg(feature = "cap-functions")]
    if config.functions.is_some() {
        run_functions_migrations(pool).await?;
    }

    // 5. Jobs migrations
    #[cfg(feature = "cap-jobs")]
    if config.jobs.is_some() {
        run_jobs_migrations(pool).await?;
    }

    // 6. Connect migrations
    #[cfg(feature = "cap-connect")]
    if config.connect.is_some() {
        run_connect_migrations(pool).await?;
    }

    // 7. Sites migrations
    #[cfg(feature = "cap-sites")]
    if config.sites.is_some() {
        run_sites_migrations(pool).await?;
    }

    // 8. Gateway migrations (reactor_gateway schema - must run before cloud)
    #[cfg(feature = "cap-cloud")]
    if config.cloud.is_some() {
        run_gateway_migrations(pool).await?;
    }

    // 9. Cloud control plane migrations (reactor_cloud schema for multi-tenant)
    #[cfg(feature = "cap-cloud")]
    if config.cloud.is_some() {
        run_cloud_migrations(pool).await?;
    }

    tracing::info!("all capability migrations complete");
    Ok(())
}

/// Create shared PostgreSQL extensions in the public schema.
///
/// These extensions (like citext) are used by multiple capabilities and must
/// be created in public before running capability migrations, since each
/// capability's migrations run with an isolated search_path.
async fn create_shared_extensions(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("creating shared extensions in public schema");

    pool.execute("CREATE EXTENSION IF NOT EXISTS citext WITH SCHEMA public")
        .await
        .map_err(|e| ServerError::Migration(format!("create citext extension: {}", e)))?;

    Ok(())
}

/// Acquire a connection with `search_path` set so that the `_sqlx_migrations`
/// tracking table lives in `schema`. The capability's actual tables use
/// fully-qualified names in the migration SQL and are unaffected.
async fn acquire_isolated_conn(
    pool: &PgPool,
    schema: &str,
) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, ServerError> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| ServerError::Migration(format!("acquire conn: {}", e)))?;

    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema))
        .execute(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("create schema {}: {}", schema, e)))?;

    sqlx::query(&format!("SET search_path TO {}, public", schema))
        .execute(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("set search_path {}: {}", schema, e)))?;

    Ok(conn)
}

/// Run auth capability migrations.
#[cfg(feature = "cap-auth")]
async fn run_auth_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running auth migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_auth_mig").await?;
    reactor_auth::migrator()
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("auth: {}", e)))?;

    tracing::info!("auth migrations complete");
    Ok(())
}

/// Run data capability migrations (internal schema only).
///
/// User migrations are handled separately via the deploy pipeline.
#[cfg(feature = "cap-data")]
async fn run_data_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running data migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_data_mig").await?;
    sqlx::migrate!("../reactor-data/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("data: {}", e)))?;

    tracing::info!("data migrations complete");
    Ok(())
}

/// Run storage capability migrations.
#[cfg(feature = "cap-storage")]
async fn run_storage_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running storage migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_storage_mig").await?;
    sqlx::migrate!("../reactor-storage/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("storage: {}", e)))?;

    tracing::info!("storage migrations complete");
    Ok(())
}

/// Run functions capability migrations.
#[cfg(feature = "cap-functions")]
async fn run_functions_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running functions migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_functions_mig").await?;
    sqlx::migrate!("../reactor-functions/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("functions: {}", e)))?;

    tracing::info!("functions migrations complete");
    Ok(())
}

/// Run jobs capability migrations.
#[cfg(feature = "cap-jobs")]
async fn run_jobs_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running jobs migrations");

    // Jobs exposes migrate() on the store (uses raw SQL, no _sqlx_migrations conflict)
    let store = reactor_jobs::PgJobsStore::new(pool.clone());
    store
        .migrate()
        .await
        .map_err(|e| ServerError::Migration(format!("jobs: {}", e)))?;

    tracing::info!("jobs migrations complete");
    Ok(())
}

/// Run connect capability migrations.
#[cfg(feature = "cap-connect")]
async fn run_connect_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running connect migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_connect_mig").await?;
    sqlx::migrate!("../reactor-connect/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("connect: {}", e)))?;

    tracing::info!("connect migrations complete");
    Ok(())
}

/// Run gateway migrations (reactor_gateway schema).
///
/// This must run before cloud migrations as the cloud provisioner inserts into
/// reactor_gateway.routes for project hostname routing.
#[cfg(feature = "cap-cloud")]
async fn run_gateway_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running gateway migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_gateway_mig").await?;
    sqlx::migrate!("../reactor-gateway/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("gateway: {}", e)))?;

    tracing::info!("gateway migrations complete");
    Ok(())
}

/// Run cloud control plane migrations (reactor_cloud schema).
#[cfg(feature = "cap-cloud")]
async fn run_cloud_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running cloud control plane migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_cloud_mig").await?;
    sqlx::migrate!("../reactor-cloud-api/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("cloud: {}", e)))?;

    tracing::info!("cloud control plane migrations complete");
    Ok(())
}

/// Run sites capability migrations.
#[cfg(feature = "cap-sites")]
async fn run_sites_migrations(pool: &PgPool) -> Result<(), ServerError> {
    tracing::debug!("running sites migrations");

    let mut conn = acquire_isolated_conn(pool, "_reactor_sites_mig").await?;
    sqlx::migrate!("../reactor-sites/migrations")
        .run(&mut *conn)
        .await
        .map_err(|e| ServerError::Migration(format!("sites: {}", e)))?;

    tracing::info!("sites migrations complete");
    Ok(())
}
