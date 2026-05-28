//! Admin routes for project and key management.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::{event_types, write_audit};
use crate::error::AnalyticsError;
use crate::state::{AnalyticsCtx, AnalyticsState};
use crate::store::{AnalyticsStore, Project, ProjectCreate, ProjectKeyCreate, ProjectKeyRecord};

// ---------------------- Request/Response types ----------------------

/// Create project request.
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

/// Project response.
#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            org_id: p.org_id,
            name: p.name,
            created_at: p.created_at,
        }
    }
}

/// Create project key request.
#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub sampling_rate: Option<f64>,
    pub allowed_origins: Option<Vec<String>>,
}

/// Project key response (returned on creation with full key).
#[derive(Debug, Serialize)]
pub struct KeyCreatedResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub key: String,
    pub sampling_rate: f64,
    pub allowed_origins: Option<Vec<String>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Project key response (for listing, without full key).
#[derive(Debug, Serialize)]
pub struct KeyResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub key_last4: String,
    pub sampling_rate: f64,
    pub allowed_origins: Option<Vec<String>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ProjectKeyRecord> for KeyResponse {
    fn from(k: ProjectKeyRecord) -> Self {
        Self {
            id: k.id,
            project_id: k.project_id,
            name: k.name,
            key_prefix: k.key_prefix,
            key_last4: k.key_last4,
            sampling_rate: k.sampling_rate,
            allowed_origins: k.allowed_origins,
            created_at: k.created_at,
            revoked_at: k.revoked_at,
        }
    }
}

// ---------------------- Key generation ----------------------

/// Generate a new project API key with argon2id hash.
fn generate_project_key() -> (String, Vec<u8>, String) {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    use rand::rngs::OsRng;
    use rand::Rng;

    // Generate random key bytes
    let mut key_bytes = [0u8; 24];
    OsRng.fill(&mut key_bytes);

    // Base32 encode (no padding, lowercase)
    let key_body = base32_encode(&key_bytes);
    let full_key = format!("rapk_{}", key_body);
    let key_last4 = full_key[full_key.len() - 4..].to_string();

    // Hash with argon2id
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(full_key.as_bytes(), &salt)
        .expect("argon2 hash should succeed")
        .to_string();

    (full_key, password_hash.into_bytes(), key_last4)
}

/// Hash a key for lookup.
pub fn hash_project_key(key: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.finalize().to_vec()
}

/// Base32 encode bytes (RFC 4648, no padding, lowercase).
fn base32_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut result = String::new();
    let mut buffer = 0u64;
    let mut bits = 0;

    for &byte in data {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            result.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
        }
    }

    if bits > 0 {
        result.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }

    result
}

// ---------------------- Handlers ----------------------

/// Create a new project.
pub async fn create_project<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check permission
    if !ctx.has_permission("analytics:project:create") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:project:create".to_string(),
        ));
    }

    let project = state
        .store
        .create_project(
            ctx.org_id.into(),
            ProjectCreate { name: req.name },
        )
        .await?;

    // Write audit log
    write_audit(
        &state.store,
        &ctx,
        event_types::PROJECT_CREATE,
        Some(project.id),
        serde_json::json!({
            "project_id": project.id,
            "name": project.name
        }),
    )
    .await?;

    let response = ProjectResponse::from(project);
    Ok((StatusCode::CREATED, Json(response)))
}

/// List projects for the organization.
pub async fn list_projects<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
) -> Result<impl IntoResponse, AnalyticsError> {
    if !ctx.has_permission("analytics:project:read") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:project:read".to_string(),
        ));
    }

    let projects = state.store.list_projects(ctx.org_id.into()).await?;
    let response: Vec<ProjectResponse> = projects.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

/// Get a project by ID.
pub async fn get_project<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse, AnalyticsError> {
    if !ctx.has_permission("analytics:project:read") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:project:read".to_string(),
        ));
    }

    let project = state
        .store
        .get_project(project_id)
        .await?
        .ok_or(AnalyticsError::ProjectNotFound)?;

    // Verify org ownership
    let ctx_org_id: Uuid = ctx.org_id.into();
    if project.org_id != ctx_org_id {
        return Err(AnalyticsError::ProjectNotFound);
    }

    let response = ProjectResponse::from(project);
    Ok(Json(response))
}

