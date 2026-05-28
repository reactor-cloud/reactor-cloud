//! Authentication error types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Authentication and authorization errors.
///
/// Error codes are stable strings that clients can rely on.
/// HTTP status codes are informational; always switch on `code`.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(tag = "code", content = "details")]
pub enum AuthError {
    /// Invalid or missing credentials.
    #[error("invalid credentials")]
    #[serde(rename = "invalid_credentials")]
    InvalidCredentials,

    /// Token has expired.
    #[error("token expired")]
    #[serde(rename = "token_expired")]
    TokenExpired,

    /// Token signature is invalid.
    #[error("invalid token signature")]
    #[serde(rename = "invalid_token")]
    InvalidToken,

    /// User not found.
    #[error("user not found")]
    #[serde(rename = "user_not_found")]
    UserNotFound,

    /// User account is disabled.
    #[error("user disabled")]
    #[serde(rename = "user_disabled")]
    UserDisabled,

    /// Email already registered.
    #[error("email already exists")]
    #[serde(rename = "email_exists")]
    EmailExists,

    /// Organization not found.
    #[error("org not found")]
    #[serde(rename = "org_not_found")]
    OrgNotFound,

    /// Organization slug already exists.
    #[error("org slug already exists")]
    #[serde(rename = "org_slug_exists")]
    OrgSlugExists,

    /// Membership already exists.
    #[error("membership already exists")]
    #[serde(rename = "membership_exists")]
    MembershipExists,

    /// Membership not found.
    #[error("membership not found")]
    #[serde(rename = "membership_not_found")]
    MembershipNotFound,

    /// User is not a member of the organization.
    #[error("not a member of this org")]
    #[serde(rename = "not_org_member")]
    NotOrgMember,

    /// Permission denied.
    #[error("permission denied")]
    #[serde(rename = "permission_denied")]
    PermissionDenied,

    /// Session has been revoked.
    #[error("session revoked")]
    #[serde(rename = "session_revoked")]
    SessionRevoked,

    /// Refresh token has been used or is invalid.
    #[error("invalid refresh token")]
    #[serde(rename = "invalid_refresh_token")]
    InvalidRefreshToken,

    /// Refresh token reuse detected — session revoked for security.
    #[error("refresh token reuse detected")]
    #[serde(rename = "refresh_token_reuse")]
    RefreshTokenReuse,

    /// Invitation not found or expired.
    #[error("invitation not found or expired")]
    #[serde(rename = "invitation_not_found")]
    InvitationNotFound,

    /// Invitation has already been used.
    #[error("invitation already used")]
    #[serde(rename = "invitation_used")]
    InvitationUsed,

    /// Role not found.
    #[error("role not found")]
    #[serde(rename = "role_not_found")]
    RoleNotFound,

    /// Cannot delete or modify system role.
    #[error("cannot modify system role")]
    #[serde(rename = "system_role")]
    SystemRole,

    /// Cannot remove last owner from organization.
    #[error("cannot remove last owner")]
    #[serde(rename = "last_owner")]
    LastOwner,

    /// Invalid request parameters.
    #[error("validation error: {message}")]
    #[serde(rename = "validation_error")]
    ValidationError {
        /// Description of what failed validation.
        message: String,
    },

    /// Password does not meet policy requirements.
    #[error("weak password: {0}")]
    #[serde(rename = "weak_password")]
    WeakPassword(String),

    /// Internal server error.
    #[error("internal error")]
    #[serde(rename = "internal_error")]
    Internal,

    /// Email sending is disabled (SMTP not configured).
    #[error("email disabled")]
    #[serde(rename = "email_disabled")]
    EmailDisabled,

    /// Missing or invalid authorization header.
    #[error("missing or invalid authorization header")]
    #[serde(rename = "unauthorized")]
    Unauthorized,

    /// Missing required org context (no X-Reactor-Org header and no default_org).
    #[error("org context required")]
    #[serde(rename = "org_required")]
    OrgRequired,
}

impl AuthError {
    /// Get the HTTP status code for this error.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::InvalidCredentials
            | Self::TokenExpired
            | Self::InvalidToken
            | Self::InvalidRefreshToken
            | Self::RefreshTokenReuse
            | Self::SessionRevoked
            | Self::Unauthorized => 401,

            Self::PermissionDenied | Self::NotOrgMember | Self::SystemRole | Self::LastOwner => 403,

            Self::UserNotFound
            | Self::OrgNotFound
            | Self::MembershipNotFound
            | Self::InvitationNotFound
            | Self::RoleNotFound => 404,

            Self::EmailExists
            | Self::OrgSlugExists
            | Self::MembershipExists
            | Self::InvitationUsed => 409,

            Self::ValidationError { .. } | Self::OrgRequired | Self::WeakPassword(_) => 400,

            Self::UserDisabled => 403,

            Self::Internal | Self::EmailDisabled => 500,
        }
    }

    /// Get the stable error code string for this error.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::TokenExpired => "token_expired",
            Self::InvalidToken => "invalid_token",
            Self::UserNotFound => "user_not_found",
            Self::UserDisabled => "user_disabled",
            Self::EmailExists => "email_exists",
            Self::OrgNotFound => "org_not_found",
            Self::OrgSlugExists => "org_slug_exists",
            Self::MembershipExists => "membership_exists",
            Self::MembershipNotFound => "membership_not_found",
            Self::NotOrgMember => "not_org_member",
            Self::PermissionDenied => "permission_denied",
            Self::SessionRevoked => "session_revoked",
            Self::InvalidRefreshToken => "invalid_refresh_token",
            Self::RefreshTokenReuse => "refresh_token_reuse",
            Self::InvitationNotFound => "invitation_not_found",
            Self::InvitationUsed => "invitation_used",
            Self::RoleNotFound => "role_not_found",
            Self::SystemRole => "system_role",
            Self::LastOwner => "last_owner",
            Self::ValidationError { .. } => "validation_error",
            Self::WeakPassword(_) => "weak_password",
            Self::Internal => "internal_error",
            Self::EmailDisabled => "email_disabled",
            Self::Unauthorized => "unauthorized",
            Self::OrgRequired => "org_required",
        }
    }
}
