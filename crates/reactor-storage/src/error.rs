//! Storage error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Storage error type.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Object not found.
    #[error("Object not found: {0}")]
    NotFound(String),

    /// Bucket not found.
    #[error("Bucket not found: {0}")]
    BucketNotFound(String),

    /// Bucket already exists.
    #[error("Bucket already exists: {0}")]
    BucketExists(String),

    /// Permission denied.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Authentication required.
    #[error("Authentication required")]
    AuthRequired,

    /// Invalid signature.
    #[error("Invalid signature")]
    InvalidSignature,

    /// Signature expired.
    #[error("Signature expired")]
    SignatureExpired,

    /// Object too large.
    #[error("Object too large: {size} bytes exceeds limit of {limit} bytes")]
    TooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// Database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for StorageError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            StorageError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            StorageError::BucketNotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            StorageError::BucketExists(msg) => (StatusCode::CONFLICT, msg.clone()),
            StorageError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            StorageError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            StorageError::AuthRequired => {
                (StatusCode::UNAUTHORIZED, "Authentication required".into())
            }
            StorageError::InvalidSignature => (StatusCode::FORBIDDEN, "Invalid signature".into()),
            StorageError::SignatureExpired => (StatusCode::FORBIDDEN, "Signature expired".into()),
            StorageError::TooLarge { size, limit } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Object size {} exceeds limit {}", size, limit),
            ),
            StorageError::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into())
            }
            StorageError::Io(e) => {
                tracing::error!(error = %e, "IO error");
                (StatusCode::INTERNAL_SERVER_ERROR, "IO error".into())
            }
            StorageError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
            }
        };

        let body = Json(json!({
            "error": message,
        }));

        (status, body).into_response()
    }
}
