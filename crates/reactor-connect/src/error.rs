//! Connect error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for the Connect service.
#[derive(Debug, Error)]
pub enum ConnectError {
    // Auth errors
    /// Missing or invalid authentication.
    #[error("authentication required")]
    Unauthorized,

    /// Missing org context.
    #[error("org context required")]
    MissingOrgContext,

    /// Permission denied.
    #[error("permission denied: {0}")]
    Forbidden(String),

    // Instance errors
    /// Instance not found.
    #[error("instance not found: {0}")]
    InstanceNotFound(String),

    /// Instance already exists.
    #[error("instance already exists: {0}")]
    InstanceAlreadyExists(String),

    /// Invalid instance name.
    #[error("invalid instance name: {0}")]
    InvalidInstanceName(String),

    // Connector errors
    /// Connector type not found.
    #[error("connector type not found: {0}")]
    ConnectorTypeNotFound(String),

    /// Connector check failed.
    #[error("connector check failed: {cause}")]
    ConnectorCheckFailed {
        /// Error cause.
        cause: String,
        /// Suggested fix.
        suggested_fix: Option<String>,
    },

    /// Connector action not found.
    #[error("action not found: {0}")]
    ActionNotFound(String),

    /// Connector action failed.
    #[error("action failed: {cause}")]
    ActionFailed {
        /// Error code.
        code: String,
        /// Error cause.
        cause: String,
        /// Suggested fix.
        suggested_fix: Option<String>,
    },

    /// Dry run not supported.
    #[error("dry run not supported for action: {0}")]
    DryRunNotSupported(String),

    // Credential errors
    /// Credentials not configured.
    #[error("credentials not configured for instance: {0}")]
    CredentialsNotConfigured(String),

    /// Credentials expired.
    #[error("credentials expired for instance: {0}")]
    CredentialsExpired(String),

    /// OAuth callback failed.
    #[error("OAuth callback failed: {0}")]
    OAuthCallbackFailed(String),

    // Connection errors
    /// Connection not found.
    #[error("connection not found: {0}")]
    ConnectionNotFound(String),

    /// Connection already exists.
    #[error("connection already exists: {0}")]
    ConnectionAlreadyExists(String),

    // Receiver errors
    /// Receiver not found.
    #[error("receiver not found")]
    ReceiverNotFound,

    /// Receiver disabled.
    #[error("receiver disabled")]
    ReceiverDisabled,

    /// Webhook signature verification failed.
    #[error("webhook signature verification failed")]
    WebhookSignatureInvalid,

    /// Webhook replay detected.
    #[error("webhook replay detected")]
    WebhookReplayDetected,

    // Sandbox errors
    /// Sandbox run failed.
    #[error("sandbox run failed: {0}")]
    SandboxFailed(String),

    /// Invalid promote token.
    #[error("invalid or expired promote token")]
    InvalidPromoteToken,

    // Drift errors
    /// Drift event not found.
    #[error("drift event not found: {0}")]
    DriftEventNotFound(String),

    // Validation errors
    /// Invalid input.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Schema validation failed.
    #[error("schema validation failed: {0}")]
    SchemaValidationFailed(String),

    // Rate limiting
    /// Rate limit exceeded.
    #[error("rate limit exceeded")]
    RateLimitExceeded,

