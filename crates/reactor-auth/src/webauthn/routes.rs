//! WebAuthn HTTP routes.

use super::types::*;
use super::{WebAuthnProvider, WebAuthnStore, WebauthnError};
use crate::error::AppError;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    async_trait,
    extract::{FromRequestParts, Path, State},
    http::{header, request::Parts, StatusCode},
    response::IntoResponse,
    Json,
};
use reactor_core::auth::{AuthError, Claims};
use std::sync::Arc;
use webauthn_rs::prelude::*;

/// WebAuthn route state.
#[derive(Clone)]
pub struct WebAuthnState<S: IdentityStore> {
    /// WebAuthn provider for cryptographic operations.
    pub provider: WebAuthnProvider,
    /// WebAuthn store for database operations.
    pub store: WebAuthnStore,
    /// Auth service for user and token operations.
    pub auth_service: Arc<AuthService<S>>,
}

/// Bearer token extractor for WebAuthn routes.
pub struct WebAuthnBearer {
    /// The verified claims from the JWT.
    pub claims: Claims,
}

#[async_trait]
impl<S: IdentityStore> FromRequestParts<WebAuthnState<S>> for WebAuthnBearer {
    type Rejection = crate::extract::AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &WebAuthnState<S>,
    ) -> Result<Self, Self::Rejection> {
        // Get Authorization header
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or(crate::extract::AuthRejection(AuthError::Unauthorized))?
            .to_str()
            .map_err(|_| crate::extract::AuthRejection(AuthError::Unauthorized))?;

        // Parse Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(crate::extract::AuthRejection(AuthError::Unauthorized))?;

        // Verify token using the auth_service from WebAuthnState
        let claims = state.auth_service.verify_token(token).await
            .map_err(crate::extract::AuthRejection)?;

        Ok(Self { claims })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Registration
// ─────────────────────────────────────────────────────────────────────────────

/// POST /auth/v1/webauthn/register/start
///
/// Start a WebAuthn registration ceremony.
pub async fn register_start<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
    Json(_req): Json<StartRegistrationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    // Get existing credentials to exclude them
    let existing = state.store.find_credentials_by_user(&user_id).await
        .map_err(|_e| AppError::Auth(AuthError::Internal))?;

    let exclude_credentials: Vec<CredentialID> = existing
        .iter()
        .map(|c| CredentialID::from(c.credential_id.clone()))
        .collect();

    // Get user info
    let user = state.auth_service.get_user(&user_id).await?;

    // Start registration
    let (challenge_response, registration_state) = state.provider
        .start_registration(
            user_id.as_uuid().as_bytes(),
            &user.email,
            &user.email,
            exclude_credentials,
        )
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to start webauthn registration");
            AppError::Auth(AuthError::Internal)
        })?;

    // Serialize the registration state
    let state_bytes = serde_json::to_vec(&registration_state)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to serialize registration state");
            AppError::Auth(AuthError::Internal)
        })?;

    // Store the challenge
    let session_id = uuid::Uuid::new_v4();
    state.store
        .create_challenge(
            session_id,
            challenge_response.public_key.challenge.as_ref(),
            ChallengeType::Registration,
            Some(&user_id),
            &state_bytes,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to store webauthn challenge");
            AppError::Auth(AuthError::Internal)
        })?;

    // Convert to JSON for the response
    let options = serde_json::to_value(&challenge_response)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to serialize challenge response");
            AppError::Auth(AuthError::Internal)
        })?;

    Ok((
        StatusCode::OK,
        Json(StartRegistrationResponse {
            session_id: session_id.to_string(),
            options,
        }),
    ))
}

