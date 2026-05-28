//! Connect capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{ConnectConfigSlice, ReactorConfig};
use crate::error::ServerError;
use reactor_cache::PostgresBackend;
use reactor_connect::{ConnectConfig, ConnectState, PgConnectStore};
use reactor_core::auth::AuthClient;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Build the connect capability slot.
#[allow(unused_variables)]
pub async fn build(
    shared: &SharedResources,
    config: &ConnectConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<ConnectState<PgConnectStore>>, ServerError> {
    // Build connect config from slice
    let connect_config = ConnectConfig {
        database_url: String::new(), // Use shared pool
        data_key: Some(config.data_key.clone()),
        jobs_url: config.jobs_url.clone(),
        data_url: config.data_url.clone(),
        storage_url: config.storage_url.clone(),
        token_refresh_interval_seconds: config.refresh_interval_secs,
        sandbox_ttl_seconds: config.sandbox_ttl_secs,
        max_concurrent_actions: config.max_concurrent_syncs,
        ..Default::default()
    };

    // Get cache backend
    let cache = Arc::new(PostgresBackend::new(shared.pg.clone()));

    // Build connect store
    let store = PgConnectStore::new(shared.pg.clone());

    // Build the runtime (native-only for now, empty connector map)
    let runtime = reactor_connect::NativeRuntime::new(HashMap::new());

    // Build connect state
    let state = ConnectState::new(
        store,
        auth_client,
        shared.vault.clone(),
        cache,
        Arc::new(runtime),
        connect_config.clone(),
    );

    // Build the router
    let router = reactor_connect::router(state.clone());

    // Spawn background tasks
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    // Token refresh worker
    let refresh_state = state.clone();
    let refresh_shutdown = shared.shutdown.clone();
    let refresh_interval = Duration::from_secs(config.refresh_interval_secs);
    let refresh_task = tokio::spawn(async move {
        reactor_connect::credentials::start_refresh_worker(
            refresh_state,
            refresh_shutdown,
            refresh_interval,
        )
        .await;
    });
    tasks.push(refresh_task);

    // Sandbox cleanup worker
    let cleanup_state = state.clone();
    let cleanup_shutdown = shared.shutdown.clone();
    let cleanup_ttl = Duration::from_secs(config.sandbox_ttl_secs);
    let cleanup_task = tokio::spawn(async move {
        reactor_connect::sandbox::start_cleanup_worker_with_shutdown(
            cleanup_state,
            cleanup_shutdown,
            cleanup_ttl,
        )
        .await;
    });
    tasks.push(cleanup_task);

    tracing::info!("connect capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
