//! Functions error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Functions error type.
#[derive(Debug, Error)]
pub enum FunctionsError {
    /// Function not found.
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    /// Deployment not found.
    #[error("Deployment not found: {0}")]
    DeploymentNotFound(String),

    /// Deployment not ready (cold start in progress or failed).
    #[error("Deployment not ready")]
    DeploymentNotReady,

    /// Function timeout.
    #[error("Function timeout after {duration_ms}ms")]
    FunctionTimeout {
        /// Function name.
        function: String,
        /// Deployment ID.
        deployment_id: String,
        /// Duration before timeout in milliseconds.
        duration_ms: u64,
    },

    /// Invocation timeout (generic).
    #[error("Invocation timeout")]
    InvocationTimeout,

    /// Function crashed.
    #[error("Function crashed: {0}")]
    FunctionCrashed(String),

    /// Payload too large.
    #[error("Payload too large: {size} bytes exceeds limit of {limit} bytes")]
    PayloadTooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// Response too large.
    #[error("Response too large: exceeded limit of {limit} bytes")]
    ResponseTooLarge {
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// Request body too large.
    #[error("Request body too large: {size} bytes exceeds limit of {max} bytes")]
    RequestBodyTooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        max: u64,
    },

    /// Too many concurrent requests.
    #[error("Too many requests: concurrency limit reached")]
    TooManyRequests {
        /// Seconds to wait before retrying.
        retry_after: u32,
    },

    /// Runtime error (adapter failure).
    #[error("Runtime error: {0}")]
    RuntimeError(String),

    /// Bundle invalid.
    #[error("Invalid bundle: {0}")]
    BundleInvalid(String),

    /// Manifest invalid.
    #[error("Invalid manifest: {0}")]
    ManifestInvalid(String),

    /// Bundle too large.
    #[error("Bundle too large: {size} bytes exceeds limit of {limit} bytes")]
    BundleTooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// Policy denied.
    #[error("Policy denied: {0}")]
    PolicyDenied(String),

    /// Unsupported runtime.
    #[error("Unsupported runtime: {0}")]
    UnsupportedRuntime(String),

    /// Invalid env key.
    #[error("Invalid env key: {0}")]
    EnvKeyInvalid(String),

    /// Cold start failed.
    #[error("Cold start failed: {0}")]
    ColdStartFailed(String),

    /// Permission denied.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Authentication required.
    #[error("Authentication required")]
    AuthRequired,

    /// Organization required.
    #[error("Organization context required")]
    OrgRequired,

    /// Function already exists.
    #[error("Function already exists: {0}")]
    FunctionExists(String),

    /// Invalid function name.
    #[error("Invalid function name: {0}")]
    InvalidFunctionName(String),

    /// Database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Storage error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl FunctionsError {
    /// Get the error code as a string.
    pub fn code(&self) -> &'static str {
        match self {
            FunctionsError::FunctionNotFound(_) => "function_not_found",
            FunctionsError::DeploymentNotFound(_) => "deployment_not_found",
            FunctionsError::DeploymentNotReady => "deployment_not_ready",
            FunctionsError::FunctionTimeout { .. } => "function_timeout",
            FunctionsError::InvocationTimeout => "invocation_timeout",
            FunctionsError::FunctionCrashed(_) => "function_crashed",
            FunctionsError::PayloadTooLarge { .. } => "payload_too_large",
            FunctionsError::ResponseTooLarge { .. } => "response_too_large",
            FunctionsError::RequestBodyTooLarge { .. } => "request_body_too_large",
            FunctionsError::TooManyRequests { .. } => "too_many_requests",
            FunctionsError::RuntimeError(_) => "runtime_error",
            FunctionsError::BundleInvalid(_) => "bundle_invalid",
            FunctionsError::ManifestInvalid(_) => "manifest_invalid",
            FunctionsError::BundleTooLarge { .. } => "bundle_too_large",
            FunctionsError::PolicyDenied(_) => "policy_denied",
            FunctionsError::UnsupportedRuntime(_) => "unsupported_runtime",
            FunctionsError::EnvKeyInvalid(_) => "env_key_invalid",
            FunctionsError::ColdStartFailed(_) => "cold_start_failed",
            FunctionsError::PermissionDenied(_) => "permission_denied",
            FunctionsError::InvalidRequest(_) => "invalid_request",
            FunctionsError::AuthRequired => "auth_required",
            FunctionsError::OrgRequired => "org_required",
            FunctionsError::FunctionExists(_) => "function_exists",
            FunctionsError::InvalidFunctionName(_) => "invalid_function_name",
            FunctionsError::Database(_) => "database_error",
            FunctionsError::Io(_) => "io_error",
            FunctionsError::Storage(_) => "storage_error",
            FunctionsError::Internal(_) => "internal_error",
        }
    }
}

