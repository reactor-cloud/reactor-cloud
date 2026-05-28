//! JWKS endpoint.

use crate::error::AppError;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::store::IdentityStore;
use crate::token::KeyringManager;

/// State needed for the keys endpoint.
pub struct KeysState<S: IdentityStore> {
    /// Keyring manager.
    pub keyring: Arc<KeyringManager<S>>,
}

impl<S: IdentityStore> Clone for KeysState<S> {
    fn clone(&self) -> Self {
        Self {
            keyring: self.keyring.clone(),
        }
    }
}

/// GET /auth/v1/keys
///
/// Returns the JSON Web Key Set for JWT verification.
#[utoipa::path(
    get,
    path = "/auth/v1/keys",
    tag = "auth",
    responses(
        (status = 200, description = "JSON Web Key Set"),
    )
)]
pub async fn jwks<S: IdentityStore>(
    State(state): State<KeysState<S>>,
) -> Result<impl IntoResponse, AppError> {
    let keyring = state.keyring.keyring().await;
    let jwks = keyring.to_jwks().map_err(|e| {
        tracing::error!(error = %e, "failed to build JWKS");
        AppError::Internal("failed to build JWKS".to_string())
    })?;

    Ok((StatusCode::OK, Json(jwks)))
}

/// Minimal OIDC discovery response.
#[derive(serde::Serialize, ToSchema)]
pub struct OidcDiscovery {
    issuer: String,
    jwks_uri: String,
    response_types_supported: Vec<&'static str>,
    subject_types_supported: Vec<&'static str>,
    id_token_signing_alg_values_supported: Vec<&'static str>,
}

/// GET /auth/v1/.well-known/openid-configuration
///
/// Minimal OIDC discovery document.
#[utoipa::path(
    get,
    path = "/auth/v1/.well-known/openid-configuration",
    tag = "auth",
    responses(
        (status = 200, description = "OIDC discovery document", body = OidcDiscovery),
    )
)]
pub async fn openid_configuration(public_url: String) -> impl IntoResponse {
    let discovery = OidcDiscovery {
        issuer: format!("{}/auth/v1", public_url),
        jwks_uri: format!("{}/auth/v1/keys", public_url),
        response_types_supported: vec!["code"],
        subject_types_supported: vec!["public"],
        id_token_signing_alg_values_supported: vec!["RS256"],
    };

    (StatusCode::OK, Json(discovery))
}
