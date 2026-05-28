//! Base error types for Reactor.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Base error type for all Reactor operations.
#[derive(Debug, Error)]
pub enum ReactorError {
    /// Authentication error.
    #[error("auth error: {0}")]
    Auth(#[from] crate::auth::AuthError),

    /// Internal error that should not be exposed to clients.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Error response envelope returned by all Reactor HTTP endpoints.
///
/// This provides a consistent error format across all capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// The error details.
    pub error: ErrorDetails,
}

/// Details of an error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// Stable error code (snake_case). Clients should switch on this, not status.
    pub code: String,

    /// Human-readable error message.
    pub message: String,

    /// HTTP status code.
    pub status: u16,

    /// Request ID for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ErrorResponse {
    /// Create a new error response.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, status: u16) -> Self {
        Self {
            error: ErrorDetails {
                code: code.into(),
                message: message.into(),
                status,
                request_id: None,
            },
        }
    }

    /// Add a request ID to this error response.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.error.request_id = Some(request_id.into());
        self
    }
}
