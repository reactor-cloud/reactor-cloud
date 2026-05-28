use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotResponse {
    pub success: bool,
    pub path: Option<String>,
    pub error: Option<String>,
}

pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/get-state", get(get_state))
        .route("/screenshot", get(screenshot))
        .route("/windows/close-others", axum::routing::post(close_other_windows))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_ms: 0, // TODO: track actual uptime
    })
}

async fn get_state(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.get_snapshot().await;
    Json(snapshot)
}

async fn screenshot(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Implement with Tauri WebviewWindow::capture when tauri-integration feature is enabled
    Json(ScreenshotResponse {
        success: false,
        path: None,
        error: Some("Screenshot not implemented yet".to_string()),
    })
}

async fn close_other_windows(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Implement window management when tauri-integration feature is enabled
    (StatusCode::OK, Json(serde_json::json!({ "closed": 0 })))
}
