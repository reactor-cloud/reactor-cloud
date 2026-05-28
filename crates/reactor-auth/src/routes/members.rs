//! Organization membership endpoints.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::routes::signup::UserResponse;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reactor_core::auth::AuthError;
use reactor_core::id::UserId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Member response.
#[derive(Debug, Serialize, ToSchema)]
pub struct MemberResponse {
    /// User information.
    pub user: UserResponse,
    /// Role ID.
    pub role_id: String,
    /// Role name.
    pub role_name: String,
    /// When the user joined the organization.
    pub joined_at: String,
}

/// Update member request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMemberRequest {
    /// New role ID.
    pub role_id: String,
}

/// GET /auth/v1/orgs/{ref}/members
///
/// Lists all members of an organization.
#[utoipa::path(
    get,
    path = "/auth/v1/orgs/{org_ref}/members",
    tag = "auth.members",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    responses(
        (status = 200, description = "List of members", body = Vec<MemberResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not a member"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn list_members<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;
    service.check_org_membership(&user_id, &org.id).await?;

    let members = service.list_org_members(&org.id).await?;
    let response: Vec<MemberResponse> = members
        .into_iter()
        .map(|(user, membership, role)| MemberResponse {
            user: UserResponse {
                id: user.id.to_string(),
                email: user.email,
                email_verified: user.email_verified,
                metadata: user.metadata,
                created_at: user.created_at.to_rfc3339(),
            },
            role_id: membership.role_id.to_string(),
            role_name: role.name,
            joined_at: membership.joined_at.to_rfc3339(),
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
}

/// GET /auth/v1/orgs/{ref}/members/{user_id}
///
/// Gets a specific member's details.
#[utoipa::path(
    get,
    path = "/auth/v1/orgs/{org_ref}/members/{user_id}",
    tag = "auth.members",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug"),
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "Member details", body = MemberResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not a member"),
        (status = 404, description = "Member not found"),
    )
)]
pub async fn get_member<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path((org_ref, target_user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;
    service.check_org_membership(&user_id, &org.id).await?;

    let target_id: UserId = target_user_id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid user_id".to_string(),
        })?;

    let (user, membership, role) = service.get_member(&org.id, &target_id).await?;

    let response = MemberResponse {
        user: UserResponse {
            id: user.id.to_string(),
            email: user.email,
            email_verified: user.email_verified,
            metadata: user.metadata,
            created_at: user.created_at.to_rfc3339(),
        },
        role_id: membership.role_id.to_string(),
        role_name: role.name,
        joined_at: membership.joined_at.to_rfc3339(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// PATCH /auth/v1/orgs/{ref}/members/{user_id}
///
/// Updates a member's role. Requires admin or owner role.
#[utoipa::path(
    patch,
    path = "/auth/v1/orgs/{org_ref}/members/{user_id}",
    tag = "auth.members",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug"),
        ("user_id" = String, Path, description = "User ID")
    ),
    request_body = UpdateMemberRequest,
    responses(
        (status = 200, description = "Member updated", body = MemberResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an admin or owner"),
        (status = 404, description = "Member not found"),
    )
)]
pub async fn update_member<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path((org_ref, target_user_id)): Path<(String, String)>,
    Json(req): Json<UpdateMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Must be admin or owner to update members
    service
        .require_org_admin_or_owner(&user_id, &org.id)
        .await?;

    let target_id: UserId = target_user_id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid user_id".to_string(),
        })?;

    let new_role_id = req
        .role_id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid role_id".to_string(),
        })?;

    let (user, membership, role) = service
        .update_member_role(&org.id, &target_id, &new_role_id)
        .await?;

    let response = MemberResponse {
        user: UserResponse {
            id: user.id.to_string(),
            email: user.email,
            email_verified: user.email_verified,
            metadata: user.metadata,
            created_at: user.created_at.to_rfc3339(),
        },
        role_id: membership.role_id.to_string(),
        role_name: role.name,
        joined_at: membership.joined_at.to_rfc3339(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// DELETE /auth/v1/orgs/{ref}/members/{user_id}
///
/// Removes a member from the organization. Cannot remove the last owner.
#[utoipa::path(
    delete,
    path = "/auth/v1/orgs/{org_ref}/members/{user_id}",
    tag = "auth.members",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug"),
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 204, description = "Member removed"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an admin or owner"),
        (status = 404, description = "Member not found"),
    )
)]
pub async fn delete_member<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path((org_ref, target_user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Must be admin or owner to remove members
    service
        .require_org_admin_or_owner(&user_id, &org.id)
        .await?;

    let target_id: UserId = target_user_id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid user_id".to_string(),
        })?;

    service.remove_member(&org.id, &target_id).await?;

    Ok(StatusCode::NO_CONTENT)
}
