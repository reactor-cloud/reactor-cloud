//! Error handling for the AI capability.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// API error response body (OpenAI-compatible format).
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    /// Error details.
    pub error: ApiErrorBody,
}

/// API error details.
#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    /// Error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Error type (always "api_error").
    #[serde(rename = "type")]
    pub error_type: String,
}

/// AI capability error types.
#[derive(Debug, Error)]
pub enum AiError {
    /// Authentication failed.
    #[error("Authentication required")]
    Unauthorized,

    /// Model not found in registry.
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Alias could not be resolved.
    #[error("Alias could not be resolved: {0}")]
    AliasResolutionFailed(String),

    /// No providers available for the requested model.
    #[error("No providers available for model: {0}")]
    NoProvidersAvailable(String),

    /// Bad request (invalid input).
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Upstream provider returned an error.
    #[error("Upstream error: {0}")]
    UpstreamError(String),

    /// Upstream provider timed out.
    #[error("Upstream timeout")]
    UpstreamTimeout,

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Registry loading error.
    #[error("Registry error: {0}")]
    Registry(String),
}

impl AiError {
    /// Get the HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            AiError::Unauthorized => StatusCode::UNAUTHORIZED,
            AiError::ModelNotFound(_) => StatusCode::NOT_FOUND,
            AiError::AliasResolutionFailed(_) => StatusCode::NOT_FOUND,
            AiError::NoProvidersAvailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AiError::UpstreamError(_) => StatusCode::BAD_GATEWAY,
            AiError::UpstreamTimeout => StatusCode::GATEWAY_TIMEOUT,
            AiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AiError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AiError::Registry(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error code for API responses.
    pub fn error_code(&self) -> &'static str {
        match self {
            AiError::Unauthorized => "unauthorized",
            AiError::ModelNotFound(_) => "model_not_found",
            AiError::AliasResolutionFailed(_) => "alias_resolution_failed",
            AiError::NoProvidersAvailable(_) => "no_providers_available",
            AiError::BadRequest(_) => "bad_request",
            AiError::UpstreamError(_) => "upstream_error",
            AiError::UpstreamTimeout => "upstream_timeout",
            AiError::Internal(_) => "internal_error",
            AiError::Config(_) => "config_error",
            AiError::Registry(_) => "registry_error",
        }
    }
}

impl IntoResponse for AiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = ApiErrorResponse {
            error: ApiErrorBody {
                code: self.error_code().to_string(),
                message: self.to_string(),
                error_type: "api_error".to_string(),
            },
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for AI operations.
pub type AiResult<T> = Result<T, AiError>;

/// Check if an error is eligible for fallback to another provider.
pub fn is_fallback_eligible(err: &AiError) -> bool {
    match err {
        AiError::UpstreamError(msg) => {
            msg.contains("500")
                || msg.contains("502")
                || msg.contains("503")
                || msg.contains("504")
                || msg.contains("429")
                || msg.contains("throttl")
                || msg.contains("timeout")
                || msg.contains("timed out")
                || msg.contains("connection")
                || msg.contains("Connection")
        }
        AiError::UpstreamTimeout => true,
        _ => false,
    }
}
