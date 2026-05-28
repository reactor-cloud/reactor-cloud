//! Client error types.

use crate::ApiErrorDetail;

/// Result type for client operations.
pub type ClientResult<T> = Result<T, ClientError>;

/// Errors that can occur when using the Reactor client.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Network error (connection refused, timeout, DNS failure).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Server returned an error response.
    #[error("server error: {code} - {message}")]
    Server {
        code: String,
        message: String,
        hint: Option<String>,
        status: u16,
    },

    /// Failed to parse server response.
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// Request validation failed locally.
    #[error("validation error: {0}")]
    Validation(String),

    /// Authentication failed.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// URL parsing error.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl ClientError {
    /// Create a server error from API error detail.
    pub fn from_api_error(detail: ApiErrorDetail, status: u16) -> Self {
        Self::Server {
            code: detail.code,
            message: detail.message,
            hint: detail.hint,
            status,
        }
    }

    /// Whether this error is a network/connectivity issue.
    pub fn is_network(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    /// Whether this error is an authentication issue.
    pub fn is_auth(&self) -> bool {
        matches!(self, Self::Auth(_))
            || matches!(self, Self::Server { status, .. } if *status == 401 || *status == 403)
    }

    /// Whether this error is a server-side issue (5xx).
    pub fn is_server_error(&self) -> bool {
        matches!(self, Self::Server { status, .. } if *status >= 500)
    }

    /// Whether this error is retriable.
    pub fn is_retriable(&self) -> bool {
        self.is_network() || self.is_server_error()
    }
}
