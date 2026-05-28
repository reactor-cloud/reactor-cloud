//! API key management endpoints.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reactor_core::auth::AuthError;
use reactor_core::ReactorId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Request body for creating an API key.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    /// A name for the key (for identification).
    pub name: String,
    /// Optional scopes to limit key permissions.
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
}

/// Response for creating an API key.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateApiKeyResponse {
    /// The API key ID.
    pub id: String,
    /// The full API key (shown only once).
    pub key: String,
    /// The key name.
    pub name: String,
    /// The key prefix (for identification).
    pub prefix: String,
    /// The key scopes.
    pub scopes: Option<Vec<String>>,
    /// When the key was created.
    pub created_at: String,
}

/// Response for listing API keys.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyResponse {
    /// The API key ID.
    pub id: String,
    /// The key name.
    pub name: String,
    /// The key prefix (for identification).
    pub prefix: String,
    /// The key scopes.
    pub scopes: Option<Vec<String>>,
    /// When the key was created.
    pub created_at: String,
    /// When the key was last used.
    pub last_used_at: Option<String>,
}

/// Response for listing all API keys.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListApiKeysResponse {
    /// The list of API keys.
    pub keys: Vec<ApiKeyResponse>,
}

/// Response for revoking an API key.
#[derive(Debug, Serialize, ToSchema)]
pub struct RevokeApiKeyResponse {
    /// Message indicating the result.
    pub message: String,
}

/// GET /auth/v1/api-keys
///
/// List all API keys for the authenticated user.
#[utoipa::path(
    get,
    path = "/auth/v1/api-keys",
    tag = "auth",
    responses(
        (status = 200, description = "List of API keys", body = ListApiKeysResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn list_api_keys<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;
    let keys = service.list_api_keys(&user_id).await?;

    let response = ListApiKeysResponse {
        keys: keys
            .into_iter()
            .map(|k| ApiKeyResponse {
                id: k.id.to_string(),
                name: k.name,
                prefix: k.prefix,
                scopes: k.scopes,
                created_at: k.created_at.to_rfc3339(),
                last_used_at: k.last_used_at.map(|t| t.to_rfc3339()),
            })
            .collect(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// POST /auth/v1/api-keys
///
/// Create a new API key. The full key is returned only once.
#[utoipa::path(
    post,
    path = "/auth/v1/api-keys",
    tag = "auth",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API key created", body = CreateApiKeyResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn create_api_key<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;
    let (key, api_key) = service
        .create_api_key(&user_id, &req.name, req.scopes)
        .await?;

    let response = CreateApiKeyResponse {
        id: api_key.id.to_string(),
        key,
        name: api_key.name,
        prefix: api_key.prefix,
        scopes: api_key.scopes,
        created_at: api_key.created_at.to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// DELETE /auth/v1/api-keys/:id
///
/// Revoke an API key.
#[utoipa::path(
    delete,
    path = "/auth/v1/api-keys/{id}",
    tag = "auth",
    params(
        ("id" = String, Path, description = "The API key ID")
    ),
    responses(
        (status = 200, description = "API key revoked", body = RevokeApiKeyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "API key not found"),
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn revoke_api_key<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or(AuthError::Unauthorized)?;
    let key_id: ReactorId = id
        .parse()
        .map_err(|_| AuthError::ValidationError {
            message: "invalid API key ID".to_string(),
        })?;

    service.revoke_api_key(&key_id, &user_id).await?;

    Ok((
        StatusCode::OK,
        Json(RevokeApiKeyResponse {
            message: "API key revoked".to_string(),
        }),
    ))
}
