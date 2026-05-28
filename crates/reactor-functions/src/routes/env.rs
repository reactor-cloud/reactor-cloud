//! Environment variable routes.
//!
//! Manages per-function environment variables and secrets.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    error::FunctionsError,
    state::{FunctionCtx, FunctionsState},
    store::{FunctionsStore, PgFunctionsStore},
    ENV_KEY_REGEX,
};

/// Request body for setting an environment variable.
#[derive(Debug, Deserialize)]
pub struct SetEnvRequest {
    /// Variable value.
    pub value: String,
    /// Whether this is a secret (encrypted at rest, never returned via API).
    #[serde(default)]
    pub is_secret: bool,
}

/// Response for a single environment variable.
#[derive(Debug, Serialize)]
pub struct EnvVarResponse {
    /// Variable key.
    pub key: String,
    /// Variable value (null for secrets).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Whether this is a secret.
    pub is_secret: bool,
    /// Last update timestamp.
    pub last_updated_at: String,
}

/// Response for listing environment variables.
#[derive(Debug, Serialize)]
pub struct ListEnvResponse {
    /// List of environment variables.
    pub env: Vec<EnvVarResponse>,
}

/// Path parameters for env routes.
#[derive(Debug, Deserialize)]
pub struct EnvPathParams {
    /// Function name.
    pub name: String,
}

/// Path parameters for single env var routes.
#[derive(Debug, Deserialize)]
pub struct EnvKeyPathParams {
    /// Function name.
    pub name: String,
    /// Environment variable key.
    pub key: String,
}

/// PUT /fn/v1/_admin/functions/{name}/env/{key}
///
/// Create or update an environment variable.
pub async fn set_env(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(params): Path<EnvKeyPathParams>,
    Json(body): Json<SetEnvRequest>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", params.name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    // Validate key format
    if !ENV_KEY_REGEX.is_match(&params.key) {
        return Err(FunctionsError::EnvKeyInvalid(format!(
            "'{}' must be uppercase alphanumeric with underscores, starting with letter, max 128 chars",
            params.key
        )));
    }

    // Get the function
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), &params.name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

    // Encrypt if secret, otherwise store plaintext
    let (value_plaintext, value_encrypted) = if body.is_secret {
        let encrypted = encrypt_value(&body.value, &state.config.data_key)?;
        (None, Some(encrypted))
    } else {
        (Some(body.value), None)
    };

    // Upsert the env var
    store
        .upsert_env(
            function.id,
            &params.key,
            value_plaintext,
            value_encrypted,
            body.is_secret,
        )
        .await?;

    // TODO: PR 14 - Record audit event

    Ok(StatusCode::NO_CONTENT)
}

/// GET /fn/v1/_admin/functions/{name}/env
///
/// List all environment variables for a function.
pub async fn list_env(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(params): Path<EnvPathParams>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", params.name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    // Get the function
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), &params.name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

    // Get env vars
    let env_vars = store.get_env(function.id).await?;

    // Convert to response (never return secret values)
    let env: Vec<EnvVarResponse> = env_vars
        .into_iter()
        .map(|e| EnvVarResponse {
            key: e.key,
            value: if e.is_secret { None } else { e.value_plaintext },
            is_secret: e.is_secret,
            last_updated_at: e.last_updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListEnvResponse { env }))
}

/// GET /fn/v1/_admin/functions/{name}/env/{key}
///
/// Get a single environment variable.
pub async fn get_env(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(params): Path<EnvKeyPathParams>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", params.name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    // Get the function
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), &params.name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

    // Get the env var
    let env_var = store
        .get_env_var(function.id, &params.key)
        .await?
        .ok_or_else(|| FunctionsError::EnvKeyInvalid(format!("env var '{}' not found", params.key)))?;

    let response = EnvVarResponse {
        key: env_var.key,
        value: if env_var.is_secret { None } else { env_var.value_plaintext },
        is_secret: env_var.is_secret,
        last_updated_at: env_var.last_updated_at.to_rfc3339(),
    };

    Ok(Json(response))
}

