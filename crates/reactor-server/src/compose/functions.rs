//! Functions capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{FunctionsConfigSlice, ReactorConfig};
use crate::error::ServerError;
use reactor_core::auth::AuthClient;
use reactor_core::primitives::vault::Vault;
use reactor_functions::{
    BunRuntime, BunRuntimeConfig, Deployment, FunctionsConfig, FunctionsState, RuntimeRegistry,
    WasmRuntime, WasmRuntimeConfig,
};
use std::path::PathBuf;
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

/// Build the functions capability slot.
pub async fn build(
    shared: &SharedResources,
    config: &FunctionsConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<FunctionsState>, ServerError> {
    // Resolve data key (potentially from vault)
    let tenant = full_config.project.project_id();
    let data_key = resolve_secret(
        &config.data_key,
        shared.vault.as_ref(),
        &tenant,
        "functions data key",
    ).await?;

    // Convert config slice to full FunctionsConfig
    let functions_config = FunctionsConfig {
        database_url: String::new(), // Use shared pool
        bind: "127.0.0.1:8083".parse().unwrap(),
        deployment: Deployment::Monolith,
        workdir: config.workdir.clone(),
        storage_url: String::new(), // Use in-process storage
        storage_api_key: String::new(),
        data_key,
        auth_url: None,
        auth_database_url: None,
        auth_data_key: None,
        internal_secret: None,
        invoke_default_timeout_ms: config.invoke_default_timeout_ms,
        invoke_max_timeout_ms: config.invoke_max_timeout_ms,
        bundle_max_bytes: config.bundle_max_bytes,
        bun_bin: config.bun_bin.clone(),
        bun_idle_ttl_secs: config.bun_idle_ttl_secs,
        bun_max_instances_per_fn: config.bun_max_instances_per_fn,
        lambda_region: config.lambda_region.clone(),
        lambda_role_arn: config.lambda_role_arn.clone(),
        lambda_bundle_s3_bucket: config.lambda_bundle_s3_bucket.clone(),
        lambda_lwa_layer_arn: None,
        lambda_log_group_prefix: "/reactor/functions/".to_string(),
        metrics: false,
        log: "info".to_string(),
    };

    // Create runtime registry and register runtimes
    let runtimes = Arc::new(RuntimeRegistry::new());
    
    // Register Bun runtime
    let bun_config = BunRuntimeConfig {
        bun_bin: config.bun_bin.clone(),
        workdir: PathBuf::from(&config.workdir).join("bun"),
        idle_ttl_secs: config.bun_idle_ttl_secs,
        max_instances_per_fn: config.bun_max_instances_per_fn,
    };
    let bun_runtime = Arc::new(BunRuntime::new(bun_config));
    runtimes.register(bun_runtime).await;
    tracing::info!("registered Bun runtime");
    
    // Register Wasm runtime
    let wasm_config = WasmRuntimeConfig {
        cache_dir: PathBuf::from(&config.workdir).join("wasm-cache"),
    };
    let wasm_runtime = Arc::new(WasmRuntime::new(wasm_config));
    runtimes.register(wasm_runtime).await;
    tracing::info!("registered Wasm runtime");

    // Build functions state
    let state = FunctionsState::new(
        shared.pg.clone(),
        Arc::new(functions_config),
        auth_client,
        runtimes,
    );

    // Build the router
    let router = reactor_functions::router(state.clone());

    // Functions background tasks (reconciler)
    let tasks = Vec::new();

    tracing::info!("functions capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
