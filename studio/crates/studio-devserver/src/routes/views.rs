use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenViewRequest {
    pub view_id: String,
    pub title: Option<String>,
    pub document_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenViewResponse {
    pub success: bool,
    pub tab_id: Option<String>,
    pub error: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/open-view", post(open_view))
}

async fn open_view(
    State(_state): State<AppState>,
    Json(_request): Json<OpenViewRequest>,
) -> impl IntoResponse {
    // TODO: Actually open the view via Tauri when tauri-integration is enabled
    let tab_id = uuid::Uuid::new_v4().to_string();

    (
        StatusCode::OK,
        Json(OpenViewResponse {
            success: true,
            tab_id: Some(tab_id),
            error: None,
        }),
    )
}
