//! Invitation endpoints.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reactor_core::auth::AuthError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Create invitation request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateInvitationRequest {
    /// Email address to invite.
    pub email: String,
    /// Role ID to assign.
    pub role_id: String,
}

/// Invitation response.
#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationResponse {
    /// Invitation ID.
    pub id: String,
    /// Email address.
    pub email: String,
    /// Organization ID.
    pub org_id: String,
    /// Role ID.
    pub role_id: String,
    /// Invitation link (only included on creation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_link: Option<String>,
    /// When the invitation was created.
    pub created_at: String,
    /// When the invitation expires.
    pub expires_at: String,
}

/// Accept invitation query params.
#[derive(Debug, Deserialize, IntoParams)]
pub struct AcceptInvitationQuery {
    /// The invitation token.
    pub token: String,
}

/// POST /auth/v1/orgs/{ref}/invitations
///
/// Creates an invitation. Returns a signed link that can be shared.
/// If SMTP is configured, also sends an email.
#[utoipa::path(
    post,
    path = "/auth/v1/orgs/{org_ref}/invitations",
    tag = "auth.invitations",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    request_body = CreateInvitationRequest,
    responses(
        (status = 201, description = "Invitation created", body = InvitationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an admin or owner"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn create_invitation<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Must be admin or owner to create invitations
    service
        .require_org_admin_or_owner(&user_id, &org.id)
        .await?;

    let role_id = req
        .role_id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid role_id".to_string(),
        })?;

    let (invitation, invite_link) = service
        .create_invitation(&org.id, &req.email, &role_id)
        .await?;

    let response = InvitationResponse {
        id: invitation.id.to_string(),
        email: invitation.email,
        org_id: invitation.org_id.to_string(),
        role_id: invitation.role_id.to_string(),
        invite_link: Some(invite_link),
        created_at: invitation.created_at.to_rfc3339(),
        expires_at: invitation.expires_at.to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /auth/v1/orgs/{ref}/invitations
///
/// Lists pending invitations for an organization.
#[utoipa::path(
    get,
    path = "/auth/v1/orgs/{org_ref}/invitations",
    tag = "auth.invitations",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    responses(
        (status = 200, description = "List of invitations", body = Vec<InvitationResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an admin or owner"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn list_invitations<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Must be admin or owner to list invitations
    service
        .require_org_admin_or_owner(&user_id, &org.id)
        .await?;

    let invitations = service.list_org_invitations(&org.id).await?;
    let response: Vec<InvitationResponse> = invitations
        .into_iter()
        .map(|inv| InvitationResponse {
            id: inv.id.to_string(),
            email: inv.email,
            org_id: inv.org_id.to_string(),
            role_id: inv.role_id.to_string(),
            invite_link: None,
            created_at: inv.created_at.to_rfc3339(),
            expires_at: inv.expires_at.to_rfc3339(),
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
}

/// DELETE /auth/v1/orgs/{ref}/invitations/{invitation_id}
///
/// Revokes an invitation.
#[utoipa::path(
    delete,
    path = "/auth/v1/orgs/{org_ref}/invitations/{invitation_id}",
    tag = "auth.invitations",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug"),
        ("invitation_id" = String, Path, description = "Invitation ID")
    ),
    responses(
        (status = 204, description = "Invitation revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an admin or owner"),
        (status = 404, description = "Invitation not found"),
    )
)]
pub async fn delete_invitation<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path((org_ref, invitation_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Must be admin or owner to revoke invitations
    service
        .require_org_admin_or_owner(&user_id, &org.id)
        .await?;

    let inv_id = invitation_id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid invitation_id".to_string(),
        })?;

    service.delete_invitation(&inv_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /auth/v1/invitations/accept
///
/// Accepts an invitation using a signed token. Creates membership if user exists,
/// or returns the invitation details for signup flow.
#[utoipa::path(
    post,
    path = "/auth/v1/invitations/accept",
    tag = "auth.invitations",
    params(AcceptInvitationQuery),
    responses(
        (status = 200, description = "Invitation accepted"),
        (status = 400, description = "Invalid or expired token"),
    )
)]
pub async fn accept_invitation<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Query(query): Query<AcceptInvitationQuery>,
) -> Result<impl IntoResponse, AppError> {
    let result = service.accept_invitation(&query.token).await?;

    Ok((StatusCode::OK, Json(result)))
}
