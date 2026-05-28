//! Reactor AI capability.
//!
//! Provides an OpenAI-compatible HTTP surface with multi-provider dispatch
//! (Bedrock, OpenRouter, Azure Foundry, generic OpenAI-compatible endpoints).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod dispatch;
pub mod error;
pub mod ext;
pub mod middleware;
pub mod registry;
pub mod router;
pub mod routes;
pub mod state;
pub mod store;

pub use config::AiConfig;
pub use error::AiError;
pub use registry::{Alias, Model, Provider, Registry, ResolveStrategy};
pub use router::router;
pub use state::AiState;

use utoipa::OpenApi;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// OpenAPI documentation for the AI service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor AI API",
        version = "1.0.0",
        description = "OpenAI-compatible LLM gateway API"
    ),
    paths(
        routes::chat::chat_completions,
        routes::embeddings::embeddings,
        routes::models::list_models,
        routes::health::health,
    ),
    components(schemas(
        dispatch::ChatCompletionRequest,
        dispatch::ChatCompletionResponse,
        dispatch::ChatCompletionChunk,
        dispatch::Message,
        dispatch::Choice,
        dispatch::ChunkChoice,
        dispatch::Delta,
        dispatch::Usage,
        dispatch::EmbeddingsRequest,
        dispatch::EmbeddingsResponse,
        dispatch::EmbeddingData,
        dispatch::EmbeddingsUsage,
        routes::models::ModelInfo,
        routes::models::ModelsListResponse,
        routes::health::HealthResponse,
    )),
    tags(
        (name = "ai", description = "AI inference operations"),
        (name = "ai.chat", description = "Chat completions"),
        (name = "ai.embeddings", description = "Text embeddings"),
        (name = "ai.models", description = "Model listing"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

/// Returns the OpenAPI specification for the AI service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
