//! Data capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::DataConfigSlice;
use crate::error::ServerError;
use reactor_core::auth::AuthClient;
use reactor_data::{DataConfig, DataState, Deployment, PgDataStore};
use std::sync::Arc;

/// Build the data capability slot.
pub async fn build(
    shared: &SharedResources,
    config: &DataConfigSlice,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<DataState>, ServerError> {
    // Build DataConfig from slice
    let data_config = DataConfig {
        database_url: String::new(), // We use shared pool
        bind: "127.0.0.1:8002".parse().unwrap(),
        migrations_dir: config.migrations_dir.clone(),
        run_migrations: false, // Migrations handled separately
        user_schema: config.user_schema.clone(),
        max_embed_depth: config.max_embed_depth,
        max_limit: config.max_limit,
        default_limit: config.default_limit,
        deployment: Deployment::Monolith, // Always monolith in unified server
        auth_url: None,
        internal_secret: None,
        auth_database_url: None,
        auth_data_key: None,
        log: "info".to_string(),
        metrics: false,
    };

    // Build the data store
    let store = Arc::new(PgDataStore::new(shared.pg.clone()));

    // Build the data state
    let state = DataState::new(store, auth_client, Arc::new(data_config));

    // Build the router
    let router = reactor_data::router(state.clone());

    // Data has no background tasks at v0
    let tasks = Vec::new();

    tracing::info!("data capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
