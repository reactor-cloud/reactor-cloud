//! Analytics capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{AnalyticsConfigSlice, ReactorConfig};
use crate::error::ServerError;
use reactor_analytics::{
    config::{AnalyticsConfig, Deployment},
    ingest::{create_batcher_channel, Batcher, BatcherConfig},
    AnalyticsState, PgAnalyticsStore,
};
use reactor_core::auth::AuthClient;
use reactor_core::primitives::vault::Vault;
use std::sync::Arc;

/// Resolve a secret value, potentially from vault.
///
/// Supports:
/// - Direct value: `"my-secret-key"` - uses the value directly
/// - Vault reference: `"vault:secret-name"` - fetches from vault KV
async fn resolve_secret(
    config_value: &str,
    vault: &dyn Vault,
    tenant: &reactor_core::ProjectId,
    description: &str,
) -> Result<String, ServerError> {
    if config_value.starts_with("vault:") {
        let vault_key = &config_value[6..]; // Strip "vault:" prefix
        let secret = vault
            .get_secret(tenant, vault_key)
            .await
            .map_err(|e| ServerError::Boot(format!("failed to get {} from vault: {}", description, e)))?
            .ok_or_else(|| ServerError::Config(format!(
                "{} '{}' not found in vault", description, vault_key
            )))?;

        String::from_utf8(secret.data)
            .map_err(|_| ServerError::Config(format!("{} is not valid UTF-8", description)))
    } else {
        Ok(config_value.to_string())
    }
}

/// Build the analytics capability slot.
pub async fn build(
    shared: &SharedResources,
    config: &AnalyticsConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<AnalyticsState<PgAnalyticsStore>>, ServerError> {
    // Resolve internal secret (potentially from vault) if configured
    let tenant = full_config.project.project_id();
    let internal_secret = if let Some(ref secret_cfg) = config.internal_secret {
        Some(resolve_secret(
            secret_cfg,
            shared.vault.as_ref(),
            &tenant,
            "analytics internal secret",
        ).await?)
    } else {
        None
    };

    // Convert config slice to full AnalyticsConfig
    let analytics_config = AnalyticsConfig {
        database_url: String::new(), // Use shared pool
        bind: "127.0.0.1:8006".parse().unwrap(),
        deployment: Deployment::Monolith,
        admin_token: Some(full_config.admin.token.clone()),
        auth_url: None,
        auth_database_url: None,
        auth_data_key: None,
        internal_secret,
        geo_db_path: config.geo_db_path.clone(),
        honor_dnt: config.honor_dnt.unwrap_or(true),
        max_properties_bytes: config.max_properties_bytes.unwrap_or(32768),
        quota_per_org_monthly: config.quota_per_org_monthly.unwrap_or(1_000_000),
        retention_days: config.retention_days.unwrap_or(90),
        batch_interval_ms: config.batch_interval_ms.unwrap_or(200),
        batch_max_rows: config.batch_max_rows.unwrap_or(500),
        batch_queue_depth: config.batch_queue_depth.unwrap_or(50000),
        query_timeout_ms: config.query_timeout_ms.unwrap_or(30000),
        query_max_rows: config.query_max_rows.unwrap_or(100000),
        query_raw_range_days: config.query_raw_range_days.unwrap_or(90),
        query_agg_range_days: config.query_agg_range_days.unwrap_or(730),
        rate_limit_rps: config.rate_limit_rps.unwrap_or(1000),
        rate_limit_burst: config.rate_limit_burst.unwrap_or(100),
        rate_limit_per_second: config.rate_limit_per_second.unwrap_or(50),
        quota_cache_ttl_secs: config.quota_cache_ttl_secs.unwrap_or(60),
        sample_rate: config.sample_rate.unwrap_or(1.0),
        metrics: false,
        log: "info".to_string(),
    };

    let config_arc = Arc::new(analytics_config);

    // Build the store using shared pool
    let store = Arc::new(PgAnalyticsStore::new(shared.pg.clone()));

    // Create batcher channel
    let (batcher_tx, batcher_rx) = create_batcher_channel(&config_arc);

    // Build analytics state
    let state = AnalyticsState::new(store.clone(), config_arc.clone(), auth_client, batcher_tx);

    // Build the router
    let router = reactor_analytics::router(state.clone());

    // Spawn batcher background task
    let batcher_store = store.clone();
    let batcher_config = BatcherConfig::from(config_arc.as_ref());
    let batcher_task = tokio::spawn(async move {
        let batcher = Batcher::new(batcher_store, batcher_config, batcher_rx);
        batcher.run().await;
    });

    let tasks = vec![batcher_task];

    tracing::info!("analytics capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
