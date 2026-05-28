//! Permission checking endpoints.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::middleware::OrgContext;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use reactor_core::auth::{permissions, AuthCtx, AuthError, OrgRef};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Permission check request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CheckPermissionsRequest {
    /// Permissions to check (as patterns).
    pub permissions: Vec<String>,
}

/// Permission check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionsResponse {
    /// All permissions granted in the current org context.
    pub permissions: Vec<String>,
    /// Results of permission checks (same order as request).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<PermissionCheck>>,
}

/// Single permission check result.
#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionCheck {
    /// The permission that was checked.
    pub permission: String,
    /// Whether it was granted.
    pub granted: bool,
}

/// Helper to resolve OrgRef to OrgId.
async fn resolve_org_ref<S: IdentityStore>(
    service: &AuthService<S>,
    org_ref: Option<&OrgRef>,
) -> Result<Option<reactor_core::id::OrgId>, AuthError> {
    match org_ref {
        Some(OrgRef::Id(id)) => Ok(Some(*id)),
        Some(OrgRef::Slug(slug)) => {
            let org_id = service.resolve_org_ref(slug).await?;
            Ok(Some(org_id))
        }
        None => Ok(None),
    }
}

/// GET /auth/v1/permissions
///
/// Returns all permissions the current user has in the active org.
#[utoipa::path(
    get,
    path = "/auth/v1/permissions",
    tag = "auth.permissions",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "User permissions", body = PermissionsResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn get_permissions<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Extension(org_ctx): Extension<OrgContext>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    // Resolve org context (handles both UUID and slug)
    let resolved_org = resolve_org_ref(&service, org_ctx.org_ref.as_ref()).await?;
    let org_id = resolved_org
        .or(auth.claims.default_org)
        .ok_or(AuthError::OrgRequired)?;

    // Get user's permissions in this org
    let perms = service.get_user_permissions(&user_id, &org_id).await?;

    let response = PermissionsResponse {
        permissions: perms,
        checks: None,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// POST /auth/v1/permissions
///
/// Checks if the current user has specific permissions.
#[utoipa::path(
    post,
    path = "/auth/v1/permissions",
    tag = "auth.permissions",
    security(("bearer" = [])),
    request_body = CheckPermissionsRequest,
    responses(
        (status = 200, description = "Permission check results", body = PermissionsResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn check_permissions<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Extension(org_ctx): Extension<OrgContext>,
    Json(req): Json<CheckPermissionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    // Resolve org context (handles both UUID and slug)
    let resolved_org = resolve_org_ref(&service, org_ctx.org_ref.as_ref()).await?;
    let org_id = resolved_org
        .or(auth.claims.default_org)
        .ok_or(AuthError::OrgRequired)?;

    // Get user's permissions in this org
    let granted_perms = service.get_user_permissions(&user_id, &org_id).await?;

    // Check each requested permission
    let checks: Vec<PermissionCheck> = req
        .permissions
        .iter()
        .map(|p| PermissionCheck {
            permission: p.clone(),
            granted: permissions::matches_any(&granted_perms, p),
        })
        .collect();

    let response = PermissionsResponse {
        permissions: granted_perms,
        checks: Some(checks),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Context resolution response (internal endpoint).
#[derive(Debug, Serialize)]
pub struct ResolveCtxResponse {
    /// Resolved authentication context.
    pub ctx: AuthCtx,
}

/// POST /_internal/resolve_ctx
///
/// Internal endpoint for other Reactor capabilities to resolve auth context.
/// Takes a bearer token and X-Reactor-Org header (UUID or slug) and returns
/// the full AuthCtx with permissions.
pub async fn resolve_ctx<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Extension(org_ctx): Extension<OrgContext>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;

    // Resolve org context (handles both UUID and slug via OrgRef)
    let resolved_org = resolve_org_ref(&service, org_ctx.org_ref.as_ref()).await?;
    let org_id = resolved_org.or(auth.claims.default_org);

    // Build AuthCtx
    let permissions = if let Some(ref oid) = org_id {
        service.get_user_permissions(&user_id, oid).await?
    } else {
        vec![]
    };

    let ctx = AuthCtx {
        claims: auth.claims,
        active_org: org_id,
        permissions,
    };

    let response = ResolveCtxResponse { ctx };

    Ok((StatusCode::OK, Json(response)))
}
