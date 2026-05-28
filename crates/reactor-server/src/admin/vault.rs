//! Vault admin endpoints for secret management.

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::Engine;
use reactor_core::primitives::vault::{SecretValue, Vault};
use reactor_core::ProjectId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Vault state extension.
#[derive(Clone)]
pub struct VaultState {
    /// The vault instance.
    pub vault: Arc<dyn Vault>,
    /// The tenant ID.
    pub tenant_id: ProjectId,
}

/// Secret info for list response.
#[derive(Serialize)]
pub struct SecretInfo {
    /// Secret name.
    pub name: String,
    /// Current version.
    pub version: u64,
}

/// List secrets response.
#[derive(Serialize)]
pub struct ListSecretsResponse {
    /// Secret keys with versions.
    pub secrets: Vec<SecretInfo>,
}

/// Get secret response.
#[derive(Serialize)]
pub struct GetSecretResponse {
    /// Secret key.
    pub key: String,
    /// Secret value (base64 encoded if binary).
    pub value: String,
    /// Whether the value is base64 encoded.
    pub is_base64: bool,
    /// Secret version.
    pub version: u64,
}

/// Set secret request.
#[derive(Deserialize)]
pub struct SetSecretRequest {
    /// Secret value.
    pub value: String,
    /// Whether the value is base64 encoded.
    #[serde(default)]
    pub is_base64: bool,
}

/// Set secret response.
#[derive(Serialize)]
pub struct SetSecretResponse {
    /// Status.
    pub status: String,
    /// Message.
    pub message: String,
}

/// Error response.
#[derive(Serialize)]
pub struct VaultError {
    /// Error code.
    pub error: String,
    /// Error message.
    pub message: String,
}

impl VaultError {
    fn not_found(key: &str) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self {
                error: "not_found".into(),
                message: format!("secret '{}' not found", key),
            }),
        )
    }

    fn internal(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self {
                error: "internal_error".into(),
                message: message.into(),
            }),
        )
    }
}

/// List all secrets.
pub async fn list_secrets_handler(
    Extension(vault_state): Extension<VaultState>,
) -> impl IntoResponse {
    match vault_state
        .vault
        .list_secrets(&vault_state.tenant_id)
        .await
    {
        Ok(metadata_list) => {
            let secrets = metadata_list
                .into_iter()
                .map(|m| SecretInfo {
                    name: m.name,
                    version: m.version,
                })
                .collect();
            (StatusCode::OK, Json(ListSecretsResponse { secrets })).into_response()
        }
        Err(e) => VaultError::internal(format!("failed to list secrets: {}", e)).into_response(),
    }
}

/// Get a secret by key.
pub async fn get_secret_handler(
    Extension(vault_state): Extension<VaultState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match vault_state
        .vault
        .get_secret(&vault_state.tenant_id, &key)
        .await
    {
        Ok(Some(secret)) => {
            let (value, is_base64) = match String::from_utf8(secret.data.clone()) {
                Ok(s) => (s, false),
                Err(_) => (
                    base64::engine::general_purpose::STANDARD.encode(&secret.data),
                    true,
                ),
            };

            (
                StatusCode::OK,
                Json(GetSecretResponse {
                    key,
                    value,
                    is_base64,
                    version: secret.version,
                }),
            )
                .into_response()
        }
        Ok(None) => VaultError::not_found(&key).into_response(),
        Err(e) => VaultError::internal(format!("failed to get secret: {}", e)).into_response(),
    }
}

/// Set a secret.
pub async fn set_secret_handler(
    Extension(vault_state): Extension<VaultState>,
    Path(key): Path<String>,
    Json(body): Json<SetSecretRequest>,
) -> impl IntoResponse {
    let data = if body.is_base64 {
        match base64::engine::general_purpose::STANDARD.decode(&body.value) {
            Ok(d) => d,
            Err(e) => {
                return VaultError::internal(format!("invalid base64: {}", e)).into_response()
            }
        }
    } else {
        body.value.into_bytes()
    };

    let secret_value = SecretValue::new(data);

    match vault_state
        .vault
        .put_secret(&vault_state.tenant_id, &key, secret_value)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(SetSecretResponse {
                status: "ok".into(),
                message: format!("secret '{}' set", key),
            }),
        )
            .into_response(),
        Err(e) => VaultError::internal(format!("failed to set secret: {}", e)).into_response(),
    }
}

/// Delete a secret.
pub async fn delete_secret_handler(
    Extension(vault_state): Extension<VaultState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match vault_state
        .vault
        .delete_secret(&vault_state.tenant_id, &key)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(SetSecretResponse {
                status: "ok".into(),
                message: format!("secret '{}' deleted", key),
            }),
        )
            .into_response(),
        Err(e) => VaultError::internal(format!("failed to delete secret: {}", e)).into_response(),
    }
}

/// Rotate a transit key.
#[derive(Deserialize)]
pub struct RotateKeyRequest {
    /// Transit key name to rotate.
    pub key_name: String,
}

/// Rotate key response.
#[derive(Serialize)]
pub struct RotateKeyResponse {
    /// Status.
    pub status: String,
    /// New key version.
    pub new_version: u32,
}

/// Rotate a transit encryption key.
pub async fn rotate_key_handler(
    Extension(vault_state): Extension<VaultState>,
    Json(body): Json<RotateKeyRequest>,
) -> impl IntoResponse {
    match vault_state
        .vault
        .rotate_key(&vault_state.tenant_id, &body.key_name)
        .await
    {
        Ok(new_version) => (
            StatusCode::OK,
            Json(RotateKeyResponse {
                status: "ok".into(),
                new_version,
            }),
        )
            .into_response(),
        Err(e) => VaultError::internal(format!("failed to rotate key: {}", e)).into_response(),
    }
}
