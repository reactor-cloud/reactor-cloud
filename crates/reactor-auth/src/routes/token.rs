//! Token endpoint — password grant, refresh token, and authorization code (PKCE).

use crate::error::AppError;
use crate::routes::signup::UserResponse;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use reactor_core::auth::AuthError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Token request query params.
#[derive(Debug, Deserialize, IntoParams)]
pub struct TokenQuery {
    /// Grant type: "password", "refresh_token", or "authorization_code".
    pub grant_type: String,
}

/// Password grant request body.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PasswordGrantRequest {
    /// User's email address.
    pub email: String,
    /// User's password.
    pub password: String,
}

/// Refresh token request body.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshTokenRequest {
    /// The refresh token.
    pub refresh_token: String,
}

/// Authorization code grant request body (PKCE).
#[derive(Debug, Deserialize, ToSchema)]
pub struct AuthorizationCodeRequest {
    /// The authorization code.
    pub code: String,
    /// The PKCE code verifier.
    pub code_verifier: String,
    /// The client ID.
    pub client_id: String,
    /// The redirect URI (must match the one used in /authorize).
    pub redirect_uri: String,
}

/// Token response.
///
/// Conforms to [RFC 6749 §5.1](https://www.rfc-editor.org/rfc/rfc6749#section-5.1)
/// (`token_type`, `expires_in`, `scope`) while also exposing legacy fields
/// (`expires_at`, `user`) that first-party callers depend on.
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenResponse {
    /// JWT access token.
    pub access_token: String,
    /// Token type — always `"Bearer"`.
    #[serde(default = "default_token_type")]
    pub token_type: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// Seconds until `access_token` expires (OAuth 2.0 standard).
    pub expires_in: i64,
    /// When the access token expires (RFC3339; legacy).
    pub expires_at: String,
    /// Space-separated scopes granted on this access token (OAuth 2.0 standard).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scope: String,
    /// The user (included for password and authorization_code grants).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserResponse>,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

/// Compute `expires_in` seconds from an absolute expiry timestamp.
fn expires_in_secs(expires_at: chrono::DateTime<chrono::Utc>) -> i64 {
    (expires_at - chrono::Utc::now()).num_seconds().max(0)
}

/// POST /auth/v1/token?grant_type=password
pub async fn token_password<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    headers: header::HeaderMap,
    Json(req): Json<PasswordGrantRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim());

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());

    let auth_response = service
        .password_grant(&req.email, &req.password, ip, user_agent)
        .await?;

    let response = build_token_response(auth_response, true);

    Ok((StatusCode::OK, Json(response)))
}

/// POST /auth/v1/token?grant_type=refresh_token
pub async fn token_refresh<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Json(req): Json<RefreshTokenRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_response = service.refresh(&req.refresh_token).await?;

    let response = build_token_response(auth_response, false);

    Ok((StatusCode::OK, Json(response)))
}

/// Build a [`TokenResponse`] from an [`AuthResponse`].
///
/// `include_user` controls whether the user profile is embedded (true for
/// password and authorization_code grants, false for refresh_token).
fn build_token_response(
    auth_response: crate::service::AuthResponse,
    include_user: bool,
) -> TokenResponse {
    let user = if include_user {
        Some(UserResponse {
            id: auth_response.user.id.to_string(),
            email: auth_response.user.email,
            email_verified: auth_response.user.email_verified,
            metadata: auth_response.user.metadata,
            created_at: auth_response.user.created_at.to_rfc3339(),
        })
    } else {
        None
    };

    TokenResponse {
        access_token: auth_response.access_token,
        token_type: default_token_type(),
        refresh_token: auth_response.refresh_token,
        expires_in: expires_in_secs(auth_response.expires_at),
        expires_at: auth_response.expires_at.to_rfc3339(),
        // AuthResponse does not currently expose granted scopes; clients
        // decode the JWT for the canonical scopes claim.
        scope: String::new(),
        user,
    }
}

