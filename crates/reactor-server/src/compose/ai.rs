//! AI capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{AiConfigSlice, ReactorConfig};
use crate::error::ServerError;
use reactor_ai::{AiConfig, AiState, Registry};
use reactor_ai::config::{Deployment, ProvidersConfig};
use reactor_ai::ext::NoopExtensions;
use reactor_core::auth::AuthClient;
use reactor_core::primitives::vault::Vault;
use std::sync::Arc;

/// Resolve a secret value that may reference vault.
async fn resolve_secret(
    value: &str,
    vault: &dyn Vault,
    tenant: &reactor_core::ProjectId,
) -> Result<String, ServerError> {
    if value.starts_with("vault:") {
        let vault_key = &value[6..];
        let secret = vault
            .get_secret(tenant, vault_key)
            .await
            .map_err(|e| ServerError::Boot(format!("failed to get secret from vault: {}", e)))?
            .ok_or_else(|| ServerError::Config(format!(
                "secret '{}' not found in vault", vault_key
            )))?;

        String::from_utf8(secret.data)
            .map_err(|_| ServerError::Config("secret is not valid UTF-8".to_string()))
    } else {
        Ok(value.to_string())
    }
}

/// Build the AI capability slot.
pub async fn build(
    shared: &SharedResources,
    config: &AiConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<AiState>, ServerError> {
    let tenant = full_config.project.project_id();

    // Resolve provider credentials from vault if needed
    let openrouter_api_key = if let Some(ref key) = config.openrouter_api_key {
        Some(resolve_secret(key, shared.vault.as_ref(), &tenant).await?)
    } else {
        None
    };

    let aws_access_key_id = if let Some(ref key) = config.aws_access_key_id {
        Some(resolve_secret(key, shared.vault.as_ref(), &tenant).await?)
    } else {
        None
    };

    let aws_secret_access_key = if let Some(ref key) = config.aws_secret_access_key {
        Some(resolve_secret(key, shared.vault.as_ref(), &tenant).await?)
    } else {
        None
    };

    let azure_foundry_api_key = if let Some(ref key) = config.azure_foundry_api_key {
        Some(resolve_secret(key, shared.vault.as_ref(), &tenant).await?)
    } else {
        None
    };

    // Build the AI config
    let ai_config = AiConfig {
        bind: "127.0.0.1:8090".parse().unwrap(),
        deployment: Deployment::Monolith,
        admin_token: Some(full_config.admin.token.clone()),
        providers: ProvidersConfig {
            openrouter_api_key,
            aws_access_key_id,
            aws_secret_access_key,
            aws_session_token: config.aws_session_token.clone(),
            aws_bedrock_region: config.aws_bedrock_region.clone().unwrap_or_else(|| "us-east-1".to_string()),
            azure_foundry_endpoint: config.azure_foundry_endpoint.clone(),
            azure_foundry_api_key,
            openai_compatible: Vec::new(),
        },
        registry_overlay: config.registry_overlay.clone(),
        registry_url: config.registry_url.clone(),
        default_alias: config.default_alias.clone(),
        auth_url: None,
        metrics: false,
        log: "info".to_string(),
        timeout_secs: 120,
    };

    // Load model registry with optional overlay
    let mut registry = Registry::load_defaults()
        .map_err(|e| ServerError::Boot(format!("failed to load AI model registry: {}", e)))?;

    if let Some(ref overlay_path) = ai_config.registry_overlay {
        let overlay = reactor_ai::registry::RegistryOverlay::from_file(overlay_path)
            .map_err(|e| ServerError::Boot(format!("failed to load registry overlay: {}", e)))?;
        registry = registry.with_overlay(overlay);
    }

    let ai_config = Arc::new(ai_config);
    let registry = Arc::new(registry);
    let extensions = Arc::new(NoopExtensions::new());

    let mut state = AiState::new(ai_config.clone(), registry, auth_client, extensions);

    // Initialize providers based on configuration
    #[cfg(feature = "openrouter")]
    if let Some(ref api_key) = ai_config.providers.openrouter_api_key {
        let client = Arc::new(reactor_ai::dispatch::OpenRouterClient::new(api_key.clone()));
        state = state.with_openrouter(client);
        tracing::info!("OpenRouter provider initialized");
    }

    #[cfg(feature = "bedrock")]
    if ai_config.providers.aws_access_key_id.is_some()
        && ai_config.providers.aws_secret_access_key.is_some()
    {
        let bedrock_config = reactor_ai::dispatch::bedrock::BedrockConfig {
            access_key_id: ai_config.providers.aws_access_key_id.clone().unwrap(),
            secret_access_key: ai_config.providers.aws_secret_access_key.clone().unwrap(),
            session_token: ai_config.providers.aws_session_token.clone(),
            region: ai_config.providers.aws_bedrock_region.clone(),
        };
        let client = Arc::new(reactor_ai::dispatch::BedrockClient::new(bedrock_config));
        state = state.with_bedrock(client);
        tracing::info!(region = %ai_config.providers.aws_bedrock_region, "Bedrock provider initialized");
    }

    #[cfg(feature = "foundry")]
    if let (Some(ref endpoint), Some(ref api_key)) = (
        &ai_config.providers.azure_foundry_endpoint,
        &ai_config.providers.azure_foundry_api_key,
    ) {
        let foundry_config = reactor_ai::dispatch::foundry::FoundryConfig {
            endpoint: endpoint.clone(),
            api_key: api_key.clone(),
        };
        let client = Arc::new(reactor_ai::dispatch::FoundryClient::new(foundry_config));
        state = state.with_foundry(client);
        tracing::info!(endpoint = %endpoint, "Azure Foundry provider initialized");
    }

    // Build the router
    let router = reactor_ai::router(state.clone());

    // No background tasks for AI capability currently
    let tasks = Vec::new();

    tracing::info!("AI capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
