//! Admin token authentication middleware.

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use subtle::ConstantTimeEq;

/// Admin auth state.
#[derive(Clone)]
pub struct AdminAuthState {
    /// Expected admin token.
    pub token: String,
    /// Whether to allow remote admin access.
    pub allow_remote: bool,
    /// Functions state (for deploy handler).
    #[cfg(feature = "cap-functions")]
    pub functions: Option<reactor_functions::FunctionsState>,
    /// Sites state (for deploy handler).
    #[cfg(feature = "cap-sites")]
    pub sites: Option<reactor_sites::SitesState>,
    /// Default org slug for sites deployment.
    #[cfg(feature = "cap-sites")]
    pub default_org_slug: String,
}

/// Error response for auth failures.
#[derive(Debug, Serialize)]
pub struct AdminAuthError {
    pub error: String,
    pub message: String,
}

/// Admin authentication middleware.
///
/// Validates:
/// 1. Bearer token matches admin.token
/// 2. If allow_remote=false, source IP must be loopback
pub async fn admin_auth_middleware(
    State(state): State<AdminAuthState>,
    maybe_connect_info: Option<ConnectInfo<SocketAddr>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Check loopback if allow_remote=false
    if !state.allow_remote {
        let is_loopback = maybe_connect_info
            .as_ref()
            .map(|ci| ci.0.ip().is_loopback())
            .unwrap_or(false);

        if !is_loopback {
            return (
                StatusCode::FORBIDDEN,
                Json(AdminAuthError {
                    error: "forbidden".to_string(),
                    message: "admin access restricted to loopback".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Extract and validate bearer token
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AdminAuthError {
                    error: "unauthorized".to_string(),
                    message: "missing or invalid Authorization header".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Constant-time comparison to prevent timing attacks
    let expected = state.token.as_bytes();
    let provided = token.as_bytes();

    if expected.len() != provided.len() || !bool::from(expected.ct_eq(provided)) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AdminAuthError {
                error: "unauthorized".to_string(),
                message: "invalid admin token".to_string(),
            }),
        )
            .into_response();
    }

    next.run(request).await
}
