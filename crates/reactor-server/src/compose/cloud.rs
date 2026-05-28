//! Cloud control plane API endpoints.
//!
//! Mounts at `/_cloud/v1/*` and provides project management, member management,
//! API key issuance, and audit log access. Protected by admin token authentication.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use reactor_cloud_api::{
    AuditService, CloudProvider, CreateProjectRequest, KeyKind, KeyService, MemberRole,
    MemberService, PgProjectStore, ProjectService, SingleNodeConfig, SingleNodeProvider,
};
use reactor_core::Vault;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Shared state for cloud API handlers.
#[derive(Clone)]
pub struct CloudApiState {
    projects: Arc<ProjectService>,
    members: Arc<MemberService>,
    keys: Arc<KeyService>,
    audit: Arc<AuditService>,
    /// Base domain for project hostnames (e.g., "reactor.cloud" or "superscalable.cloud").
    base_domain: String,
}

impl CloudApiState {
    /// Create a new cloud API state.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `vault` - Vault for secrets management  
    /// * `backend_target` - Backend target URL (e.g., "reactor-cloud.internal:8000")
    /// * `base_domain` - Base domain for projects (e.g., "reactor.cloud" or "superscalable.cloud")
    /// * `tls_mode` - TLS mode: "wildcard", "on_demand", or "manual"
    /// * `provider_kind` - Provider type: Some("shared_cluster") for shared or None for single-node
    pub fn new(
        pool: PgPool,
        vault: Arc<dyn Vault>,
        backend_target: String,
        base_domain: String,
        tls_mode: String,
        provider_kind: Option<String>,
        shared_postgres_url: Option<String>,
    ) -> Self {
        let store = Arc::new(PgProjectStore::new(pool.clone()));
        let base_domain_for_state = base_domain.clone();

        let provider: Arc<dyn CloudProvider> = match provider_kind.as_deref() {
            Some("shared_cluster") => {
                use reactor_cloud_api::{SharedClusterConfig, SharedClusterProvider};
                let config = SharedClusterConfig {
                    backend_target,
                    base_domain,
                    tls_mode,
                    default_connection_limit: 5,
                    database_collation: "en_US.utf8".to_string(),
                    database_encoding: "UTF8".to_string(),
                    shared_postgres_url,
                };
                // For shared cluster, we use the same pool for both control and admin operations.
                // In production, these could be different pools with different privileges.
                Arc::new(SharedClusterProvider::new(
                    pool.clone(),       // control_pool (reactor_cloud schema)
                    pool.clone(),       // admin_pool (CREATEDB privileges)
                    vault.clone(),
                    store.clone(),
                    config,
                ))
            }
            _ => {
                let config = SingleNodeConfig {
                    backend_target,
                    base_domain,
                    tls_mode,
                };
                Arc::new(SingleNodeProvider::new(
                    pool,
                    vault.clone(),
                    store.clone(),
                    config,
                ))
            }
        };

        let projects = Arc::new(ProjectService::new(store.clone(), provider));
        let members = Arc::new(MemberService::new(store.clone()));
        let keys = Arc::new(KeyService::new(store.clone(), vault));
        let audit = Arc::new(AuditService::new(store));

        Self {
            projects,
            members,
            keys,
            audit,
            base_domain: base_domain_for_state,
        }
    }

    /// Get a reference to the project service.
    pub fn projects(&self) -> &ProjectService {
        &self.projects
    }

    /// Get the base domain for project hostnames.
    pub fn base_domain(&self) -> &str {
        &self.base_domain
    }
}

/// Build the cloud API router.
///
/// All endpoints require admin token authentication (handled by middleware).
pub fn router() -> Router<CloudApiState> {
    Router::new()
        // Projects
        .route("/projects", post(create_project))
        .route("/projects", get(list_projects))
        .route("/projects/:ref", get(get_project))
        .route("/projects/:ref", delete(delete_project))
        // Members
        .route("/projects/:ref/members", get(list_members))
        .route("/projects/:ref/members", post(add_member))
        .route("/projects/:ref/members/:user_id", delete(remove_member))
        // Keys
        .route("/projects/:ref/keys", get(list_keys))
        .route("/projects/:ref/keys", post(create_key))
        .route("/projects/:ref/keys/:key_id/rotate", post(rotate_key))
        .route("/projects/:ref/keys/:key_id", delete(revoke_key))
        // Audit
        .route("/projects/:ref/audit", get(get_project_audit))
        .route("/audit", get(get_global_audit))
}

