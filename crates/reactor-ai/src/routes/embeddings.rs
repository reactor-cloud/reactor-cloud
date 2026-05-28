//! Embeddings endpoint.

use axum::{extract::State, Extension, Json};

use crate::dispatch::{EmbeddingsRequest, EmbeddingsResponse};
use crate::error::AiError;
use crate::middleware::AiCtx;
use crate::state::AiState;

/// Embeddings endpoint.
#[utoipa::path(
    post,
    path = "/ai/v1/embeddings",
    tag = "ai.embeddings",
    request_body = EmbeddingsRequest,
    responses(
        (status = 200, description = "Embeddings response", body = EmbeddingsResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Model not found"),
        (status = 502, description = "Upstream provider error"),
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn embeddings(
    State(state): State<AiState>,
    ai_ctx: Option<Extension<AiCtx>>,
    Json(request): Json<EmbeddingsRequest>,
) -> Result<Json<EmbeddingsResponse>, AiError> {
    // Extract user_id from AiCtx if available (for metering)
    let user_id = ai_ctx.as_ref().and_then(|ctx| ctx.user_id());

    // Resolve model/alias
    let (model, resolved_model) = state
        .registry
        .resolve(&request.model)
        .ok_or_else(|| AiError::ModelNotFound(request.model.clone()))?;

    // Get the provider
    let provider = state
        .get_provider(&model.provider)
        .ok_or_else(|| AiError::NoProvidersAvailable(model.provider.clone()))?;

    // Run pre-request extension hook
    let ctx = crate::ext::RequestCtx {
        model_id: model.id.clone(),
        user_id: user_id.clone(),
    };
    state.extensions.pre_request(&ctx).await?;

    // Make the embeddings request
    let (response, _duration) = provider.embeddings(&request, &resolved_model).await?;

    // Run post-usage extension hook
    let event = crate::ext::UsageEvent {
        model_id: model.id.clone(),
        user_id,
        tokens_in: response.usage.prompt_tokens,
        tokens_out: 0,
    };
    state.extensions.post_usage(&event).await?;

    Ok(Json(response))
}
