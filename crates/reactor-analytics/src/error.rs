//! Analytics error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Analytics error type.
#[derive(Debug, Error)]
pub enum AnalyticsError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Project key is invalid or revoked.
    #[error("invalid or revoked project key")]
    InvalidProjectKey,

    /// Project not found.
    #[error("project not found")]
    ProjectNotFound,

    /// Unauthorized access.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Forbidden access.
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// Monthly event quota exceeded.
    #[error("monthly event quota exceeded for organization {org_id}: limit is {limit}")]
    QuotaExceeded {
        /// Organization ID.
        org_id: uuid::Uuid,
        /// Quota limit.
        limit: u64,
    },

    /// Rate limit exceeded.
    #[error("rate limit exceeded, please slow down")]
    RateLimited,

    /// Event payload too large.
    #[error("event payload too large: {size} bytes exceeds limit of {limit} bytes")]
    EventTooLarge { size: usize, limit: usize },

    /// Batch too large.
    #[error("batch too large: {count} events or {size} bytes exceeds limits")]
    BatchTooLarge { count: usize, size: usize },

    /// Invalid event name (system reserved).
    #[error("event name '{0}' is reserved for system events")]
    SystemReservedEventName(String),

    /// Query timeout.
    #[error("query timeout after {0}ms")]
    QueryTimeout(u64),

    /// Query time range too wide.
    #[error("query time range too wide: {days} days exceeds limit of {limit} days")]
    QueryRangeTooWide { days: u32, limit: u32 },

    /// Consent denied (DNT/opt-out).
    #[error("consent denied")]
    ConsentDenied,

    /// User or anonymous ID not found for erasure.
    #[error("subject not found for erasure")]
    SubjectNotFound,

    /// Validation error.
    #[error("validation error: {0}")]
    Validation(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
}

/// Error code for API responses.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorCode(pub &'static str);

impl AnalyticsError {
    /// Get the error code for API responses.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Database(_) => ErrorCode("analytics.database_error"),
            Self::InvalidProjectKey => ErrorCode("analytics.project_key.invalid"),
            Self::ProjectNotFound => ErrorCode("analytics.project.not_found"),
            Self::Unauthorized(_) => ErrorCode("analytics.unauthorized"),
            Self::Forbidden(_) => ErrorCode("analytics.forbidden"),
            Self::QuotaExceeded { .. } => ErrorCode("analytics.quota.exceeded"),
            Self::RateLimited => ErrorCode("analytics.rate_limit"),
            Self::EventTooLarge { .. } => ErrorCode("analytics.event.too_large"),
            Self::BatchTooLarge { .. } => ErrorCode("analytics.batch.too_large"),
            Self::SystemReservedEventName(_) => ErrorCode("analytics.event.system_reserved"),
            Self::QueryTimeout(_) => ErrorCode("analytics.query.timeout"),
            Self::QueryRangeTooWide { .. } => ErrorCode("analytics.query.range_too_wide"),
            Self::ConsentDenied => ErrorCode("analytics.consent.denied"),
            Self::SubjectNotFound => ErrorCode("analytics.erasure.subject_not_found"),
            Self::Validation(_) => ErrorCode("analytics.validation_error"),
            Self::Internal(_) => ErrorCode("analytics.internal_error"),
            Self::Config(_) => ErrorCode("analytics.config_error"),
        }
    }

    /// Get the HTTP status code.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Database(_) | Self::Internal(_) | Self::Config(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::InvalidProjectKey | Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::ProjectNotFound | Self::SubjectNotFound => StatusCode::NOT_FOUND,
            Self::QuotaExceeded { .. } | Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::EventTooLarge { .. } | Self::BatchTooLarge { .. } => {
                StatusCode::PAYLOAD_TOO_LARGE
            }
            Self::SystemReservedEventName(_)
            | Self::QueryRangeTooWide { .. }
            | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::QueryTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::ConsentDenied => StatusCode::NO_CONTENT,
        }
    }
}

/// Error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl IntoResponse for AnalyticsError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        if status == StatusCode::NO_CONTENT {
            return status.into_response();
        }

        let body = ErrorResponse {
            error: self.to_string(),
            code: self.code().0.to_string(),
        };

        (status, Json(body)).into_response()
    }
}
