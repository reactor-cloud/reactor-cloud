//! Authentication middleware for reactor-ai.
//!
//! Extracts Bearer token and resolves auth context, inserts AiCtx into request extensions.

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use reactor_core::auth::{AuthCtx, OrgRef};
use serde::Serialize;
use uuid::Uuid;

use crate::state::AiState;

/// Request context for AI operations.
#[derive(Debug, Clone)]
pub struct AiCtx {
    /// Authenticated user context (None for anonymous/API key only).
    pub auth: Option<AuthCtx>,
    /// Request ID for tracing.
    pub request_id: Uuid,
}

impl AiCtx {
    /// Create an authenticated AI context.
    pub fn authenticated(auth: AuthCtx, request_id: Uuid) -> Self {
        Self {
            auth: Some(auth),
            request_id,
        }
    }

    /// Create an anonymous AI context.
    pub fn anonymous(request_id: Uuid) -> Self {
        Self {
            auth: None,
            request_id,
        }
    }

    /// Get the user ID from the auth context if available.
    pub fn user_id(&self) -> Option<String> {
        self.auth.as_ref()?.claims.user_id().map(|id| id.to_string())
    }
}

/// Error response for auth failures.
#[derive(Debug, Serialize)]
struct AuthError {
    error: String,
}

/// Authentication middleware that resolves auth context.
///
/// Unlike storage, AI requests always require some form of authentication
/// (JWT or API key), but the metering user_id is only available for JWT auth.
pub async fn auth_middleware(
    State(state): State<AiState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Get request ID from headers or generate one
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(Uuid::now_v7);

    // Extract Bearer token
    let token = match request.headers().get(header::AUTHORIZATION) {
        Some(auth_header) => match auth_header.to_str() {
            Ok(s) if s.starts_with("Bearer ") => s.trim_start_matches("Bearer ").to_string(),
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(AuthError {
                        error: "Invalid authorization header".to_string(),
                    }),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "Authorization required".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Extract org header if present and parse it
    let org_ref: Option<OrgRef> = request
        .headers()
        .get("x-reactor-org")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    // Resolve full auth context with auth client
    match state.auth.resolve_ctx(&token, org_ref.as_ref()).await {
        Ok(auth_ctx) => {
            let ai_ctx = AiCtx::authenticated(auth_ctx, request_id);
            request.extensions_mut().insert(ai_ctx);
            next.run(request).await
        }
        Err(e) => {
            tracing::warn!(error = %e, "Auth verification failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: format!("Authentication failed: {}", e),
                }),
            )
                .into_response()
        }
    }
}
