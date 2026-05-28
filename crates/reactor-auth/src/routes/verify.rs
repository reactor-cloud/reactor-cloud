//! Email verification endpoints.

use crate::error::AppError;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for verification.
#[derive(Debug, Deserialize, IntoParams)]
pub struct VerifyQuery {
    /// The verification token.
    pub token: String,
}

/// Request body for resending verification email.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResendRequest {
    /// User's email address.
    pub email: String,
}

/// Response for verification.
#[derive(Debug, Serialize, ToSchema)]
pub struct VerifyResponse {
    /// Whether verification succeeded.
    pub verified: bool,
    /// User email.
    pub email: String,
    /// Message.
    pub message: String,
}

/// Response for resend.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResendResponse {
    /// Whether the email was sent.
    pub sent: bool,
    /// Message.
    pub message: String,
}

/// GET /auth/v1/verify?token=xxx
///
/// Verifies a user's email address using the token from the verification email.
#[utoipa::path(
    get,
    path = "/auth/v1/verify",
    tag = "auth",
    params(VerifyQuery),
    responses(
        (status = 200, description = "Email verified", body = VerifyResponse),
        (status = 400, description = "Invalid or expired token"),
    )
)]
pub async fn verify_email<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Query(query): Query<VerifyQuery>,
) -> Result<impl IntoResponse, AppError> {
    let user = service.verify_email(&query.token).await?;

    let response = VerifyResponse {
        verified: true,
        email: user.email,
        message: "Email verified successfully".to_string(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// POST /auth/v1/verify/resend
///
/// Resends the verification email to a user.
#[utoipa::path(
    post,
    path = "/auth/v1/verify/resend",
    tag = "auth",
    request_body = ResendRequest,
    responses(
        (status = 200, description = "Resend result", body = ResendResponse),
        (status = 404, description = "User not found"),
    )
)]
pub async fn resend_verification<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Json(req): Json<ResendRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Find user by email
    let user = service
        .find_user_by_email(&req.email)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;

    // Don't resend if already verified
    if user.email_verified {
        return Ok((
            StatusCode::OK,
            Json(ResendResponse {
                sent: false,
                message: "Email is already verified".to_string(),
            }),
        ));
    }

    // Send verification email
    service.send_verification_email(&user).await?;

    Ok((
        StatusCode::OK,
        Json(ResendResponse {
            sent: true,
            message: "Verification email sent".to_string(),
        }),
    ))
}
