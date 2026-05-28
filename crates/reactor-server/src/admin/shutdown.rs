//! Shutdown endpoint.

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Serialize;

use crate::boot::ShutdownHandle;

/// Shutdown response.
#[derive(Debug, Serialize)]
pub struct ShutdownResponse {
    pub status: String,
    pub message: String,
}

/// POST /_admin/shutdown handler.
///
/// Triggers graceful shutdown of the server.
pub async fn shutdown_handler(
    Extension(shutdown): Extension<ShutdownHandle>,
) -> impl IntoResponse {
    tracing::info!("shutdown requested via /_admin/shutdown");
    shutdown.shutdown();

    (
        StatusCode::ACCEPTED,
        Json(ShutdownResponse {
            status: "accepted".to_string(),
            message: "shutdown initiated".to_string(),
        }),
    )
}
