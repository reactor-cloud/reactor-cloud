//! Error types for reactor-data.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// Data capability error.
#[derive(Debug, Error)]
pub enum DataError {
    #[error("unauthorized")]
    Unauthorized,

    #[error("permission denied")]
    PermissionDenied,

    #[error("table not found: {0}")]
    TableNotFound(String),

    #[error("column not found: {0}")]
    ColumnNotFound(String),

    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("invalid filter: {0}")]
    InvalidFilter(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("ambiguous embed '{name}': multiple FK relationships found. Disambiguate with: {hints:?}")]
    AmbiguousEmbed { name: String, hints: Vec<String> },

    #[error("database error")]
    Database,

    #[error("internal error")]
    Internal,

    #[error("auth error: {0}")]
    Auth(#[from] reactor_core::auth::AuthError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("policy store error: {0}")]
    PolicyStore(String),
}

impl DataError {
    /// Get the stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::PermissionDenied => "permission_denied",
            Self::TableNotFound(_) => "table_not_found",
            Self::ColumnNotFound(_) => "column_not_found",
            Self::PolicyDenied(_) => "policy_denied",
            Self::InvalidFilter(_) => "invalid_filter",
            Self::InvalidQuery(_) => "invalid_query",
            Self::AmbiguousEmbed { .. } => "ambiguous_embed",
            Self::Database => "database_error",
            Self::Internal => "internal_error",
            Self::Auth(_) => "auth_error",
            Self::Config(_) => "config_error",
            Self::PolicyStore(_) => "policy_store_error",
        }
    }

    /// Get the HTTP status code.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Unauthorized | Self::Auth(_) => 401,
            Self::PermissionDenied | Self::PolicyDenied(_) => 403,
            Self::TableNotFound(_) => 404,
            Self::ColumnNotFound(_)
            | Self::InvalidFilter(_)
            | Self::InvalidQuery(_)
            | Self::AmbiguousEmbed { .. }
            | Self::Config(_) => 400,
            Self::Database | Self::Internal | Self::PolicyStore(_) => 500,
        }
    }
}

impl From<crate::policy::PolicyStoreError> for DataError {
    fn from(err: crate::policy::PolicyStoreError) -> Self {
        DataError::PolicyStore(err.to_string())
    }
}

/// Error response envelope.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

/// Error body.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for DataError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let body = ErrorResponse {
            error: ErrorBody {
                code: self.code().to_string(),
                message: self.to_string(),
                status: self.status_code(),
                request_id: None,
                details: None,
            },
        };

        (status, Json(body)).into_response()
    }
}