/// POST /auth/v1/webauthn/register/finish
///
/// Finish a WebAuthn registration ceremony.
pub async fn register_finish<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
    Json(req): Json<FinishRegistrationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    // Parse session ID
    let session_id: uuid::Uuid = req.session_id.parse()
        .map_err(|_| AppError::Auth(AuthError::InvalidToken))?;

    // Consume the challenge
    let challenge = state.store
        .consume_challenge(session_id, ChallengeType::Registration)
        .await
        .map_err(|e| match e {
            WebauthnError::ChallengeNotFound => AppError::Auth(AuthError::InvalidToken),
            _ => {
                tracing::error!(error = ?e, "failed to consume webauthn challenge");
                AppError::Auth(AuthError::Internal)
            }
        })?;

    // Verify user matches
    if challenge.user_id != Some(user_id) {
        return Err(AppError::Auth(AuthError::PermissionDenied));
    }

    // Deserialize the registration state
    let registration_state: PasskeyRegistration = serde_json::from_slice(&challenge.state)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to deserialize registration state");
            AppError::Auth(AuthError::Internal)
        })?;

    // Parse the credential response
    let credential_response: RegisterPublicKeyCredential = serde_json::from_value(req.credential)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to parse credential response");
            AppError::Auth(AuthError::ValidationError {
                message: "Invalid credential response".to_string(),
            })
        })?;

    // Finish registration
    let passkey = state.provider
        .finish_registration(&credential_response, &registration_state)
        .map_err(|e| {
            tracing::error!(error = ?e, "webauthn registration verification failed");
            AppError::Auth(AuthError::ValidationError {
                message: format!("Registration verification failed: {:?}", e),
            })
        })?;

    // Extract credential data
    let cred_id = passkey.cred_id().to_vec();
    let public_key = serde_json::to_vec(&passkey)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to serialize passkey");
            AppError::Auth(AuthError::Internal)
        })?;

    // Store the credential
    let credential = state.store
        .create_credential(
            &user_id,
            &cred_id,
            &public_key,
            None, // AAGUID extracted from passkey if needed
            0, // Initial counter
            vec![], // Transports
            req.name.as_deref(),
        )
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to store webauthn credential");
            AppError::Auth(AuthError::Internal)
        })?;

    tracing::info!(
        user_id = %user_id,
        credential_id = %credential.id,
        "webauthn credential registered"
    );

    Ok((
        StatusCode::OK,
        Json(FinishRegistrationResponse {
            credential_id: credential.id.to_string(),
            message: "Passkey registered successfully".to_string(),
        }),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Authentication
// ─────────────────────────────────────────────────────────────────────────────

/// POST /auth/v1/webauthn/authenticate/start
///
/// Start a WebAuthn authentication ceremony for step-up.
pub async fn authenticate_start<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
    Json(_req): Json<StartAuthenticationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    // Get user's credentials
    let credentials = state.store.find_credentials_by_user(&user_id).await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to find webauthn credentials");
            AppError::Auth(AuthError::Internal)
        })?;

    if credentials.is_empty() {
        return Err(AppError::Auth(AuthError::ValidationError {
            message: "No passkeys registered. Please register a passkey first.".to_string(),
        }));
    }

    // Convert stored credentials to Passkeys
    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| serde_json::from_slice(&c.public_key).ok())
        .collect();

    if passkeys.is_empty() {
        return Err(AppError::Auth(AuthError::Internal));
    }

    // Start authentication
    let (challenge_response, auth_state) = state.provider
        .start_authentication(&passkeys)
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to start webauthn authentication");
            AppError::Auth(AuthError::Internal)
        })?;

    // Serialize the authentication state
    let state_bytes = serde_json::to_vec(&auth_state)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to serialize authentication state");
            AppError::Auth(AuthError::Internal)
        })?;

    // Store the challenge
    let session_id = uuid::Uuid::new_v4();
    state.store
        .create_challenge(
            session_id,
            challenge_response.public_key.challenge.as_ref(),
            ChallengeType::Authentication,
            Some(&user_id),
            &state_bytes,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to store webauthn challenge");
            AppError::Auth(AuthError::Internal)
        })?;

    // Convert to JSON for the response
    let options = serde_json::to_value(&challenge_response)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to serialize challenge response");
            AppError::Auth(AuthError::Internal)
        })?;

    Ok((
        StatusCode::OK,
        Json(StartAuthenticationResponse {
            session_id: session_id.to_string(),
            options,
        }),
    ))
}

