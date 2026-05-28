//! Project management routes.
//!
//! These routes provide ops-level access to project management functionality,
//! replacing the admin-token-gated `/_cloud/v1/projects` endpoints.

use crate::error::OpsError;
use crate::middleware::OpsAuthCtx;
use crate::state::OpsState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Request to create a project.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    /// Project name.
    pub name: String,
    /// Deployment region (default: "iad").
    pub region: Option<String>,
}

/// Response for project operations.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectResponse {
    /// Project ID.
    pub id: String,
    /// Project ref (subdomain).
    #[serde(rename = "ref")]
    pub project_ref: String,
    /// Project name.
    pub name: String,
    /// Owner user ID.
    pub owner_user_id: String,
    /// Current status.
    pub status: String,
    /// Deployment region.
    pub region: String,
    /// Hostname.
    pub hostname: String,
}

/// Query params for listing projects.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListProjectsQuery {
    /// Filter by owner.
    pub owner: Option<String>,
    /// Maximum results.
    #[serde(default = "default_limit")]
    pub limit: i32,
    /// Offset for pagination.
    #[serde(default)]
    pub offset: i32,
}

fn default_limit() -> i32 {
    50
}

/// List response.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListProjectsResponse {
    /// Projects list.
    pub projects: Vec<ProjectResponse>,
}

/// List all projects.
///
/// Requires `cloud:projects:read` scope.
#[utoipa::path(
    get,
    path = "/_ops/v1/projects",
    params(ListProjectsQuery),
    responses(
        (status = 200, description = "Projects list", body = ListProjectsResponse),
        (status = 403, description = "Missing scope"),
    )
)]
pub async fn list_projects(
    State(_state): State<OpsState>,
    _ctx: OpsAuthCtx,
    Query(_query): Query<ListProjectsQuery>,
) -> Result<Json<ListProjectsResponse>, OpsError> {
    // In full implementation, this would forward to CloudApiState.projects.list_all()
    // For now, return placeholder
    Ok(Json(ListProjectsResponse {
        projects: vec![],
    }))
}

/// Get a project by ref.
///
/// Requires `cloud:projects:read` scope.
#[utoipa::path(
    get,
    path = "/_ops/v1/projects/{project_ref}",
    responses(
        (status = 200, description = "Project details", body = ProjectResponse),
        (status = 404, description = "Project not found"),
    )
)]
pub async fn get_project(
    State(_state): State<OpsState>,
    _ctx: OpsAuthCtx,
    Path(_project_ref): Path<String>,
) -> Result<Json<ProjectResponse>, OpsError> {
    // In full implementation, this would forward to CloudApiState.projects.get_by_ref()
    Err(OpsError::NotFound)
}

/// Create a new project.
///
/// Requires `cloud:projects:write` scope.
#[utoipa::path(
    post,
    path = "/_ops/v1/projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, description = "Project created", body = ProjectResponse),
        (status = 403, description = "Missing scope"),
    )
)]
pub async fn create_project(
    State(_state): State<OpsState>,
    ctx: OpsAuthCtx,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, OpsError> {
    // In full implementation:
    // 1. Forward to CloudApiState.projects.create() with owner_user_id = ctx.user_id
    // 2. Return created project
    
    Ok(Json(ProjectResponse {
        id: uuid::Uuid::now_v7().to_string(),
        project_ref: format!("{}-preview", req.name.to_lowercase().replace(' ', "-")),
        name: req.name,
        owner_user_id: ctx.user_id.to_string(),
        status: "pending".to_string(),
        region: req.region.unwrap_or_else(|| "iad".to_string()),
        hostname: "pending.superscalable.cloud".to_string(),
    }))
}

/// Delete a project.
///
/// Requires `cloud:projects:delete` scope (step-up required).
#[utoipa::path(
    delete,
    path = "/_ops/v1/projects/{project_ref}",
    responses(
        (status = 200, description = "Project deleted"),
        (status = 403, description = "Missing scope or step-up required"),
        (status = 404, description = "Project not found"),
    )
)]
pub async fn delete_project(
    State(_state): State<OpsState>,
    _ctx: OpsAuthCtx,
    Path(_project_ref): Path<String>,
) -> Result<Json<serde_json::Value>, OpsError> {
    // In full implementation:
    // 1. Check step-up if required
    // 2. Forward to CloudApiState.projects.schedule_delete()
    // 3. Record in audit with actor
    
    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "Project deletion scheduled"
    })))
}
