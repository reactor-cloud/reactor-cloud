//! Chat completions endpoint.

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    Extension, Json,
};
use futures::Stream;
use std::convert::Infallible;
use std::pin::Pin;

use crate::dispatch::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse};
use crate::error::AiError;
use crate::middleware::AiCtx;
use crate::state::AiState;

/// Chat completions endpoint.
#[utoipa::path(
    post,
    path = "/ai/v1/chat/completions",
    tag = "ai.chat",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Chat completion response", body = ChatCompletionResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Model not found"),
        (status = 502, description = "Upstream provider error"),
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn chat_completions(
    State(state): State<AiState>,
    ai_ctx: Option<Extension<AiCtx>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, AiError> {
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

    // Check if streaming
    if request.stream.unwrap_or(false) {
        // Streaming response
        let (stream, _start) = provider
            .chat_completion_stream(&request, &resolved_model)
            .await?;

        let sse_stream = stream_to_sse(stream);
        let sse = Sse::new(sse_stream).keep_alive(KeepAlive::default());

        Ok(sse.into_response())
    } else {
        // Non-streaming response
        let (response, _duration) = provider.chat_completion(&request, &resolved_model).await?;

        // Run post-usage extension hook
        if let Some(ref usage) = response.usage {
            let event = crate::ext::UsageEvent {
                model_id: model.id.clone(),
                user_id,
                tokens_in: usage.prompt_tokens,
                tokens_out: usage.completion_tokens,
            };
            state.extensions.post_usage(&event).await?;
        }

        Ok(Json(response).into_response())
    }
}

type SseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

fn stream_to_sse(
    stream: Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, AiError>> + Send>>,
) -> SseStream {
    use futures::StreamExt;

    let mapped = stream.map(|result| match result {
        Ok(chunk) => {
            let data = serde_json::to_string(&chunk).unwrap_or_default();
            Ok(Event::default().data(data))
        }
        Err(e) => {
            let error_data = format!(r#"{{"error": "{}"}}"#, e);
            Ok(Event::default().data(error_data))
        }
    });

    Box::pin(mapped)
}
