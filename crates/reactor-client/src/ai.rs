//! AI capability client (`/ai/v1/*`).

use crate::error::{ClientError, ClientResult};
use crate::http::Client;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl ChatCompletionRequest {
    /// Create a new chat completion request.
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
            stream: None,
        }
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// Usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Choice in a chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

/// Chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Delta content in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Choice in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

/// Streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Embeddings input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingsInput {
    Single(String),
    Multiple(Vec<String>),
}

/// Embeddings request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: EmbeddingsInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

impl EmbeddingsRequest {
    /// Create a new embeddings request with a single input.
    pub fn new(model: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            input: EmbeddingsInput::Single(input.into()),
            dimensions: None,
        }
    }

    /// Create a new embeddings request with multiple inputs.
    pub fn with_multiple(model: impl Into<String>, inputs: Vec<String>) -> Self {
        Self {
            model: model.into(),
            input: EmbeddingsInput::Multiple(inputs),
            dimensions: None,
        }
    }

    /// Set output dimensions.
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

/// Embedding data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: u32,
}

/// Embeddings usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

/// Embeddings response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingsUsage,
}

/// Model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

/// Models list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsListResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

impl Client {
    /// Send a chat completion request (non-streaming).
    pub async fn ai_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> ClientResult<ChatCompletionResponse> {
        self.post("/ai/v1/chat/completions", &request).await
    }

    /// Send a chat completion request with streaming.
    ///
    /// Returns a stream of chunks that yields until completion.
    pub async fn ai_chat_stream(
        &self,
        mut request: ChatCompletionRequest,
    ) -> ClientResult<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ClientError>> + Send>>>
    {
        request.stream = Some(true);
        let response = self.post_sse("/ai/v1/chat/completions", &request).await?;
        Ok(Box::pin(parse_sse_stream(response)))
    }

    /// Send an embeddings request.
    pub async fn ai_embed(
        &self,
        request: EmbeddingsRequest,
    ) -> ClientResult<EmbeddingsResponse> {
        self.post("/ai/v1/embeddings", &request).await
    }

    /// List available models.
    pub async fn ai_models_list(&self) -> ClientResult<ModelsListResponse> {
        self.get("/ai/v1/models").await
    }
}

fn parse_sse_stream(
    response: reqwest::Response,
) -> impl Stream<Item = Result<ChatCompletionChunk, ClientError>> {
    use async_stream::stream;
    use futures::StreamExt;

    stream! {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(ClientError::Network(e));
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
                            yield Err(ClientError::InvalidResponse(format!(
                                "Failed to parse SSE chunk: {}", e
                            )));
                        }
                    }
                }
            }
        }
    }
}

/// Helper to create a user message.
pub fn user_message(content: impl Into<String>) -> Message {
    Message {
        role: "user".to_string(),
        content: Some(content.into()),
        name: None,
    }
}

/// Helper to create a system message.
pub fn system_message(content: impl Into<String>) -> Message {
    Message {
        role: "system".to_string(),
        content: Some(content.into()),
        name: None,
    }
}

/// Helper to create an assistant message.
pub fn assistant_message(content: impl Into<String>) -> Message {
    Message {
        role: "assistant".to_string(),
        content: Some(content.into()),
        name: None,
    }
}
