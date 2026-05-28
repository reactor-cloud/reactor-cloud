//! Sites error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Sites error type.
#[derive(Debug, Error)]
pub enum SitesError {
    /// Site not found.
    #[error("site not found: {0}")]
    SiteNotFound(String),

    /// Deployment not found.
    #[error("deployment not found: {0}")]
    DeploymentNotFound(String),

    /// Deployment not ready.
    #[error("deployment not ready: {0}")]
    DeploymentNotReady(String),

    /// Invalid site name.
    #[error("invalid site name: {0}")]
    InvalidSiteName(String),

    /// Invalid framework.
    #[error("invalid framework: {0}")]
    InvalidFramework(String),

    /// Site already exists.
    #[error("site already exists: {0}")]
    SiteAlreadyExists(String),

    /// Domain already taken.
    #[error("domain already taken: {0}")]
    DomainTaken(String),

    /// Domain not verified.
    #[error("domain not verified: {0}")]
    DomainUnverified(String),

    /// Domain verification failed.
    #[error("domain verification failed: {0}")]
    DomainVerificationFailed(String),

    /// Bundle invalid.
    #[error("bundle invalid: {0}")]
    BundleInvalid(String),

    /// Manifest invalid.
    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),

    /// Bundle too large.
    #[error("bundle too large: max {max} bytes, got {actual} bytes")]
    BundleTooLarge { max: u64, actual: u64 },

    /// Static upload failed.
    #[error("static upload failed: {0}")]
    StaticUploadFailed(String),

    /// Function deploy failed.
    #[error("function deploy failed: {0}")]
    FunctionDeployFailed(String),

    /// Route unmatched.
    #[error("no route matched path: {0}")]
    RouteUnmatched(String),

    /// Function dispatch failed.
    #[error("function dispatch failed: {0}")]
    FunctionDispatchFailed(String),

    /// Policy denied.
    #[error("policy denied: {0}")]
    PolicyDenied(String),

    /// Revalidate failed.
    #[error("revalidate failed: {0}")]
    RevalidateFailed(String),

    /// ACME challenge failed.
    #[error("ACME challenge failed: {0}")]
    AcmeChallengeFailed(String),

    /// Organization required.
    #[error("organization context required")]
    OrgRequired,

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Authentication required.
    #[error("authentication required")]
    AuthRequired,

    /// Invalid auth token.
    #[error("invalid authentication token")]
    InvalidToken,

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// Storage client error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Functions client error.
    #[error("functions error: {0}")]
    Functions(String),

    /// HTTP client error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error details.
    pub error: ErrorDetail,
}

/// Error detail.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    /// Error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// HTTP status code.
    pub status: u16,
    /// Request ID for tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Additional details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl SitesError {
    /// Get the error code.
    pub fn code(&self) -> &'static str {
        match self {
            SitesError::SiteNotFound(_) => "site_not_found",
            SitesError::DeploymentNotFound(_) => "deployment_not_found",
            SitesError::DeploymentNotReady(_) => "deployment_not_ready",
            SitesError::InvalidSiteName(_) => "invalid_site_name",
            SitesError::InvalidFramework(_) => "invalid_framework",
            SitesError::SiteAlreadyExists(_) => "site_already_exists",
            SitesError::DomainTaken(_) => "domain_taken",
            SitesError::DomainUnverified(_) => "domain_unverified",
            SitesError::DomainVerificationFailed(_) => "domain_verification_failed",
            SitesError::BundleInvalid(_) => "bundle_invalid",
            SitesError::ManifestInvalid(_) => "manifest_invalid",
            SitesError::BundleTooLarge { .. } => "bundle_too_large",
            SitesError::StaticUploadFailed(_) => "static_upload_failed",
            SitesError::FunctionDeployFailed(_) => "function_deploy_failed",
            SitesError::RouteUnmatched(_) => "route_unmatched",
            SitesError::FunctionDispatchFailed(_) => "function_dispatch_failed",
            SitesError::PolicyDenied(_) => "policy_denied",
            SitesError::RevalidateFailed(_) => "revalidate_failed",
            SitesError::AcmeChallengeFailed(_) => "acme_challenge_failed",
            SitesError::OrgRequired => "org_required",
            SitesError::PermissionDenied(_) => "permission_denied",
            SitesError::AuthRequired => "auth_required",
            SitesError::InvalidToken => "invalid_token",
            SitesError::Database(_) => "database_error",
            SitesError::Internal(_) => "internal_error",
            SitesError::Storage(_) => "storage_error",
            SitesError::Functions(_) => "functions_error",
            SitesError::Http(_) => "http_error",
        }
    }

    /// Get the HTTP status code.
    pub fn status_code(&self) -> StatusCode {
        match self {
            SitesError::SiteNotFound(_) => StatusCode::NOT_FOUND,
            SitesError::DeploymentNotFound(_) => StatusCode::NOT_FOUND,
            SitesError::DeploymentNotReady(_) => StatusCode::SERVICE_UNAVAILABLE,
            SitesError::InvalidSiteName(_) => StatusCode::BAD_REQUEST,
            SitesError::InvalidFramework(_) => StatusCode::BAD_REQUEST,
            SitesError::SiteAlreadyExists(_) => StatusCode::CONFLICT,
            SitesError::DomainTaken(_) => StatusCode::CONFLICT,
            SitesError::DomainUnverified(_) => StatusCode::PRECONDITION_FAILED,
            SitesError::DomainVerificationFailed(_) => StatusCode::PRECONDITION_FAILED,
            SitesError::BundleInvalid(_) => StatusCode::BAD_REQUEST,
            SitesError::ManifestInvalid(_) => StatusCode::BAD_REQUEST,
            SitesError::BundleTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            SitesError::StaticUploadFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::FunctionDeployFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::RouteUnmatched(_) => StatusCode::NOT_FOUND,
            SitesError::FunctionDispatchFailed(_) => StatusCode::BAD_GATEWAY,
            SitesError::PolicyDenied(_) => StatusCode::FORBIDDEN,
            SitesError::RevalidateFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::AcmeChallengeFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::OrgRequired => StatusCode::BAD_REQUEST,
            SitesError::PermissionDenied(_) => StatusCode::FORBIDDEN,
            SitesError::AuthRequired => StatusCode::UNAUTHORIZED,
            SitesError::InvalidToken => StatusCode::UNAUTHORIZED,
            SitesError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::Functions(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SitesError::Http(_) => StatusCode::BAD_GATEWAY,
        }
    }

    /// Convert to error response with optional request ID.
    pub fn to_response(&self, request_id: Option<String>) -> ErrorResponse {
        ErrorResponse {
            error: ErrorDetail {
                code: self.code().to_string(),
                message: self.to_string(),
                status: self.status_code().as_u16(),
                request_id,
                details: None,
            },
        }
    }
}

impl IntoResponse for SitesError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = self.to_response(None);
        (status, Json(body)).into_response()
    }
}
