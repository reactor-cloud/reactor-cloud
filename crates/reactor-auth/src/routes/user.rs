//! User self-management endpoints.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::routes::signup::UserResponse;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use reactor_core::auth::AuthError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Response for /me endpoint — current user with org memberships.
#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentUserResponse {
    /// User ID.
    pub user_id: String,
    /// User email.
    pub email: String,
    /// Organization memberships.
    #[serde(default)]
    pub orgs: Vec<OrgMembershipResponse>,
}

/// Organization membership in /me response.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrgMembershipResponse {
    /// Organization ID.
    pub org_id: String,
    /// Organization slug.
    pub org_slug: String,
    /// Role name.
    pub role: String,
}

/// GET /auth/v1/me
///
/// Returns the current authenticated user with their organization memberships.
/// This is the primary endpoint for CLI `whoami` and client identity checks.
#[utoipa::path(
    get,
    path = "/auth/v1/me",
    tag = "auth",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Current user with org memberships", body = CurrentUserResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn get_me<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let user = service.get_user(&user_id).await?;
    let user_orgs = service.list_user_orgs(&user_id).await?;

    // Get membership details for each org
    let mut orgs = Vec::with_capacity(user_orgs.len());
    for org in user_orgs {
        if let Ok((_, _, role)) = service.get_member(&org.id, &user_id).await {
            orgs.push(OrgMembershipResponse {
                org_id: org.id.to_string(),
                org_slug: org.slug,
                role: role.name,
            });
        }
    }

    let response = CurrentUserResponse {
        user_id: user.id.to_string(),
        email: user.email,
        orgs,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Update user request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    /// New email address (optional).
    #[serde(default)]
    pub email: Option<String>,
    /// New password (optional).
    #[serde(default)]
    pub password: Option<String>,
    /// New metadata (optional, replaces existing).
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// GET /auth/v1/user
///
/// Returns the current authenticated user.
#[utoipa::path(
    get,
    path = "/auth/v1/user",
    tag = "auth",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Current user", body = UserResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn get_user<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let user = service.get_user(&user_id).await?;

    let response = UserResponse {
        id: user.id.to_string(),
        email: user.email,
        email_verified: user.email_verified,
        metadata: user.metadata,
        created_at: user.created_at.to_rfc3339(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// PATCH /auth/v1/user
///
/// Updates the current user's email, password, or metadata.
#[utoipa::path(
    patch,
    path = "/auth/v1/user",
    tag = "auth",
    security(("bearer" = [])),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn update_user<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    // Hash password if provided
    let password_hash = if let Some(ref password) = req.password {
        let hasher = crate::password::PasswordHasherService::new();
        Some(hasher.hash(password).map_err(|_| AuthError::Internal)?)
    } else {
        None
    };

    let user = service
        .update_user(
            &user_id,
            req.email.as_deref(),
            password_hash.as_deref(),
            req.metadata,
        )
        .await?;

    let response = UserResponse {
        id: user.id.to_string(),
        email: user.email,
        email_verified: user.email_verified,
        metadata: user.metadata,
        created_at: user.created_at.to_rfc3339(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// DELETE /auth/v1/user
///
/// Soft-deletes the current user and revokes all sessions.
#[utoipa::path(
    delete,
    path = "/auth/v1/user",
    tag = "auth",
    security(("bearer" = [])),
    responses(
        (status = 204, description = "User deleted"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn delete_user<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    service.delete_user(&user_id).await?;

    Ok(StatusCode::NO_CONTENT)
}