impl IntoResponse for FunctionsError {
    fn into_response(self) -> Response {
        let (status, retry_after) = match &self {
            FunctionsError::FunctionNotFound(_) => (StatusCode::NOT_FOUND, None),
            FunctionsError::DeploymentNotFound(_) => (StatusCode::NOT_FOUND, None),
            FunctionsError::DeploymentNotReady => (StatusCode::SERVICE_UNAVAILABLE, Some(1)),
            FunctionsError::FunctionTimeout { .. } => (StatusCode::REQUEST_TIMEOUT, None),
            FunctionsError::InvocationTimeout => (StatusCode::REQUEST_TIMEOUT, None),
            FunctionsError::FunctionCrashed(_) => (StatusCode::INTERNAL_SERVER_ERROR, None),
            FunctionsError::PayloadTooLarge { .. } => (StatusCode::PAYLOAD_TOO_LARGE, None),
            FunctionsError::ResponseTooLarge { .. } => (StatusCode::INTERNAL_SERVER_ERROR, None),
            FunctionsError::RequestBodyTooLarge { .. } => (StatusCode::PAYLOAD_TOO_LARGE, None),
            FunctionsError::TooManyRequests { retry_after } => {
                (StatusCode::TOO_MANY_REQUESTS, Some(*retry_after))
            }
            FunctionsError::RuntimeError(_) => (StatusCode::BAD_GATEWAY, None),
            FunctionsError::BundleInvalid(_) => (StatusCode::BAD_REQUEST, None),
            FunctionsError::ManifestInvalid(_) => (StatusCode::BAD_REQUEST, None),
            FunctionsError::BundleTooLarge { .. } => (StatusCode::PAYLOAD_TOO_LARGE, None),
            FunctionsError::PolicyDenied(_) => (StatusCode::FORBIDDEN, None),
            FunctionsError::UnsupportedRuntime(_) => (StatusCode::BAD_REQUEST, None),
            FunctionsError::EnvKeyInvalid(_) => (StatusCode::BAD_REQUEST, None),
            FunctionsError::ColdStartFailed(_) => (StatusCode::SERVICE_UNAVAILABLE, Some(5)),
            FunctionsError::PermissionDenied(_) => (StatusCode::FORBIDDEN, None),
            FunctionsError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, None),
            FunctionsError::AuthRequired => (StatusCode::UNAUTHORIZED, None),
            FunctionsError::OrgRequired => (StatusCode::BAD_REQUEST, None),
            FunctionsError::FunctionExists(_) => (StatusCode::CONFLICT, None),
            FunctionsError::InvalidFunctionName(_) => (StatusCode::BAD_REQUEST, None),
            FunctionsError::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (StatusCode::INTERNAL_SERVER_ERROR, None)
            }
            FunctionsError::Io(e) => {
                tracing::error!(error = %e, "IO error");
                (StatusCode::INTERNAL_SERVER_ERROR, None)
            }
            FunctionsError::Storage(e) => {
                tracing::error!(error = %e, "Storage error");
                (StatusCode::INTERNAL_SERVER_ERROR, None)
            }
            FunctionsError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, None)
            }
        };

        let code = self.code();
        let message = self.to_string();

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
                "status": status.as_u16(),
            }
        }));

        let mut response = (status, body).into_response();

        if let Some(retry_secs) = retry_after {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                retry_secs.to_string().parse().unwrap(),
            );
        }

        response
    }
}