// ============================================================================
// Request/Response types
// ============================================================================

/// Request to create a project.
#[derive(Debug, Deserialize)]
pub struct CreateProjectReq {
    /// Project name.
    pub name: String,
    /// Deployment region (default: "iad").
    pub region: Option<String>,
    /// Owner user ID.
    pub owner_user_id: Uuid,
}

/// Response for project creation.
#[derive(Debug, Serialize)]
pub struct CreateProjectResp {
    /// Project information.
    pub project: ProjectResp,
    /// Anon API key (returned once).
    pub anon_key: String,
    /// Service API key (returned once).
    pub service_key: String,
}

/// Project response.
#[derive(Debug, Serialize)]
pub struct ProjectResp {
    /// Project ID.
    pub id: Uuid,
    /// Project ref (subdomain).
    #[serde(rename = "ref")]
    pub project_ref: String,
    /// Project name.
    pub name: String,
    /// Owner user ID.
    pub owner_user_id: Uuid,
    /// Backend kind.
    pub backend_kind: String,
    /// Current status.
    pub status: String,
    /// Deployment region.
    pub region: String,
    /// Hostname.
    pub hostname: String,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ProjectResp {
    /// Create a ProjectResp from a Project using the given base domain for hostname.
    pub fn from_project(p: reactor_cloud_api::Project, base_domain: &str) -> Self {
        Self {
            hostname: p.hostname_for(base_domain),
            id: p.id,
            project_ref: p.project_ref,
            name: p.name,
            owner_user_id: p.owner_user_id,
            backend_kind: p.backend_kind,
            status: p.status,
            region: p.region,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

impl From<reactor_cloud_api::Project> for ProjectResp {
    fn from(p: reactor_cloud_api::Project) -> Self {
        Self::from_project(p, "reactor.cloud")
    }
}

/// Query params for listing projects.
#[derive(Debug, Deserialize)]
pub struct ListProjectsQuery {
    /// Filter by owner.
    pub owner: Option<Uuid>,
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

/// Request to add a member.
#[derive(Debug, Deserialize)]
pub struct AddMemberReq {
    /// User ID.
    pub user_id: Uuid,
    /// Member role.
    pub role: String,
}

/// Member response.
#[derive(Debug, Serialize)]
pub struct MemberResp {
    /// Project ID.
    pub project_id: Uuid,
    /// User ID.
    pub user_id: Uuid,
    /// Member role.
    pub role: String,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<reactor_cloud_api::ProjectMember> for MemberResp {
    fn from(m: reactor_cloud_api::ProjectMember) -> Self {
        Self {
            project_id: m.project_id,
            user_id: m.user_id,
            role: m.role,
            created_at: m.created_at,
        }
    }
}

/// Request to create a key.
#[derive(Debug, Deserialize)]
pub struct CreateKeyReq {
    /// Key kind.
    pub kind: String,
}

/// Key response.
#[derive(Debug, Serialize)]
pub struct KeyResp {
    /// Key ID.
    pub id: Uuid,
    /// Project ID.
    pub project_id: Uuid,
    /// Key kind.
    pub kind: String,
    /// Whether the key is active.
    pub active: bool,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Revocation timestamp (if revoked).
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<reactor_cloud_api::ProjectKey> for KeyResp {
    fn from(k: reactor_cloud_api::ProjectKey) -> Self {
        let active = k.is_active();
        Self {
            id: k.id,
            project_id: k.project_id,
            kind: k.kind,
            active,
            created_at: k.created_at,
            revoked_at: k.revoked_at,
        }
    }
}

/// Key creation response (includes value).
#[derive(Debug, Serialize)]
pub struct CreateKeyResp {
    /// Key metadata.
    pub key: KeyResp,
    /// Key value (returned once).
    pub value: String,
}

/// Audit entry response.
#[derive(Debug, Serialize)]
pub struct AuditResp {
    /// Entry ID.
    pub id: i64,
    /// Project ID.
    pub project_id: Option<Uuid>,
    /// Actor.
    pub actor: String,
    /// Action.
    pub action: String,
    /// Metadata.
    pub metadata: serde_json::Value,
    /// Timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<reactor_cloud_api::AuditEntry> for AuditResp {
    fn from(e: reactor_cloud_api::AuditEntry) -> Self {
        Self {
            id: e.id,
            project_id: e.project_id,
            actor: e.actor,
            action: e.action,
            metadata: e.metadata,
            created_at: e.created_at,
        }
    }
}

/// Query params for audit logs.
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub offset: i32,
}

// ============================================================================
// Handlers
// ============================================================================

/// Create a new project.
#[instrument(skip(state))]
async fn create_project(
    State(state): State<CloudApiState>,
    Json(req): Json<CreateProjectReq>,
) -> Result<impl IntoResponse, CloudApiError> {
    let result = state
        .projects
        .create(CreateProjectRequest {
            name: req.name,
            region: req.region,
            owner_user_id: req.owner_user_id,
        })
        .await?;

    let base_domain = state.base_domain();
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "ok": true,
            "data": CreateProjectResp {
                project: ProjectResp::from_project(result.project, base_domain),
                anon_key: result.anon_key,
                service_key: result.service_key,
            }
        })),
    ))
}

/// List projects.
#[instrument(skip(state))]
async fn list_projects(
    State(state): State<CloudApiState>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<impl IntoResponse, CloudApiError> {
    let projects = match query.owner {
        Some(owner_id) => {
            state
                .projects
                .list_for_user(owner_id, query.limit, query.offset)
                .await?
        }
        None => {
            state
                .projects
                .list_all(query.limit, query.offset)
                .await?
        }
    };

    let base_domain = state.base_domain();
    let data: Vec<ProjectResp> = projects
        .into_iter()
        .map(|p| ProjectResp::from_project(p, base_domain))
        .collect();

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

/// Get a project by ref.
#[instrument(skip(state))]
async fn get_project(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
) -> Result<impl IntoResponse, CloudApiError> {
    let project = state
        .projects
        .get_by_ref(&project_ref)
        .await?
        .ok_or(CloudApiError::NotFound(format!(
            "project not found: {}",
            project_ref
        )))?;

    let data = ProjectResp::from_project(project, state.base_domain());

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

/// Delete a project (soft delete).
#[instrument(skip(state))]
async fn delete_project(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
) -> Result<impl IntoResponse, CloudApiError> {
    let project = state
        .projects
        .schedule_delete(&project_ref, "admin")
        .await?;

    let data = ProjectResp::from_project(project, state.base_domain());

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

/// List project members.
#[instrument(skip(state))]
async fn list_members(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
) -> Result<impl IntoResponse, CloudApiError> {
    let members = state.members.list(&project_ref).await?;
    let data: Vec<MemberResp> = members.into_iter().map(Into::into).collect();

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

/// Add a member to a project.
#[instrument(skip(state))]
async fn add_member(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
    Json(req): Json<AddMemberReq>,
) -> Result<impl IntoResponse, CloudApiError> {
    let role: MemberRole = req.role.parse().map_err(|_| {
        CloudApiError::BadRequest(format!("invalid role: {}", req.role))
    })?;

    let member = state
        .members
        .add(&project_ref, req.user_id, role, "admin")
        .await?;

    let data: MemberResp = member.into();

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "ok": true,
            "data": data
        })),
    ))
}

/// Remove a member from a project.
#[instrument(skip(state))]
async fn remove_member(
    State(state): State<CloudApiState>,
    Path((project_ref, user_id)): Path<(String, Uuid)>,
) -> Result<impl IntoResponse, CloudApiError> {
    state.members.remove(&project_ref, user_id, "admin").await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": { "removed": user_id }
    })))
}

