//! Error handling for HTTP responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use reactor_core::auth::AuthError;
use reactor_core::error::ErrorResponse;

/// Convert an AuthError into an HTTP response.
fn auth_error_response(e: &AuthError) -> Response {
    let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let response = ErrorResponse::new(e.code(), e.to_string(), e.status_code());
    (status, Json(response)).into_response()
}

/// Internal error type that wraps various error sources.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Authentication error.
    #[error(transparent)]
    Auth(#[from] AuthError),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// Resource not found.
    #[error("not found: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::Auth(e) => auth_error_response(&e),
            Self::Database(e) => {
                tracing::error!(error = %e, "database error");
                let response = ErrorResponse::new("internal_error", "Internal server error", 500);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
            }
            Self::Config(msg) => {
                tracing::error!(error = %msg, "configuration error");
                let response = ErrorResponse::new("internal_error", "Internal server error", 500);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
            }
            Self::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                let response = ErrorResponse::new("internal_error", "Internal server error", 500);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
            }
            Self::NotFound(msg) => {
                let response = ErrorResponse::new("not_found", msg, 404);
                (StatusCode::NOT_FOUND, Json(response)).into_response()
            }
        }
    }
}

/// Result type alias for route handlers.
pub type AppResult<T> = Result<T, AppError>;
