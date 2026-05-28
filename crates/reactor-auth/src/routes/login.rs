//! Login endpoint - convenience wrapper for password grant.

use crate::error::AppError;
use crate::routes::signup::{SessionResponse, UserResponse};
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Login request body.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// User's email address.
    pub email: String,
    /// User's password.
    pub password: String,
}

/// Login response.
#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    /// The authenticated user.
    pub user: UserResponse,
    /// Session tokens.
    #[serde(flatten)]
    pub session: SessionResponse,
}

/// POST /auth/v1/login
///
/// Authenticate with email and password.
#[utoipa::path(
    post,
    path = "/auth/v1/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
    )
)]
pub async fn login<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_response = service
        .password_grant(&req.email, &req.password, None, None)
        .await?;

    let response = LoginResponse {
        user: UserResponse {
            id: auth_response.user.id.to_string(),
            email: auth_response.user.email,
            email_verified: auth_response.user.email_verified,
            metadata: auth_response.user.metadata,
            created_at: auth_response.user.created_at.to_rfc3339(),
        },
        session: SessionResponse {
            access_token: auth_response.access_token,
            refresh_token: auth_response.refresh_token,
            expires_at: auth_response.expires_at.to_rfc3339(),
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
