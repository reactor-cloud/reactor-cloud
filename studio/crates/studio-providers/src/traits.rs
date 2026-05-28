// Ported from 1jehuang/jcode (MIT) - jcode-provider-core/src/traits.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use studio_protocol::{Message, StreamChunk};

use crate::ProviderError;

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Options for LLM requests
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Option<Vec<String>>,
}

/// Provider configuration
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Stream a response from the LLM
    async fn stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        options: RequestOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>;

    /// Non-streaming request (convenience method)
    async fn generate(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        options: RequestOptions,
    ) -> Result<Message, ProviderError>;
}
