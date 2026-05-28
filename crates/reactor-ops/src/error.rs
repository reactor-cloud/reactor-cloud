//! Error types for the ops control surface.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Error type for ops operations.
#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    /// Request came from untrusted network.
    #[error("access denied: untrusted network")]
    UntrustedNetwork,

    /// No authentication provided.
    #[error("authentication required")]
    AuthenticationRequired,

    /// Invalid or expired token.
    #[error("invalid or expired token")]
    InvalidToken,

    /// Missing required scope.
    #[error("access denied: missing scope '{0}'")]
    MissingScope(String),

    /// Step-up authentication required.
    #[error("step-up authentication required")]
    StepUpRequired,

    /// Resource not found.
    #[error("resource not found")]
    NotFound,

    /// Validation error.
    #[error("validation error: {0}")]
    Validation(String),

    /// Internal error.
    #[error("internal error")]
    Internal,

    /// Auth service error.
    #[error("auth error: {0}")]
    Auth(#[from] reactor_core::auth::AuthError),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error code.
    pub error: String,
    /// Human-readable message.
    pub message: String,
    /// Additional details (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for OpsError {
    fn into_response(self) -> Response {
        let (status, error, message) = match &self {
            OpsError::UntrustedNetwork => (
                StatusCode::FORBIDDEN,
                "untrusted_network",
                self.to_string(),
            ),
            OpsError::AuthenticationRequired => (
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                self.to_string(),
            ),
            OpsError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                self.to_string(),
            ),
            OpsError::MissingScope(scope) => (
                StatusCode::FORBIDDEN,
                "missing_scope",
                format!("Access denied: missing required scope '{}'", scope),
            ),
            OpsError::StepUpRequired => (
                StatusCode::FORBIDDEN,
                "step_up_required",
                self.to_string(),
            ),
            OpsError::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                self.to_string(),
            ),
            OpsError::Validation(msg) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                msg.clone(),
            ),
            OpsError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "An internal error occurred".to_string(),
            ),
            OpsError::Auth(e) => {
                let (status, error) = match e {
                    reactor_core::auth::AuthError::InvalidCredentials => {
                        (StatusCode::UNAUTHORIZED, "invalid_credentials")
                    }
                    reactor_core::auth::AuthError::InvalidToken => {
                        (StatusCode::UNAUTHORIZED, "invalid_token")
                    }
                    reactor_core::auth::AuthError::PermissionDenied => {
                        (StatusCode::FORBIDDEN, "permission_denied")
                    }
                    reactor_core::auth::AuthError::UserNotFound => {
                        (StatusCode::NOT_FOUND, "user_not_found")
                    }
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, "auth_error"),
                };
                (status, error, e.to_string())
            }
            OpsError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                "A database error occurred".to_string(),
            ),
        };

        let body = ErrorResponse {
            error: error.to_string(),
            message,
            details: None,
        };

        // Add WWW-Authenticate header for step-up required
        if matches!(self, OpsError::StepUpRequired) {
            let mut response = (status, Json(body)).into_response();
            response.headers_mut().insert(
                "WWW-Authenticate",
                "WebAuthn realm=\"reactor-ops\"".parse().unwrap(),
            );
            return response;
        }

        (status, Json(body)).into_response()
    }
}