/// List project keys.
#[instrument(skip(state))]
async fn list_keys(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
) -> Result<impl IntoResponse, CloudApiError> {
    let keys = state.keys.list(&project_ref).await?;
    let data: Vec<KeyResp> = keys.into_iter().map(Into::into).collect();

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

/// Create a new key.
#[instrument(skip(state))]
async fn create_key(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
    Json(req): Json<CreateKeyReq>,
) -> Result<impl IntoResponse, CloudApiError> {
    let kind: KeyKind = req.kind.parse().map_err(|_| {
        CloudApiError::BadRequest(format!("invalid key kind: {}", req.kind))
    })?;

    let result = state.keys.create(&project_ref, kind, "admin").await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "ok": true,
            "data": CreateKeyResp {
                key: result.key.into(),
                value: result.value,
            }
        })),
    ))
}

/// Rotate a key.
#[instrument(skip(state))]
async fn rotate_key(
    State(state): State<CloudApiState>,
    Path((project_ref, key_id)): Path<(String, Uuid)>,
) -> Result<impl IntoResponse, CloudApiError> {
    let result = state.keys.rotate(&project_ref, key_id, "admin").await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": CreateKeyResp {
            key: result.key.into(),
            value: result.value,
        }
    })))
}

/// Revoke a key.
#[instrument(skip(state))]
async fn revoke_key(
    State(state): State<CloudApiState>,
    Path((project_ref, key_id)): Path<(String, Uuid)>,
) -> Result<impl IntoResponse, CloudApiError> {
    state.keys.revoke(&project_ref, key_id, "admin").await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": { "revoked": key_id }
    })))
}

