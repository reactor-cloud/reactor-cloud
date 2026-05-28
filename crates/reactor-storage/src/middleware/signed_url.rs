//! Signed URL verification middleware.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::StorageState;

/// Query parameters for signed URLs.
#[derive(Debug, Deserialize)]
pub struct SignedUrlQuery {
    /// The HMAC signature.
    pub signature: Option<String>,
    /// Unix timestamp when the URL expires.
    pub expires: Option<u64>,
}

/// Error response for signed URL verification.
#[derive(Debug, Serialize)]
struct SignedUrlError {
    error: String,
    message: String,
}

impl SignedUrlError {
    fn expired() -> (StatusCode, Json<Self>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "url_expired".into(),
                message: "Signed URL has expired".into(),
            }),
        )
    }

    fn invalid() -> (StatusCode, Json<Self>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "invalid_signature".into(),
                message: "Invalid signature".into(),
            }),
        )
    }

    fn missing() -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self {
                error: "missing_signature".into(),
                message: "Signature and expires parameters required".into(),
            }),
        )
    }
}

/// Middleware to verify signed URLs.
///
/// If the request has signature and expires query parameters, this middleware
/// verifies them. If valid, the request proceeds. If invalid or expired, it
/// returns an error.
///
/// If no signature parameters are present, the request proceeds to the normal
/// auth middleware.
pub async fn verify_signed_url_middleware(
    State(state): State<StorageState>,
    Query(query): Query<SignedUrlQuery>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // If no signature params, skip verification (let normal auth handle it)
    let (signature, expires) = match (query.signature, query.expires) {
        (Some(sig), Some(exp)) => (sig, exp),
        (None, None) => return next.run(request).await,
        _ => return SignedUrlError::missing().into_response(),
    };

    // Check if URL has expired
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if now > expires {
        return SignedUrlError::expired().into_response();
    }

    // Get signing secret from config
    let secret = match &state.config.signing_secret {
        Some(s) => s,
        None => {
            tracing::error!("Signed URL received but no signing secret configured");
            return SignedUrlError::invalid().into_response();
        }
    };

    // Reconstruct the message to verify
    let method = request.method().as_str();
    let path = request.uri().path();

    // Extract org_id, bucket, key from path
    // Path format: /storage/v1/object/{bucket}/{key}
    // We need to construct: {method}:{org_id}/{bucket}/{key}:{expires}
    // But we don't have org_id in the path, so we use a simplified approach:
    // Message format: {method}:{path}:{expires}
    let message = format!("{}:{}:{}", method, path, expires);

    // Verify HMAC
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return SignedUrlError::invalid().into_response(),
    };
    mac.update(message.as_bytes());

    let expected = hex::encode(mac.finalize().into_bytes());

    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return SignedUrlError::invalid().into_response();
    }

    // Signature valid - proceed with request
    next.run(request).await
}

/// Constant-time equality comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"", b"hello"));
        assert!(constant_time_eq(b"", b""));
    }
}
