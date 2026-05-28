//! Vault management routes.
//!
//! These routes provide ops-level access to vault operations,
//! replacing the admin-token-gated `/_admin/vault` endpoints.

use crate::error::OpsError;
use crate::middleware::OpsAuthCtx;
use crate::state::OpsState;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request to write a secret.
#[derive(Debug, Deserialize, ToSchema)]
pub struct WriteSecretRequest {
    /// Secret value.
    pub value: String,
}

/// Response for secret operations.
#[derive(Debug, Serialize, ToSchema)]
pub struct SecretResponse {
    /// Secret path.
    pub path: String,
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Optional message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Secret value response (for reads).
#[derive(Debug, Serialize, ToSchema)]
pub struct SecretValueResponse {
    /// Secret path.
    pub path: String,
    /// Secret value.
    pub value: String,
}

/// Read a secret.
///
/// Requires `vault:read` scope.
#[utoipa::path(
    get,
    path = "/_ops/v1/vault/{path}",
    responses(
        (status = 200, description = "Secret value", body = SecretValueResponse),
        (status = 403, description = "Missing scope"),
        (status = 404, description = "Secret not found"),
    )
)]
pub async fn read_secret(
    State(_state): State<OpsState>,
    _ctx: OpsAuthCtx,
    Path(_path): Path<String>,
) -> Result<Json<SecretValueResponse>, OpsError> {
    // In full implementation, this would:
    // 1. Call state.vault.read() 
    // 2. Return the secret value
    Err(OpsError::NotFound)
}

/// Write a secret.
///
/// Requires `vault:write` scope (step-up required).
#[utoipa::path(
    put,
    path = "/_ops/v1/vault/{path}",
    request_body = WriteSecretRequest,
    responses(
        (status = 200, description = "Secret written", body = SecretResponse),
        (status = 403, description = "Missing scope or step-up required"),
    )
)]
pub async fn write_secret(
    State(_state): State<OpsState>,
    _ctx: OpsAuthCtx,
    Path(path): Path<String>,
    Json(_req): Json<WriteSecretRequest>,
) -> Result<Json<SecretResponse>, OpsError> {
    // In full implementation:
    // 1. Check step-up if required
    // 2. Call state.vault.write()
    // 3. Record in audit
    
    Ok(Json(SecretResponse {
        path,
        ok: true,
        message: Some("Secret written (placeholder)".to_string()),
    }))
}

/// Delete a secret.
///
/// Requires `vault:write` scope (step-up required).
#[utoipa::path(
    delete,
    path = "/_ops/v1/vault/{path}",
    responses(
        (status = 200, description = "Secret deleted", body = SecretResponse),
        (status = 403, description = "Missing scope or step-up required"),
        (status = 404, description = "Secret not found"),
    )
)]
pub async fn delete_secret(
    State(_state): State<OpsState>,
    _ctx: OpsAuthCtx,
    Path(path): Path<String>,
) -> Result<Json<SecretResponse>, OpsError> {
    // In full implementation:
    // 1. Check step-up if required
    // 2. Call state.vault.delete()
    // 3. Record in audit
    
    Ok(Json(SecretResponse {
        path,
        ok: true,
        message: Some("Secret deleted (placeholder)".to_string()),
    }))
}