/// POST /auth/v1/token — dispatches based on `grant_type`.
///
/// Per [RFC 6749 §3.2](https://www.rfc-editor.org/rfc/rfc6749#section-3.2) the
/// OAuth 2.0 token endpoint MUST accept `application/x-www-form-urlencoded`
/// bodies. We accept either:
///
/// - **Form-encoded body** (OAuth-spec-compliant; used by the PKCE / device
///   code flow and the `reactor` CLI): all parameters including `grant_type`
///   live in the request body.
/// - **JSON body + `?grant_type=...` query parameter** (legacy path used by
///   first-party callers): `grant_type` is a query parameter and the rest of
///   the parameters are in the JSON body.
///
/// The handler picks the parsing strategy based on the `Content-Type` header.
#[utoipa::path(
    post,
    path = "/auth/v1/token",
    tag = "auth",
    params(TokenQuery),
    responses(
        (status = 200, description = "Token response", body = TokenResponse),
        (status = 400, description = "Invalid grant type or request"),
        (status = 401, description = "Invalid credentials"),
    )
)]
pub async fn token<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    query: Option<Query<TokenQuery>>,
    headers: header::HeaderMap,
    body: String,
) -> Result<impl IntoResponse, AppError> {
    let is_form = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false);

    let grant_type = if is_form {
        // OAuth-spec-compliant: parse grant_type from form body.
        let probe: TokenQuery = serde_urlencoded::from_str(&body).map_err(|e| {
            AppError::Auth(AuthError::ValidationError {
                message: format!("invalid form body: missing grant_type ({})", e),
            })
        })?;
        probe.grant_type
    } else {
        // Legacy JSON path: grant_type comes from the query string.
        query
            .map(|Query(q)| q.grant_type)
            .ok_or_else(|| {
                AppError::Auth(AuthError::ValidationError {
                    message: "missing grant_type query parameter".to_string(),
                })
            })?
    };

    /// Decode either a form body or a JSON body into the target struct.
    fn decode<T: serde::de::DeserializeOwned>(
        body: &str,
        is_form: bool,
    ) -> Result<T, AppError> {
        let parsed = if is_form {
            serde_urlencoded::from_str::<T>(body)
                .map_err(|e| format!("invalid form body: {}", e))
        } else {
            serde_json::from_str::<T>(body)
                .map_err(|e| format!("invalid JSON body: {}", e))
        };
        parsed.map_err(|message| AppError::Auth(AuthError::ValidationError { message }))
    }

    match grant_type.as_str() {
        "password" => {
            let req: PasswordGrantRequest = decode(&body, is_form)?;

            let ip = headers
                .get("x-forwarded-for")
                .or_else(|| headers.get("x-real-ip"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.split(',').next().unwrap_or(s).trim());

            let user_agent = headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok());

            let auth_response = service
                .password_grant(&req.email, &req.password, ip, user_agent)
                .await?;

            Ok((StatusCode::OK, Json(build_token_response(auth_response, true))))
        }
        "refresh_token" => {
            let req: RefreshTokenRequest = decode(&body, is_form)?;

            let auth_response = service.refresh(&req.refresh_token).await?;

            Ok((StatusCode::OK, Json(build_token_response(auth_response, false))))
        }
        "authorization_code" => {
            let req: AuthorizationCodeRequest = decode(&body, is_form)?;

            let ip = headers
                .get("x-forwarded-for")
                .or_else(|| headers.get("x-real-ip"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.split(',').next().unwrap_or(s).trim());

            let user_agent = headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok());

            let resp = service
                .exchange_authorization_code(
                    &req.code,
                    &req.code_verifier,
                    &req.client_id,
                    &req.redirect_uri,
                    ip,
                    user_agent,
                )
                .await?;

            let response = TokenResponse {
                access_token: resp.access_token,
                token_type: default_token_type(),
                refresh_token: resp.refresh_token,
                expires_in: expires_in_secs(resp.expires_at),
                expires_at: resp.expires_at.to_rfc3339(),
                scope: resp.scopes.join(" "),
                user: Some(UserResponse {
                    id: resp.user.id.to_string(),
                    email: resp.user.email,
                    email_verified: resp.user.email_verified,
                    metadata: resp.user.metadata,
                    created_at: resp.user.created_at.to_rfc3339(),
                }),
            };

            Ok((StatusCode::OK, Json(response)))
        }
        _ => Err(AppError::Auth(AuthError::ValidationError {
            message: format!("unsupported grant_type: {}", grant_type),
        })),
    }
}
