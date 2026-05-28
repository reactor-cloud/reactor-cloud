//! Migration endpoint.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// Migration response.
#[derive(Debug, Serialize)]
pub struct MigrateResponse {
    pub status: String,
    pub message: String,
}

/// POST /_admin/migrate handler.
///
/// Returns guidance to use the `reactor-server migrate` CLI command.
/// Re-running migrations via HTTP endpoint is disabled for safety.
pub async fn migrate_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(MigrateResponse {
            status: "info".to_string(),
            message: "Use `reactor-server migrate` CLI command to run migrations".to_string(),
        }),
    )
}
