//! AWS Bedrock provider client.
//!
//! Implements chat completions and embeddings via AWS Bedrock Runtime API
//! using SigV4 request signing.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatProvider,
    EmbeddingsRequest, EmbeddingsResponse,
};
use crate::error::AiError;

/// AWS Bedrock client configuration.
#[derive(Clone)]
pub struct BedrockConfig {
    /// AWS Access Key ID.
    pub access_key_id: String,
    /// AWS Secret Access Key.
    pub secret_access_key: String,
    /// AWS Session Token (optional, for STS credentials).
    pub session_token: Option<String>,
    /// AWS region (e.g., "us-east-1").
    pub region: String,
}

/// AWS Bedrock API client.
#[derive(Clone)]
pub struct BedrockClient {
    client: reqwest::Client,
    config: BedrockConfig,
}

impl BedrockClient {
    /// Create a new Bedrock client.
    pub fn new(config: BedrockConfig) -> Self {
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
impl ChatProvider for BedrockClient {
    async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<(ChatCompletionResponse, Duration), AiError> {
        let start = Instant::now();

        // TODO: Implement Bedrock converse API with SigV4 signing
        // For now, return a placeholder error
        let _ = (request, upstream_model);
        Err(AiError::Internal(
            "Bedrock provider not yet implemented".to_string(),
        ))
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
        // TODO: Implement Bedrock converse stream API with SigV4 signing
        let _ = (request, upstream_model);
        Err(AiError::Internal(
            "Bedrock streaming not yet implemented".to_string(),
        ))
    }

    async fn embeddings(
        &self,
        request: &EmbeddingsRequest,
        upstream_model: &str,
    ) -> Result<(EmbeddingsResponse, Duration), AiError> {
        let start = Instant::now();

        // TODO: Implement Bedrock embeddings API
        let _ = (request, upstream_model);
        Err(AiError::Internal(
            "Bedrock embeddings not yet implemented".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "bedrock"
    }
}
