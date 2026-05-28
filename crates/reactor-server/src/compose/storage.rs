//! Storage capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{ReactorConfig, StorageConfigSlice};
use crate::error::ServerError;
use reactor_core::auth::AuthClient;
use reactor_core::primitives::vault::Vault;
use reactor_storage::{Deployment, StorageConfig, StorageState};
use reactor_storage::S3BlobStore;
use std::sync::Arc;

/// Vault KV path for storage signing secret.
const STORAGE_SIGNING_SECRET_KEY: &str = "storage/signing-secret";

/// Resolve the signing secret, potentially from vault.
///
/// Supports:
/// - Direct value: `"my-secret-key"` - uses the value directly
/// - Vault reference: `"vault:secret-name"` - fetches from vault KV
async fn resolve_signing_secret(
    config_value: &str,
    vault: &dyn Vault,
    tenant: &reactor_core::ProjectId,
) -> Result<String, ServerError> {
    if config_value.starts_with("vault:") {
        let vault_key = &config_value[6..]; // Strip "vault:" prefix
        let secret = vault
            .get_secret(tenant, vault_key)
            .await
            .map_err(|e| ServerError::Boot(format!("failed to get signing secret from vault: {}", e)))?
            .ok_or_else(|| ServerError::Config(format!(
                "signing secret '{}' not found in vault", vault_key
            )))?;

        String::from_utf8(secret.data)
            .map_err(|_| ServerError::Config("signing secret is not valid UTF-8".to_string()))
    } else {
        Ok(config_value.to_string())
    }
}

/// Build the storage capability slot.
pub async fn build(
    shared: &SharedResources,
    config: &StorageConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<StorageState>, ServerError> {
    // Resolve signing secret (potentially from vault)
    let tenant = full_config.project.project_id();
    let signing_secret = resolve_signing_secret(
        &config.signing_secret,
        shared.vault.as_ref(),
        &tenant,
    ).await?;

    // Convert config slice to full StorageConfig
    let storage_config = StorageConfig {
        database_url: String::new(), // Use shared pool
        bind: "127.0.0.1:8082".parse().unwrap(),
        deployment: Deployment::Monolith,
        admin_token: Some(full_config.admin.token.clone()),
        fs_base_path: config.fs_base_path.clone(),
        s3_bucket: config.s3_bucket.clone(),
        s3_region: config.s3_region.clone(),
        s3_endpoint: config.s3_endpoint.clone(),
        auth_url: None,
        auth_database_url: None,
        auth_data_key: None,
        signing_secret: Some(signing_secret),
        signed_url_expiry_secs: config.signed_url_expiry_secs,
        max_upload_size: config.max_upload_size,
        metrics: false,
        log: "info".to_string(),
    };

    // Build storage state with S3 blob store if configured
    let storage_config = Arc::new(storage_config);
    
    let state = if let Some(ref bucket) = storage_config.s3_bucket {
        // Initialize S3 blob store
        let s3_store = S3BlobStore::from_config(
            bucket.clone(),
            storage_config.s3_region.clone(),
            storage_config.s3_endpoint.clone(),
        )
        .await
        .map_err(|e| ServerError::Boot(format!("failed to initialize S3 blob store: {}", e)))?;
        
        tracing::info!(
            bucket = %bucket,
            region = ?storage_config.s3_region,
            endpoint = ?storage_config.s3_endpoint,
            "S3 blob store initialized"
        );
        
        StorageState::with_blob_store(
            shared.pg.clone(),
            storage_config.clone(),
            auth_client,
            Arc::new(s3_store),
        )
    } else {
        StorageState::new(shared.pg.clone(), storage_config.clone(), auth_client)
    };

    // Build the router
    let router = reactor_storage::router(state.clone());

    // Storage background tasks (signed-URL janitor, multipart cleanup)
    // These would be spawned here in a full implementation
    let tasks = Vec::new();

    tracing::info!("storage capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