    // Internal errors
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Vault error.
    #[error("vault error: {0}")]
    Vault(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// HTTP client error.
    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Structured error response for API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code (machine-readable).
    pub code: String,
    /// Error message (human-readable).
    pub message: String,
    /// Suggested fix (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    /// Documentation URL (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
}

impl ConnectError {
    /// Get the error code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::MissingOrgContext => "missing_org_context",
            Self::Forbidden(_) => "forbidden",
            Self::InstanceNotFound(_) => "instance_not_found",
            Self::InstanceAlreadyExists(_) => "instance_already_exists",
            Self::InvalidInstanceName(_) => "invalid_instance_name",
            Self::ConnectorTypeNotFound(_) => "connector_type_not_found",
            Self::ConnectorCheckFailed { .. } => "connector_check_failed",
            Self::ActionNotFound(_) => "action_not_found",
            Self::ActionFailed { code, .. } => {
                // Use the inner code if available, but we can't return it as &'static str
                // So we return a generic code
                let _ = code;
                "action_failed"
            }
            Self::DryRunNotSupported(_) => "dry_run_not_supported",
            Self::CredentialsNotConfigured(_) => "credentials_not_configured",
            Self::CredentialsExpired(_) => "credentials_expired",
            Self::OAuthCallbackFailed(_) => "oauth_callback_failed",
            Self::ConnectionNotFound(_) => "connection_not_found",
            Self::ConnectionAlreadyExists(_) => "connection_already_exists",
            Self::ReceiverNotFound => "receiver_not_found",
            Self::ReceiverDisabled => "receiver_disabled",
            Self::WebhookSignatureInvalid => "webhook_signature_invalid",
            Self::WebhookReplayDetected => "webhook_replay_detected",
            Self::SandboxFailed(_) => "sandbox_failed",
            Self::InvalidPromoteToken => "invalid_promote_token",
            Self::DriftEventNotFound(_) => "drift_event_not_found",
            Self::InvalidInput(_) => "invalid_input",
            Self::SchemaValidationFailed(_) => "schema_validation_failed",
            Self::RateLimitExceeded => "rate_limit_exceeded",
            Self::Database(_) => "database_error",
            Self::Vault(_) => "vault_error",
            Self::Serialization(_) => "serialization_error",
            Self::HttpClient(_) => "http_client_error",
            Self::Internal(_) => "internal_error",
        }
    }

    /// Get the HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::MissingOrgContext => StatusCode::BAD_REQUEST,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::InstanceNotFound(_)
            | Self::ConnectionNotFound(_)
            | Self::ReceiverNotFound
            | Self::ActionNotFound(_)
            | Self::ConnectorTypeNotFound(_)
            | Self::DriftEventNotFound(_) => StatusCode::NOT_FOUND,
            Self::InstanceAlreadyExists(_) | Self::ConnectionAlreadyExists(_) => {
                StatusCode::CONFLICT
            }
            Self::InvalidInstanceName(_)
            | Self::InvalidInput(_)
            | Self::SchemaValidationFailed(_) => StatusCode::BAD_REQUEST,
            Self::CredentialsNotConfigured(_)
            | Self::CredentialsExpired(_)
            | Self::OAuthCallbackFailed(_) => StatusCode::BAD_REQUEST,
            Self::ConnectorCheckFailed { .. } | Self::ActionFailed { .. } => {
                StatusCode::BAD_GATEWAY
            }
            Self::DryRunNotSupported(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ReceiverDisabled => StatusCode::GONE,
            Self::WebhookSignatureInvalid => StatusCode::UNAUTHORIZED,
            Self::WebhookReplayDetected => StatusCode::CONFLICT,
            Self::SandboxFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidPromoteToken => StatusCode::BAD_REQUEST,
            Self::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            Self::Database(_) | Self::Vault(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Serialization(_) => StatusCode::BAD_REQUEST,
            Self::HttpClient(_) => StatusCode::BAD_GATEWAY,
        }
    }

    /// Get the suggested fix for this error, if any.
    pub fn suggested_fix(&self) -> Option<String> {
        match self {
            Self::ConnectorCheckFailed { suggested_fix, .. } => suggested_fix.clone(),
            Self::ActionFailed { suggested_fix, .. } => suggested_fix.clone(),
            Self::CredentialsNotConfigured(name) => {
                Some(format!("Run 'reactor connect instances credentials {}' to configure credentials", name))
            }
            Self::CredentialsExpired(name) => {
                Some(format!("Run 'reactor connect instances credentials {}' to refresh credentials", name))
            }
            Self::DryRunNotSupported(_) => {
                Some("Use a vendor test instance instead of dry-run for this action".to_string())
            }
            _ => None,
        }
    }

    /// Convert to structured error response.
    pub fn to_response(&self) -> ErrorResponse {
        ErrorResponse {
            code: self.code().to_string(),
            message: self.to_string(),
            suggested_fix: self.suggested_fix(),
            docs_url: Some(format!(
                "https://docs.reactor.cloud/connect/troubleshooting#{}",
                self.code()
            )),
        }
    }
}

impl IntoResponse for ConnectError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = Json(self.to_response());
        (status, body).into_response()
    }
}
