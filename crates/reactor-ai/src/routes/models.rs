//! Models listing endpoint.

use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AiState;

/// Model information (OpenAI-compatible).
#[derive(Debug, Serialize, ToSchema)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,
    /// Object type (always "model").
    pub object: String,
    /// Created timestamp.
    pub created: i64,
    /// Owner/provider.
    pub owned_by: String,
}

/// Response for listing models.
#[derive(Debug, Serialize, ToSchema)]
pub struct ModelsListResponse {
    /// Object type (always "list").
    pub object: String,
    /// List of models.
    pub data: Vec<ModelInfo>,
}

/// List available models.
#[utoipa::path(
    get,
    path = "/ai/v1/models",
    tag = "ai.models",
    responses(
        (status = 200, description = "List of available models", body = ModelsListResponse),
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn list_models(State(state): State<AiState>) -> Json<ModelsListResponse> {
    let models: Vec<ModelInfo> = state
        .registry
        .models()
        .map(|m| ModelInfo {
            id: m.id.clone(),
            object: "model".to_string(),
            created: 0,
            owned_by: m.provider.clone(),
        })
        .collect();

    Json(ModelsListResponse {
        object: "list".to_string(),
        data: models,
    })
}
