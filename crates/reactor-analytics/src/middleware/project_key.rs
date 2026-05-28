//! Project key middleware for anonymous ingestion.
//!
//! Extracts X-Reactor-Project-Key header, validates against the store,
//! and creates an anonymous AnalyticsCtx.

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::middleware::auth::extract_or_generate_request_id;
use crate::state::{AnalyticsCtx, AnalyticsState, ConsentState};
use crate::store::AnalyticsStore;

/// Error response for key validation failures.
#[derive(Debug, Serialize)]
struct KeyErrorResponse {
    error: String,
    code: String,
}

impl KeyErrorResponse {
    fn unauthorized(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(Self {
                error: message.into(),
                code: "analytics.project_key.invalid".to_string(),
            }),
        )
    }

    fn forbidden(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: message.into(),
                code: "analytics.cors.forbidden".to_string(),
            }),
        )
    }
}

/// Hash a project key using SHA-256 for lookup.
fn hash_project_key(key: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.finalize().to_vec()
}

/// Project key middleware for anonymous ingestion.
///
/// Extracts X-Reactor-Project-Key header, validates the key,
/// checks CORS origin if configured, and creates an anonymous context.
pub async fn project_key_middleware<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    headers: HeaderMap,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Generate request ID
    let request_id = extract_or_generate_request_id(&request);

    // Extract project key
    let key = match extract_project_key(&headers) {
        Some(k) => k,
        None => {
            return KeyErrorResponse::unauthorized("missing X-Reactor-Project-Key header")
                .into_response()
        }
    };

    // Hash the key and look it up
    let key_hash = hash_project_key(&key);
    let key_record = match state.store.lookup_project_key(&key_hash).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            tracing::warn!("invalid project key attempted");
            return KeyErrorResponse::unauthorized("invalid or revoked project key")
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to lookup project key");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(KeyErrorResponse {
                    error: "internal error".to_string(),
                    code: "analytics.internal_error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Check CORS origin if configured
    if let Some(ref allowed_origins) = key_record.allowed_origins {
        if let Some(origin) = headers.get(header::ORIGIN) {
            if let Ok(origin_str) = origin.to_str() {
                if !is_origin_allowed(origin_str, allowed_origins) {
                    tracing::warn!(origin = %origin_str, "CORS origin not allowed");
                    return KeyErrorResponse::forbidden("origin not allowed").into_response();
                }
            }
        }
    }

    // Handle CORS preflight
    if request.method() == Method::OPTIONS {
        return build_cors_response(&key_record.allowed_origins).into_response();
    }

    // Check for DNT/Sec-GPC headers
    let dnt = is_dnt_set(&headers);

    // Clone allowed_origins for CORS headers (needed after move to ctx)
    let allowed_origins_for_cors = key_record.allowed_origins.clone();

    // Create anonymous context
    let ctx = AnalyticsCtx::anonymous(
        key_record.project_id,
        key_record.org_id.into(),
        key_record.id,
        key_record.sampling_rate,
        key_record.allowed_origins,
        request_id,
    )
    .with_dnt(dnt)
    .with_consent(if dnt {
        ConsentState::Denied
    } else {
        ConsentState::Unknown
    });

    // Insert context into extensions
    request.extensions_mut().insert(ctx);

    // Add CORS headers to response
    let response = next.run(request).await;
    add_cors_headers(response, &allowed_origins_for_cors)
}

/// Extract X-Reactor-Project-Key header.
fn extract_project_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-reactor-project-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Check if origin is in the allowed list.
fn is_origin_allowed(origin: &str, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }

    for pattern in allowed {
        if pattern == "*" {
            return true;
        }

        // Exact match
        if pattern == origin {
            return true;
        }

        // Wildcard subdomain match (e.g., "*.example.com")
        if pattern.starts_with("*.") {
            let suffix = &pattern[1..]; // ".example.com"
            if origin.ends_with(suffix) {
                return true;
            }
        }
    }

    false
}

/// Check if DNT or Sec-GPC header is set.
fn is_dnt_set(headers: &HeaderMap) -> bool {
    if let Some(dnt) = headers.get("dnt") {
        if let Ok(v) = dnt.to_str() {
            if v == "1" {
                return true;
            }
        }
    }

    if let Some(gpc) = headers.get("sec-gpc") {
        if let Ok(v) = gpc.to_str() {
            if v == "1" {
                return true;
            }
        }
    }

    false
}

/// Build a CORS preflight response.
fn build_cors_response(allowed_origins: &Option<Vec<String>>) -> Response {
    let origin = if let Some(origins) = allowed_origins {
        if origins.contains(&"*".to_string()) {
            "*"
        } else {
            origins.first().map(|s| s.as_str()).unwrap_or("*")
        }
    } else {
        "*"
    };

    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
        .header(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            "POST, OPTIONS",
        )
        .header(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, X-Reactor-Project-Key",
        )
        .header(header::ACCESS_CONTROL_MAX_AGE, "86400")
        .body(Body::empty())
        .unwrap()
}

/// Add CORS headers to response.
fn add_cors_headers(response: Response, allowed_origins: &Option<Vec<String>>) -> Response {
    let (mut parts, body) = response.into_parts();

    let origin = if let Some(origins) = allowed_origins {
        if origins.contains(&"*".to_string()) {
            "*".to_string()
        } else {
            origins.first().cloned().unwrap_or_else(|| "*".to_string())
        }
    } else {
        "*".to_string()
    };

    parts.headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        origin.parse().unwrap(),
    );

    Response::from_parts(parts, body)
}
