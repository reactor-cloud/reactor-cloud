//! Jobs error types.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur in jobs operations.
#[derive(Debug, Error)]
pub enum JobsError {
    /// Job not found.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// Trigger not found.
    #[error("trigger not found: {0}")]
    TriggerNotFound(String),

    /// Run not found.
    #[error("run not found: {0}")]
    RunNotFound(String),

    /// Step not found.
    #[error("step not found: {0}")]
    StepNotFound(String),

    /// Run already complete.
    #[error("run already complete: {0}")]
    RunAlreadyComplete(String),

    /// Run cancelled.
    #[error("run cancelled: {0}")]
    RunCancelled(String),

    /// Invalid cron expression.
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),

    /// Invalid trigger configuration.
    #[error("invalid trigger config: {0}")]
    InvalidTriggerConfig(String),

    /// Step failed.
    #[error("step failed: {0}")]
    StepFailed(String),

    /// Max attempts exceeded.
    #[error("max attempts exceeded for job {job}: {attempts} attempts")]
    MaxAttemptsExceeded {
        /// Job name.
        job: String,
        /// Number of attempts.
        attempts: u32,
    },

    /// Concurrency exceeded.
    #[error("concurrency exceeded for job {job}")]
    ConcurrencyExceeded {
        /// Job name.
        job: String,
    },

    /// Webhook token invalid.
    #[error("webhook token invalid")]
    WebhookTokenInvalid,

    /// Payload too large.
    #[error("payload too large: {size} bytes (max {max})")]
    PayloadTooLarge {
        /// Actual payload size.
        size: u64,
        /// Maximum allowed size.
        max: u64,
    },

    /// Policy denied.
    #[error("policy denied: {0}")]
    PolicyDenied(String),

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Missing org context.
    #[error("missing org context")]
    MissingOrgContext,

    /// Invalid job name.
    #[error("invalid job name: {0}")]
    InvalidJobName(String),

    /// Invalid event topic.
    #[error("invalid event topic: {0}")]
    InvalidEventTopic(String),

    /// Function not found.
    #[error("function not found: {0}")]
    FunctionNotFound(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Cache error.
    #[error("cache error: {0}")]
    Cache(#[from] reactor_cache::CacheError),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl JobsError {
    /// Get the error code string.
    pub fn code(&self) -> &'static str {
        match self {
            Self::JobNotFound(_) => "job_not_found",
            Self::TriggerNotFound(_) => "trigger_not_found",
            Self::RunNotFound(_) => "run_not_found",
            Self::StepNotFound(_) => "step_not_found",
            Self::RunAlreadyComplete(_) => "run_already_complete",
            Self::RunCancelled(_) => "run_cancelled",
            Self::InvalidCron(_) => "invalid_cron",
            Self::InvalidTriggerConfig(_) => "invalid_trigger_config",
            Self::StepFailed(_) => "step_failed",
            Self::MaxAttemptsExceeded { .. } => "max_attempts_exceeded",
            Self::ConcurrencyExceeded { .. } => "concurrency_exceeded",
            Self::WebhookTokenInvalid => "webhook_token_invalid",
            Self::PayloadTooLarge { .. } => "payload_too_large",
            Self::PolicyDenied(_) => "policy_denied",
            Self::PermissionDenied(_) => "permission_denied",
            Self::MissingOrgContext => "missing_org_context",
            Self::InvalidJobName(_) => "invalid_job_name",
            Self::InvalidEventTopic(_) => "invalid_event_topic",
            Self::FunctionNotFound(_) => "function_not_found",
            Self::Database(_) => "database_error",
            Self::Cache(_) => "cache_error",
            Self::Serialization(_) => "serialization_error",
            Self::Internal(_) => "internal_error",
        }
    }

    /// Get the HTTP status code.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::JobNotFound(_)
            | Self::TriggerNotFound(_)
            | Self::RunNotFound(_)
            | Self::StepNotFound(_)
            | Self::FunctionNotFound(_)
            | Self::WebhookTokenInvalid => StatusCode::NOT_FOUND,

            Self::InvalidCron(_)
            | Self::InvalidTriggerConfig(_)
            | Self::InvalidJobName(_)
            | Self::InvalidEventTopic(_)
            | Self::PayloadTooLarge { .. } => StatusCode::BAD_REQUEST,

            Self::RunAlreadyComplete(_) | Self::RunCancelled(_) => StatusCode::CONFLICT,

            Self::PermissionDenied(_) | Self::PolicyDenied(_) => StatusCode::FORBIDDEN,

            Self::MissingOrgContext => StatusCode::UNAUTHORIZED,

            Self::ConcurrencyExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,

            Self::MaxAttemptsExceeded { .. } | Self::StepFailed(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }

            Self::Database(_) | Self::Cache(_) | Self::Serialization(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    status: u16,
}

impl IntoResponse for JobsError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = ErrorResponse {
            error: ErrorBody {
                code: self.code(),
                message: self.to_string(),
                status: status.as_u16(),
            },
        };
        (status, Json(body)).into_response()
    }
}
