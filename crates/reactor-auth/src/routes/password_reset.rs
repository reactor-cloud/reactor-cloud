//! Password reset endpoints.

use crate::error::AppError;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Request body for password reset request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PasswordResetRequestBody {
    /// User's email address.
    pub email: String,
}

/// Request body for password reset confirmation.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PasswordResetConfirmBody {
    /// The password reset token from the email link.
    pub token: String,
    /// The new password to set.
    pub new_password: String,
}

/// Response for password reset request.
#[derive(Debug, Serialize, ToSchema)]
pub struct PasswordResetRequestResponse {
    /// Message indicating the result.
    pub message: String,
}

/// Response for password reset confirmation.
#[derive(Debug, Serialize, ToSchema)]
pub struct PasswordResetConfirmResponse {
    /// Whether the password was reset successfully.
    pub success: bool,
    /// Message indicating the result.
    pub message: String,
}

/// POST /auth/v1/password-reset/request
///
/// Request a password reset email. Always returns 202 Accepted regardless of
/// whether the email exists to prevent user enumeration.
#[utoipa::path(
    post,
    path = "/auth/v1/password-reset/request",
    tag = "auth",
    request_body = PasswordResetRequestBody,
    responses(
        (status = 202, description = "Password reset email sent (if user exists)", body = PasswordResetRequestResponse),
    )
)]
pub async fn request_password_reset<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Json(body): Json<PasswordResetRequestBody>,
) -> impl IntoResponse {
    // Always return 202 to prevent user enumeration
    // The actual work happens in the background
    tokio::spawn({
        let service = service.clone();
        let email = body.email.clone();
        async move {
            if let Err(e) = service.request_password_reset(&email).await {
                tracing::debug!(email = %email, error = %e, "password reset request failed (expected for non-existent users)");
            }
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(PasswordResetRequestResponse {
            message: "If an account exists with that email, a password reset link has been sent."
                .to_string(),
        }),
    )
}

/// POST /auth/v1/password-reset/confirm
///
/// Confirm password reset with token and new password.
#[utoipa::path(
    post,
    path = "/auth/v1/password-reset/confirm",
    tag = "auth",
    request_body = PasswordResetConfirmBody,
    responses(
        (status = 200, description = "Password reset successful", body = PasswordResetConfirmResponse),
        (status = 400, description = "Invalid or expired token"),
    )
)]
pub async fn confirm_password_reset<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Json(body): Json<PasswordResetConfirmBody>,
) -> Result<impl IntoResponse, AppError> {
    service
        .confirm_password_reset(&body.token, &body.new_password)
        .await?;

    Ok((
        StatusCode::OK,
        Json(PasswordResetConfirmResponse {
            success: true,
            message: "Password has been reset successfully. You can now log in with your new password.".to_string(),
        }),
    ))
}
