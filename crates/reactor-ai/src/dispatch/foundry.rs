//! Azure Foundry (Azure OpenAI) provider client.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatProvider,
    EmbeddingsRequest, EmbeddingsResponse,
};
use crate::error::AiError;

/// Azure Foundry client configuration.
#[derive(Clone)]
pub struct FoundryConfig {
    /// Azure Foundry endpoint URL.
    pub endpoint: String,
    /// API key.
    pub api_key: String,
}

/// Azure Foundry API client.
#[derive(Clone)]
pub struct FoundryClient {
    client: reqwest::Client,
    config: FoundryConfig,
}

impl FoundryClient {
    /// Create a new Foundry client.
    pub fn new(config: FoundryConfig) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            config,
        }
    }
}

#[async_trait]
impl ChatProvider for FoundryClient {
    async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<(ChatCompletionResponse, Duration), AiError> {
        let start = Instant::now();

        let mut req = request.clone();
        req.model = upstream_model.to_string();
        req.stream = Some(false);

        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version=2024-10-21",
            self.config.endpoint.trim_end_matches('/'),
            upstream_model
        );

        let response = self
            .client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::UpstreamError(format!(
                "Azure Foundry returned {}: {}",
                status, body
            )));
        }

        let result: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Failed to parse response: {}", e)))?;

        Ok((result, start.elapsed()))
    }

    async fn chat_completion_stream(
        &self,
        request: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<
        (
            Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, AiError>> + Send>>,
            Instant,
        ),
        AiError,
    > {
        // TODO: Implement Azure Foundry streaming
        let _ = (request, upstream_model);
        Err(AiError::Internal(
            "Azure Foundry streaming not yet implemented".to_string(),
        ))
    }

    async fn embeddings(
        &self,
        request: &EmbeddingsRequest,
        upstream_model: &str,
    ) -> Result<(EmbeddingsResponse, Duration), AiError> {
        let start = Instant::now();

        let mut req = request.clone();
        req.model = upstream_model.to_string();

        let url = format!(
            "{}/openai/deployments/{}/embeddings?api-version=2024-10-21",
            self.config.endpoint.trim_end_matches('/'),
            upstream_model
        );

        let response = self
            .client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::UpstreamError(format!(
                "Azure Foundry returned {}: {}",
                status, body
            )));
        }

        let result: EmbeddingsResponse = response
            .json()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Failed to parse response: {}", e)))?;

        Ok((result, start.elapsed()))
    }

    fn name(&self) -> &'static str {
        "foundry"
    }
}
