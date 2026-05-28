//! Organization endpoints.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::service::AuthService;
use crate::store::{IdentityStore, Org};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reactor_core::auth::AuthError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Create organization request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrgRequest {
    /// Organization name.
    pub name: String,
    /// Organization slug (URL-friendly identifier).
    pub slug: String,
    /// Optional metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Update organization request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateOrgRequest {
    /// New name (optional).
    #[serde(default)]
    pub name: Option<String>,
    /// New slug (optional).
    #[serde(default)]
    pub slug: Option<String>,
    /// New metadata (optional, replaces existing).
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Organization response.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrgResponse {
    /// Organization ID.
    pub id: String,
    /// Organization name.
    pub name: String,
    /// Organization slug.
    pub slug: String,
    /// Metadata.
    pub metadata: serde_json::Value,
    /// When the organization was created.
    pub created_at: String,
}

impl From<Org> for OrgResponse {
    fn from(org: Org) -> Self {
        Self {
            id: org.id.to_string(),
            name: org.name,
            slug: org.slug,
            metadata: org.metadata,
            created_at: org.created_at.to_rfc3339(),
        }
    }
}

/// Role response.
#[derive(Debug, Serialize, ToSchema)]
pub struct RoleResponse {
    /// Role ID.
    pub id: String,
    /// Organization ID.
    pub org_id: String,
    /// Role name.
    pub name: String,
    /// Role description.
    pub description: Option<String>,
    /// Whether this is a system role.
    pub is_system: bool,
    /// Role permissions.
    pub permissions: Vec<String>,
}

/// POST /auth/v1/orgs
///
/// Creates a new organization and adds the current user as owner.
#[utoipa::path(
    post,
    path = "/auth/v1/orgs",
    tag = "auth.orgs",
    security(("bearer" = [])),
    request_body = CreateOrgRequest,
    responses(
        (status = 201, description = "Organization created", body = OrgResponse),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Slug already exists"),
    )
)]
pub async fn create_org<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Json(req): Json<CreateOrgRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service
        .create_org(&user_id, &req.name, &req.slug, req.metadata)
        .await?;

    let response: OrgResponse = org.into();
    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /auth/v1/orgs
///
/// Lists all organizations the current user is a member of.
#[utoipa::path(
    get,
    path = "/auth/v1/orgs",
    tag = "auth.orgs",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "List of organizations", body = Vec<OrgResponse>),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn list_orgs<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let orgs = service.list_user_orgs(&user_id).await?;
    let response: Vec<OrgResponse> = orgs.into_iter().map(Into::into).collect();

    Ok((StatusCode::OK, Json(response)))
}

/// GET /auth/v1/orgs/{ref}
///
/// Gets an organization by ID or slug.
#[utoipa::path(
    get,
    path = "/auth/v1/orgs/{org_ref}",
    tag = "auth.orgs",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    responses(
        (status = 200, description = "Organization details", body = OrgResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not a member"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn get_org<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Check membership
    service.check_org_membership(&user_id, &org.id).await?;

    let response: OrgResponse = org.into();
    Ok((StatusCode::OK, Json(response)))
}

/// PATCH /auth/v1/orgs/{ref}
///
/// Updates an organization. Requires owner role.
#[utoipa::path(
    patch,
    path = "/auth/v1/orgs/{org_ref}",
    tag = "auth.orgs",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    request_body = UpdateOrgRequest,
    responses(
        (status = 200, description = "Organization updated", body = OrgResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an owner"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn update_org<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
    Json(req): Json<UpdateOrgRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Check if user is owner
    service.require_org_owner(&user_id, &org.id).await?;

    let updated = service
        .update_org(
            &org.id,
            req.name.as_deref(),
            req.slug.as_deref(),
            req.metadata,
        )
        .await?;

    let response: OrgResponse = updated.into();
    Ok((StatusCode::OK, Json(response)))
}

/// DELETE /auth/v1/orgs/{ref}
///
/// Deletes an organization. Requires owner role.
#[utoipa::path(
    delete,
    path = "/auth/v1/orgs/{org_ref}",
    tag = "auth.orgs",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    responses(
        (status = 204, description = "Organization deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not an owner"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn delete_org<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Check if user is owner
    service.require_org_owner(&user_id, &org.id).await?;

    service.delete_org(&org.id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /auth/v1/orgs/{ref}/roles
///
/// Lists all roles in an organization.
#[utoipa::path(
    get,
    path = "/auth/v1/orgs/{org_ref}/roles",
    tag = "auth.orgs",
    security(("bearer" = [])),
    params(
        ("org_ref" = String, Path, description = "Organization ID or slug")
    ),
    responses(
        (status = 200, description = "List of roles", body = Vec<RoleResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not a member"),
        (status = 404, description = "Organization not found"),
    )
)]
pub async fn list_roles<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(org_ref): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    let org = service.get_org_by_ref(&org_ref).await?;

    // Check membership
    service.check_org_membership(&user_id, &org.id).await?;

    let roles = service.list_org_roles(&org.id).await?;
    let response: Vec<RoleResponse> = roles
        .into_iter()
        .map(|(role, permissions)| RoleResponse {
            id: role.id.to_string(),
            org_id: role.org_id.to_string(),
            name: role.name,
            description: role.description,
            is_system: role.is_system,
            permissions,
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
}