/// POST /auth/v1/webauthn/authenticate/finish
///
/// Finish a WebAuthn authentication ceremony and get new tokens with mfa_at.
pub async fn authenticate_finish<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
    Json(req): Json<FinishAuthenticationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    // Parse session ID
    let session_id: uuid::Uuid = req.session_id.parse()
        .map_err(|_| AppError::Auth(AuthError::InvalidToken))?;

    // Consume the challenge
    let challenge = state.store
        .consume_challenge(session_id, ChallengeType::Authentication)
        .await
        .map_err(|e| match e {
            WebauthnError::ChallengeNotFound => AppError::Auth(AuthError::InvalidToken),
            _ => {
                tracing::error!(error = ?e, "failed to consume webauthn challenge");
                AppError::Auth(AuthError::Internal)
            }
        })?;

    // Verify user matches
    if challenge.user_id != Some(user_id) {
        return Err(AppError::Auth(AuthError::PermissionDenied));
    }

    // Deserialize the authentication state
    let auth_state: PasskeyAuthentication = serde_json::from_slice(&challenge.state)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to deserialize authentication state");
            AppError::Auth(AuthError::Internal)
        })?;

    // Parse the credential response
    let credential_response: PublicKeyCredential = serde_json::from_value(req.credential)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to parse credential response");
            AppError::Auth(AuthError::ValidationError {
                message: "Invalid credential response".to_string(),
            })
        })?;

    // Finish authentication
    let auth_result = state.provider
        .finish_authentication(&credential_response, &auth_state)
        .map_err(|e| {
            tracing::error!(error = ?e, "webauthn authentication verification failed");
            AppError::Auth(AuthError::InvalidCredentials)
        })?;

    // Update the credential counter
    let cred_id = auth_result.cred_id().to_vec();
    state.store
        .update_credential_counter(&cred_id, auth_result.counter().into())
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to update credential counter");
            AppError::Auth(AuthError::Internal)
        })?;

    // Get the session ID from claims
    let session_id = auth.claims.session_id.ok_or_else(|| {
        tracing::error!("no session_id in claims for mfa token issuance");
        AppError::Auth(AuthError::InvalidToken)
    })?;

    // Preserve scopes from current token
    let scopes = auth.claims.scopes.clone();

    // Issue new tokens with mfa_at set
    let mfa_response = state.auth_service
        .issue_mfa_tokens(&user_id, &session_id, scopes)
        .await?;

    tracing::info!(
        user_id = %user_id,
        mfa_at = mfa_response.mfa_at,
        "webauthn authentication successful, issued MFA tokens"
    );

    Ok((
        StatusCode::OK,
        Json(FinishAuthenticationResponse {
            success: true,
            credential_id: base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &cred_id),
            access_token: mfa_response.access_token,
            refresh_token: mfa_response.refresh_token,
            expires_at: mfa_response.expires_at.to_rfc3339(),
        }),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Credential Management
// ─────────────────────────────────────────────────────────────────────────────

/// GET /auth/v1/webauthn/credentials
///
/// List user's WebAuthn credentials.
pub async fn list_credentials<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    let credentials = state.store.find_credentials_by_user(&user_id).await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to list webauthn credentials");
            AppError::Auth(AuthError::Internal)
        })?;

    let credential_infos: Vec<CredentialInfo> = credentials
        .into_iter()
        .map(|c| CredentialInfo {
            id: c.id.to_string(),
            name: c.name,
            created_at: c.created_at.to_rfc3339(),
            last_used_at: c.last_used_at.map(|t| t.to_rfc3339()),
            transports: c.transports,
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(ListCredentialsResponse {
            credentials: credential_infos,
        }),
    ))
}

/// DELETE /auth/v1/webauthn/credentials/:id
///
/// Delete a WebAuthn credential.
pub async fn delete_credential<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
    Path(credential_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    let cred_id: reactor_core::ReactorId = credential_id.parse()
        .map_err(|_| AppError::Auth(AuthError::ValidationError {
            message: "Invalid credential ID".to_string(),
        }))?;

    state.store.delete_credential(&cred_id, &user_id).await
        .map_err(|e| match e {
            WebauthnError::CredentialNotFound => AppError::Auth(AuthError::ValidationError {
                message: "Credential not found".to_string(),
            }),
            _ => {
                tracing::error!(error = ?e, "failed to delete webauthn credential");
                AppError::Auth(AuthError::Internal)
            }
        })?;

    tracing::info!(
        user_id = %user_id,
        credential_id = %cred_id,
        "webauthn credential deleted"
    );

    Ok((
        StatusCode::OK,
        Json(DeleteCredentialResponse {
            message: "Credential deleted successfully".to_string(),
        }),
    ))
}

/// PATCH /auth/v1/webauthn/credentials/:id
///
/// Rename a WebAuthn credential.
pub async fn rename_credential<S: IdentityStore>(
    State(state): State<WebAuthnState<S>>,
    auth: WebAuthnBearer,
    Path(credential_id): Path<String>,
    Json(req): Json<RenameCredentialRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.claims.user_id().ok_or_else(|| {
        AppError::Auth(AuthError::PermissionDenied)
    })?;

    let cred_id: reactor_core::ReactorId = credential_id.parse()
        .map_err(|_| AppError::Auth(AuthError::ValidationError {
            message: "Invalid credential ID".to_string(),
        }))?;

    state.store.rename_credential(&cred_id, &user_id, &req.name).await
        .map_err(|e| match e {
            WebauthnError::CredentialNotFound => AppError::Auth(AuthError::ValidationError {
                message: "Credential not found".to_string(),
            }),
            _ => {
                tracing::error!(error = ?e, "failed to rename webauthn credential");
                AppError::Auth(AuthError::Internal)
            }
        })?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Credential renamed successfully" })),
    ))
}
