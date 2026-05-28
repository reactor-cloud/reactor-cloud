//! Signup endpoint.

use crate::error::AppError;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Signup request body.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SignupRequest {
    /// User's email address.
    pub email: String,
    /// Password (min 8 characters recommended).
    pub password: String,
    /// Optional metadata to store with the user.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Signup response.
#[derive(Debug, Serialize, ToSchema)]
pub struct SignupResponse {
    /// The created user.
    pub user: UserResponse,
    /// Session information.
    pub session: SessionResponse,
}

/// User in response.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    /// User ID.
    pub id: String,
    /// Email address.
    pub email: String,
    /// Whether email is verified.
    pub email_verified: bool,
    /// User metadata.
    pub metadata: serde_json::Value,
    /// When the user was created.
    pub created_at: String,
}

/// Session in response.
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    /// JWT access token.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// When the access token expires.
    pub expires_at: String,
}

/// POST /auth/v1/signup
#[utoipa::path(
    post,
    path = "/auth/v1/signup",
    tag = "auth",
    request_body = SignupRequest,
    responses(
        (status = 201, description = "User created successfully", body = SignupResponse),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Email already exists"),
    )
)]
pub async fn signup<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    headers: header::HeaderMap,
    Json(req): Json<SignupRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Extract client info from headers
    let ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim());

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());

    let auth_response = service
        .signup(&req.email, &req.password, req.metadata, ip, user_agent)
        .await?;

    let response = SignupResponse {
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

    Ok((StatusCode::CREATED, Json(response)))
}
