//! Generic OpenAI-compatible provider client.
//!
//! Works with any provider that implements the OpenAI API format
//! (Together, Groq, Fireworks, self-hosted vLLM, etc.).

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatProvider,
    EmbeddingsRequest, EmbeddingsResponse,
};
use crate::error::AiError;

/// OpenAI-compatible provider configuration.
#[derive(Clone)]
pub struct OpenAiCompatibleConfig {
    /// Provider name (for logging and identification).
    pub name: String,
    /// Base URL for the API.
    pub base_url: String,
    /// API key.
    pub api_key: String,
    /// Custom headers to add to requests.
    pub headers: Vec<(String, String)>,
}

/// Generic OpenAI-compatible API client.
#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    client: reqwest::Client,
    config: OpenAiCompatibleConfig,
}

impl OpenAiCompatibleClient {
    /// Create a new OpenAI-compatible client.
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
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
impl ChatProvider for OpenAiCompatibleClient {
    async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<(ChatCompletionResponse, Duration), AiError> {
        let start = Instant::now();

        let mut req = request.clone();
        req.model = upstream_model.to_string();
        req.stream = Some(false);

        let mut builder = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json");

        for (key, value) in &self.config.headers {
            builder = builder.header(key, value);
        }

        let response = builder
            .json(&req)
            .send()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::UpstreamError(format!(
                "{} returned {}: {}",
                self.config.name, status, body
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
        let start = Instant::now();

        let mut req = request.clone();
        req.model = upstream_model.to_string();
        req.stream = Some(true);

        let mut builder = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json");

        for (key, value) in &self.config.headers {
            builder = builder.header(key, value);
        }

        let response = builder
            .json(&req)
            .send()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::UpstreamError(format!(
                "{} returned {}: {}",
                self.config.name, status, body
            )));
        }

        let name = self.config.name.clone();
        let stream = parse_sse_stream(response, name);
        Ok((Box::pin(stream), start))
    }

    async fn embeddings(
        &self,
        request: &EmbeddingsRequest,
        upstream_model: &str,
    ) -> Result<(EmbeddingsResponse, Duration), AiError> {
        let start = Instant::now();

        let mut req = request.clone();
        req.model = upstream_model.to_string();

        let mut builder = self
            .client
            .post(format!("{}/embeddings", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json");

        for (key, value) in &self.config.headers {
            builder = builder.header(key, value);
        }

        let response = builder
            .json(&req)
            .send()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::UpstreamError(format!(
                "{} returned {}: {}",
                self.config.name, status, body
            )));
        }

        let result: EmbeddingsResponse = response
            .json()
            .await
            .map_err(|e| AiError::UpstreamError(format!("Failed to parse response: {}", e)))?;

        Ok((result, start.elapsed()))
    }

    fn name(&self) -> &'static str {
        // This is a workaround since we can't return the dynamic name
        // In practice, we use the config.name field for logging
        "openai-compatible"
    }
}

fn parse_sse_stream(
    response: reqwest::Response,
    _provider_name: String,
) -> impl Stream<Item = Result<ChatCompletionChunk, AiError>> {
    use async_stream::stream;
    use futures::StreamExt;

    stream! {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(AiError::UpstreamError(format!("Stream error: {}", e)));
                    continue;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        return;
                    }

                    match serde_json::from_str::<ChatCompletionChunk>(data) {
                        Ok(chunk) => yield Ok(chunk),
                        Err(e) => {
                            tracing::warn!(error = %e, data = %data, "Failed to parse SSE chunk");
                        }
                    }
                }
            }
        }
    }
}
