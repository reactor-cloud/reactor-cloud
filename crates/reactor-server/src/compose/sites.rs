//! Sites capability composition.
//!
//! Sites requires functions and storage to be available for SSR dispatch
//! and static file serving. In the unified server, we use loopback HTTP
//! clients that call back to the server's own bind address.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{ReactorConfig, SitesConfigSlice};
use crate::error::ServerError;
use reactor_cache::PostgresBackend;
use reactor_core::auth::AuthClient;
use reactor_sites::config::SitesConfig;
use reactor_sites::dispatch::{FunctionsClient, StorageClient};
use reactor_sites::SitesState;
use std::sync::Arc;

/// Build the sites capability slot.
///
/// Creates loopback HTTP clients for functions and storage that call back
/// to the server's own bind address using the admin token for auth.
pub async fn build(
    shared: &SharedResources,
    config: &SitesConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<reactor_sites::SitesState>, ServerError> {
    // Build loopback URL from server bind address
    let bind = full_config.server.bind;
    let loopback_url = format!("http://{}", bind);

    // Use admin token for internal service calls
    let admin_token = full_config.admin.token.clone();

    // Build HTTP clients for functions and storage (loopback to self)
    let functions = FunctionsClient::new(loopback_url.clone(), admin_token.clone());
    let storage = StorageClient::new(loopback_url.clone(), admin_token.clone());

    // Build cache backend from shared PostgreSQL pool
    let cache = Arc::new(PostgresBackend::new(shared.pg.clone()));

    // Build SitesConfig from the slice and shared resources
    let sites_config = Arc::new(SitesConfig {
        database_url: full_config.database.url.clone(),
        bind: full_config.server.bind,
        deployment: reactor_sites::config::Deployment::Monolith,
        workdir: config.workdir.clone(),
        functions_url: loopback_url.clone(),
        functions_api_key: admin_token.clone(),
        storage_url: loopback_url.clone(),
        storage_api_key: admin_token.clone(),
        storage_bucket: None, // Use the default bucket from storage config
        jobs_url: None,
        jobs_api_key: None,
        auth_url: None,
        auth_database_url: None,
        auth_data_key: None,
        internal_secret: None,
        revalidation_secret: config
            .revalidation_secret
            .clone()
            .unwrap_or_else(|| "reactor-revalidate".to_string()),
        static_max_files: 50_000,
        static_max_bytes: config.bundle_max_bytes,
        isr_default_ttl_secs: config.isr_default_revalidate_secs,
        preview_subdomain: config.preview_subdomain.clone(),
        acme_email: None,
        acme_directory: None,
        metrics: false,
        invocation_sample_rate: 0.01,
        log: "info".to_string(),
    });

    // Build SitesState
    let state = SitesState::new(
        shared.pg.clone(),
        sites_config,
        auth_client,
        functions,
        storage.clone(),
        cache,
    );

    // Ensure the _reactor_sites system bucket exists via direct SQL
    // (HTTP client won't work during startup - server not listening yet)
    if let Err(e) = ensure_sites_bucket(&shared.pg).await {
        tracing::warn!(error = %e, "Failed to ensure _reactor_sites bucket via SQL");
    } else {
        tracing::info!("Sites system bucket _reactor_sites ready");
    }

    // Build the router
    let router = reactor_sites::router(state.clone());

    tracing::info!("sites capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks: vec![],
    })
}

/// Ensure the _reactor_sites system bucket exists via direct SQL.
///
/// Creates a "system" bucket that's not tied to any org.
/// We use a nil UUID (all zeros) as the org_id for system buckets.
async fn ensure_sites_bucket(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // Use nil UUID for system org - this bucket is internal to reactor-sites
    let system_org_id = uuid::Uuid::nil();
    
    sqlx::query(
        r#"
        INSERT INTO _reactor_storage.buckets (id, org_id, slug, is_public, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, '_reactor_sites', false, NOW(), NOW())
        ON CONFLICT (org_id, slug) DO NOTHING
        "#,
    )
    .bind(system_org_id)
    .execute(pool)
    .await?;

    Ok(())
}