/// Delete a project (soft delete).
pub async fn delete_project<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse, AnalyticsError> {
    if !ctx.has_permission("analytics:project:delete") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:project:delete".to_string(),
        ));
    }

    // Verify project exists and belongs to org
    let project = state
        .store
        .get_project(project_id)
        .await?
        .ok_or(AnalyticsError::ProjectNotFound)?;

    let ctx_org_id: Uuid = ctx.org_id.into();
    if project.org_id != ctx_org_id {
        return Err(AnalyticsError::ProjectNotFound);
    }

    state.store.delete_project(project_id).await?;

    // Write audit log
    write_audit(
        &state.store,
        &ctx,
        event_types::PROJECT_DELETE,
        Some(project_id),
        serde_json::json!({
            "project_id": project_id,
            "name": project.name
        }),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Create a new project key.
pub async fn create_project_key<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    if !ctx.has_permission("analytics:key:create") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:key:create".to_string(),
        ));
    }

    // Verify project exists and belongs to org
    let project = state
        .store
        .get_project(project_id)
        .await?
        .ok_or(AnalyticsError::ProjectNotFound)?;

    let ctx_org_id: Uuid = ctx.org_id.into();
    if project.org_id != ctx_org_id {
        return Err(AnalyticsError::ProjectNotFound);
    }

    // Generate key
    let (full_key, key_hash, key_last4) = generate_project_key();

    let key_record = state
        .store
        .create_project_key(
            project_id,
            ProjectKeyCreate {
                name: req.name,
                sampling_rate: req.sampling_rate,
                allowed_origins: req.allowed_origins,
            },
            key_hash,
            key_last4,
        )
        .await?;

    // Write audit log
    write_audit(
        &state.store,
        &ctx,
        event_types::KEY_ISSUE,
        Some(project_id),
        serde_json::json!({
            "key_id": key_record.id,
            "key_name": key_record.name,
            "key_last4": key_record.key_last4
        }),
    )
    .await?;

    let response = KeyCreatedResponse {
        id: key_record.id,
        project_id: key_record.project_id,
        name: key_record.name,
        key: full_key,
        sampling_rate: key_record.sampling_rate,
        allowed_origins: key_record.allowed_origins,
        created_at: key_record.created_at,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// List keys for a project.
pub async fn list_project_keys<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse, AnalyticsError> {
    if !ctx.has_permission("analytics:key:read") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:key:read".to_string(),
        ));
    }

    // Verify project exists and belongs to org
    let project = state
        .store
        .get_project(project_id)
        .await?
        .ok_or(AnalyticsError::ProjectNotFound)?;

    let ctx_org_id: Uuid = ctx.org_id.into();
    if project.org_id != ctx_org_id {
        return Err(AnalyticsError::ProjectNotFound);
    }

    let keys = state.store.list_project_keys(project_id).await?;
    let response: Vec<KeyResponse> = keys.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

/// Revoke a project key.
pub async fn revoke_project_key<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Path((project_id, key_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AnalyticsError> {
    if !ctx.has_permission("analytics:key:revoke") && !ctx.has_permission("*") {
        return Err(AnalyticsError::Forbidden(
            "missing permission: analytics:key:revoke".to_string(),
        ));
    }

    // Verify project exists and belongs to org
    let project = state
        .store
        .get_project(project_id)
        .await?
        .ok_or(AnalyticsError::ProjectNotFound)?;

    let ctx_org_id: Uuid = ctx.org_id.into();
    if project.org_id != ctx_org_id {
        return Err(AnalyticsError::ProjectNotFound);
    }

    state.store.revoke_project_key(key_id).await?;

    // Write audit log
    write_audit(
        &state.store,
        &ctx,
        event_types::KEY_REVOKE,
        Some(project_id),
        serde_json::json!({
            "key_id": key_id
        }),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
