//! AI service state.

use crate::config::AiConfig;
use crate::dispatch::ChatProvider;
use crate::ext::AiExtensions;
use crate::registry::Registry;
use reactor_core::auth::AuthClient;
use std::sync::Arc;

/// AI service state.
#[derive(Clone)]
pub struct AiState {
    /// AI configuration.
    pub config: Arc<AiConfig>,

    /// Model registry.
    pub registry: Arc<Registry>,

    /// Authentication client.
    pub auth: Arc<dyn AuthClient>,

    /// OpenRouter client (if configured).
    #[cfg(feature = "openrouter")]
    pub openrouter: Option<Arc<dyn ChatProvider>>,

    /// Bedrock client (if configured).
    #[cfg(feature = "bedrock")]
    pub bedrock: Option<Arc<dyn ChatProvider>>,

    /// Azure Foundry client (if configured).
    #[cfg(feature = "foundry")]
    pub foundry: Option<Arc<dyn ChatProvider>>,

    /// Generic OpenAI-compatible clients.
    #[cfg(feature = "openai-compatible")]
    pub openai_compatible: Vec<Arc<dyn ChatProvider>>,

    /// Extension hooks (quota checks, billing, etc.).
    pub extensions: Arc<dyn AiExtensions>,
}

impl AiState {
    /// Create a new AI state with minimal configuration.
    pub fn new(
        config: Arc<AiConfig>,
        registry: Arc<Registry>,
        auth: Arc<dyn AuthClient>,
        extensions: Arc<dyn AiExtensions>,
    ) -> Self {
        Self {
            config,
            registry,
            auth,
            #[cfg(feature = "openrouter")]
            openrouter: None,
            #[cfg(feature = "bedrock")]
            bedrock: None,
            #[cfg(feature = "foundry")]
            foundry: None,
            #[cfg(feature = "openai-compatible")]
            openai_compatible: Vec::new(),
            extensions,
        }
    }

    /// Set the OpenRouter client.
    #[cfg(feature = "openrouter")]
    pub fn with_openrouter(mut self, client: Arc<dyn ChatProvider>) -> Self {
        self.openrouter = Some(client);
        self
    }

    /// Set the Bedrock client.
    #[cfg(feature = "bedrock")]
    pub fn with_bedrock(mut self, client: Arc<dyn ChatProvider>) -> Self {
        self.bedrock = Some(client);
        self
    }

    /// Set the Azure Foundry client.
    #[cfg(feature = "foundry")]
    pub fn with_foundry(mut self, client: Arc<dyn ChatProvider>) -> Self {
        self.foundry = Some(client);
        self
    }

    /// Add an OpenAI-compatible client.
    #[cfg(feature = "openai-compatible")]
    pub fn with_openai_compatible(mut self, client: Arc<dyn ChatProvider>) -> Self {
        self.openai_compatible.push(client);
        self
    }

    /// Get a chat provider by name.
    pub fn get_provider(&self, name: &str) -> Option<Arc<dyn ChatProvider>> {
        #[cfg(feature = "openrouter")]
        if name == "openrouter" {
            return self.openrouter.clone();
        }

        #[cfg(feature = "bedrock")]
        if name == "bedrock" {
            return self.bedrock.clone();
        }

        #[cfg(feature = "foundry")]
        if name == "foundry" || name == "azure" {
            return self.foundry.clone();
        }

        #[cfg(feature = "openai-compatible")]
        for client in &self.openai_compatible {
            if client.name() == name {
                return Some(client.clone());
            }
        }

        None
    }

    /// Get all available provider names.
    pub fn available_providers(&self) -> Vec<&'static str> {
        let mut providers = Vec::new();

        #[cfg(feature = "openrouter")]
        if self.openrouter.is_some() {
            providers.push("openrouter");
        }

        #[cfg(feature = "bedrock")]
        if self.bedrock.is_some() {
            providers.push("bedrock");
        }

        #[cfg(feature = "foundry")]
        if self.foundry.is_some() {
            providers.push("foundry");
        }

        providers
    }
}