/// Get audit log for a project.
#[instrument(skip(state))]
async fn get_project_audit(
    State(state): State<CloudApiState>,
    Path(project_ref): Path<String>,
    Query(query): Query<AuditQuery>,
) -> Result<impl IntoResponse, CloudApiError> {
    let entries = state
        .audit
        .get_for_project(&project_ref, query.limit, query.offset)
        .await?;

    let data: Vec<AuditResp> = entries.into_iter().map(Into::into).collect();

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

/// Get global audit log.
#[instrument(skip(state))]
async fn get_global_audit(
    State(state): State<CloudApiState>,
    Query(query): Query<AuditQuery>,
) -> Result<impl IntoResponse, CloudApiError> {
    let entries = state.audit.get_all(query.limit, query.offset).await?;
    let data: Vec<AuditResp> = entries.into_iter().map(Into::into).collect();

    Ok(Json(serde_json::json!({
        "ok": true,
        "data": data
    })))
}

// ============================================================================
// Error handling
// ============================================================================

/// Cloud API error type.
#[derive(Debug)]
pub enum CloudApiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
    Cloud(reactor_cloud_api::CloudError),
}

impl From<reactor_cloud_api::CloudError> for CloudApiError {
    fn from(e: reactor_cloud_api::CloudError) -> Self {
        Self::Cloud(e)
    }
}

impl IntoResponse for CloudApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match &self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg.clone()),
            Self::Cloud(e) => {
                use reactor_cloud_api::CloudError;
                match e {
                    CloudError::ProjectNotFound(ref s) => {
                        (StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", s.clone())
                    }
                    CloudError::ProjectAlreadyExists(ref s) => {
                        (StatusCode::CONFLICT, "PROJECT_EXISTS", s.clone())
                    }
                    CloudError::MemberNotFound { .. } => {
                        (StatusCode::NOT_FOUND, "MEMBER_NOT_FOUND", e.to_string())
                    }
                    CloudError::KeyNotFound(_) => {
                        (StatusCode::NOT_FOUND, "KEY_NOT_FOUND", e.to_string())
                    }
                    CloudError::InvalidStatusTransition { .. } => {
                        (StatusCode::BAD_REQUEST, "INVALID_STATUS", e.to_string())
                    }
                    CloudError::InvalidArgument(ref s) => {
                        (StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", s.clone())
                    }
                    CloudError::PermissionDenied(ref s) => {
                        (StatusCode::FORBIDDEN, "PERMISSION_DENIED", s.clone())
                    }
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", e.to_string()),
                }
            }
        };

        let body = serde_json::json!({
            "ok": false,
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
