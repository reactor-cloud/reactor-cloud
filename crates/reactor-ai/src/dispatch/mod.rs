//! Provider dispatch layer for routing requests to upstream providers.

#[cfg(feature = "bedrock")]
pub mod bedrock;
#[cfg(feature = "foundry")]
pub mod foundry;
#[cfg(feature = "openai-compatible")]
pub mod openai_compatible;
#[cfg(feature = "openrouter")]
pub mod openrouter;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::time::{Duration, Instant};
use utoipa::ToSchema;

use crate::error::AiError;

#[cfg(feature = "bedrock")]
pub use bedrock::BedrockClient;
#[cfg(feature = "foundry")]
pub use foundry::FoundryClient;
#[cfg(feature = "openai-compatible")]
pub use openai_compatible::OpenAiCompatibleClient;
#[cfg(feature = "openrouter")]
pub use openrouter::OpenRouterClient;

/// Trait for chat completion providers (OpenRouter, Bedrock, etc.).
#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Send a non-streaming chat completion request.
    async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<(ChatCompletionResponse, Duration), AiError>;

    /// Send a streaming chat completion request.
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
    >;

    /// Send an embeddings request.
    async fn embeddings(
        &self,
        request: &EmbeddingsRequest,
        upstream_model: &str,
    ) -> Result<(EmbeddingsResponse, Duration), AiError>;

    /// Get the provider name (for logging).
    fn name(&self) -> &'static str;
}

/// Chat completion request (OpenAI-compatible).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionRequest {
    /// Model identifier or alias.
    pub model: String,
    /// Messages in the conversation.
    pub messages: Vec<Message>,
    /// Sampling temperature (0-2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Nucleus sampling parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Whether to stream the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    /// Presence penalty (-2 to 2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty (-2 to 2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// User identifier for tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Message in a chat completion.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Message {
    /// Role of the message author.
    pub role: String,
    /// Content of the message.
    pub content: Option<String>,
    /// Optional name of the author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Stop sequence.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum Stop {
    /// Single stop sequence.
    Single(String),
    /// Multiple stop sequences.
    Multiple(Vec<String>),
}

/// Chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionResponse {
    /// Unique identifier.
    pub id: String,
    /// Object type (always "chat.completion").
    pub object: String,
    /// Creation timestamp (Unix seconds).
    pub created: i64,
    /// Model used.
    pub model: String,
    /// Generated choices.
    pub choices: Vec<Choice>,
    /// Token usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// System fingerprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// Choice in a chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Choice {
    /// Choice index.
    pub index: u32,
    /// Generated message.
    pub message: Message,
    /// Reason for finishing.
    pub finish_reason: Option<String>,
}

/// Usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Usage {
    /// Tokens in the prompt.
    pub prompt_tokens: u32,
    /// Tokens in the completion.
    pub completion_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// Streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionChunk {
    /// Unique identifier.
    pub id: String,
    /// Object type (always "chat.completion.chunk").
    pub object: String,
    /// Creation timestamp (Unix seconds).
    pub created: i64,
    /// Model used.
    pub model: String,
    /// Chunk choices.
    pub choices: Vec<ChunkChoice>,
    /// Usage (only in final chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Choice in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChunkChoice {
    /// Choice index.
    pub index: u32,
    /// Delta content.
    pub delta: Delta,
    /// Reason for finishing.
    pub finish_reason: Option<String>,
}

/// Delta content in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Delta {
    /// Role (only in first chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Content fragment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Embeddings request (OpenAI-compatible).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmbeddingsRequest {
    /// Model identifier.
    pub model: String,
    /// Input text(s) to embed.
    pub input: EmbeddingsInput,
    /// Encoding format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    /// Embedding dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// User identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Input for embeddings.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum EmbeddingsInput {
    /// Single text.
    Single(String),
    /// Multiple texts.
    Multiple(Vec<String>),
}

impl EmbeddingsInput {
    /// Get inputs as a vector of strings.
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            EmbeddingsInput::Single(s) => vec![s.as_str()],
            EmbeddingsInput::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }

    /// Count the number of inputs.
    pub fn len(&self) -> usize {
        match self {
            EmbeddingsInput::Single(_) => 1,
            EmbeddingsInput::Multiple(v) => v.len(),
        }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Embeddings response (OpenAI-compatible).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmbeddingsResponse {
    /// Object type (always "list").
    pub object: String,
    /// Embedding data.
    pub data: Vec<EmbeddingData>,
    /// Model used.
    pub model: String,
    /// Usage statistics.
    pub usage: EmbeddingsUsage,
}

/// Single embedding data.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmbeddingData {
    /// Object type (always "embedding").
    pub object: String,
    /// The embedding vector.
    pub embedding: Vec<f32>,
    /// Index in the input array.
    pub index: u32,
}

/// Usage statistics for embeddings.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmbeddingsUsage {
    /// Tokens in the prompt.
    pub prompt_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}