/// DELETE /fn/v1/_admin/functions/{name}/env/{key}
///
/// Delete an environment variable.
pub async fn delete_env(
    State(state): State<FunctionsState>,
    Extension(ctx): Extension<FunctionCtx>,
    Path(params): Path<EnvKeyPathParams>,
) -> Result<impl IntoResponse, FunctionsError> {
    // Check permission
    let permission = format!("functions:{}:admin", params.name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("functions:*:admin") {
        return Err(FunctionsError::PermissionDenied(permission));
    }

    // Get the function
    let store = PgFunctionsStore::new(state.pool.clone());
    let function = store
        .get_function_by_name(ctx.active_org(), &params.name)
        .await?
        .ok_or_else(|| FunctionsError::FunctionNotFound(params.name.clone()))?;

    // Delete the env var
    let deleted = store.delete_env(function.id, &params.key).await?;
    if !deleted {
        return Err(FunctionsError::EnvKeyInvalid(format!(
            "env var '{}' not found",
            params.key
        )));
    }

    // TODO: PR 14 - Record audit event

    Ok(StatusCode::NO_CONTENT)
}

/// Encrypt a value using AES-256-GCM.
///
/// Format: base64(nonce || ciphertext || tag)
/// Key derivation: SHA256(data_key) to get 32 bytes
fn encrypt_value(value: &str, data_key: &str) -> Result<Vec<u8>, FunctionsError> {
    use aes_gcm::{
        aead::{Aead, KeyInit, OsRng},
        Aes256Gcm, Nonce,
    };
    use rand::RngCore;

    // Derive 32-byte key from data_key
    let mut hasher = Sha256::new();
    hasher.update(data_key.as_bytes());
    let key_bytes: [u8; 32] = hasher.finalize().into();

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| FunctionsError::Internal(format!("cipher init failed: {}", e)))?;

    // Generate random nonce (12 bytes for AES-GCM)
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|e| FunctionsError::Internal(format!("encryption failed: {}", e)))?;

    // Combine nonce + ciphertext
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt a value encrypted with encrypt_value.
pub fn decrypt_value(encrypted: &[u8], data_key: &str) -> Result<String, FunctionsError> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    if encrypted.len() < 13 {
        return Err(FunctionsError::Internal("encrypted value too short".to_string()));
    }

    // Derive 32-byte key from data_key
    let mut hasher = Sha256::new();
    hasher.update(data_key.as_bytes());
    let key_bytes: [u8; 32] = hasher.finalize().into();

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| FunctionsError::Internal(format!("cipher init failed: {}", e)))?;

    // Extract nonce and ciphertext
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext = &encrypted[12..];

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| FunctionsError::Internal(format!("decryption failed: {}", e)))?;

    String::from_utf8(plaintext)
        .map_err(|e| FunctionsError::Internal(format!("invalid UTF-8 in decrypted value: {}", e)))
}

/// Load and decrypt all environment variables for a function.
///
/// Returns a map of key -> value (both plaintext and decrypted secrets).
pub async fn load_env_for_invoke(
    store: &PgFunctionsStore,
    function_id: crate::store::FunctionId,
    data_key: &str,
) -> Result<std::collections::HashMap<String, String>, FunctionsError> {
    let env_vars = store.get_env(function_id).await?;
    let mut result = std::collections::HashMap::new();

    for env_var in env_vars {
        let value = if env_var.is_secret {
            if let Some(encrypted) = env_var.value_encrypted {
                decrypt_value(&encrypted, data_key)?
            } else {
                continue; // Skip malformed secret entries
            }
        } else {
            env_var.value_plaintext.unwrap_or_default()
        };
        result.insert(env_var.key, value);
    }

    Ok(result)
}
