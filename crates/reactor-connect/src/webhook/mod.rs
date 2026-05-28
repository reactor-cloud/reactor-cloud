//! Webhook handling.

use crate::descriptor::VerificationKind;
use crate::error::ConnectError;

/// Verify a webhook signature.
pub fn verify_signature(
    verification: &VerificationKind,
    headers: &axum::http::HeaderMap,
    body: &[u8],
    secret: &[u8],
) -> Result<(), ConnectError> {
    match verification {
        VerificationKind::HmacSha256 { header, .. } => {
            verify_hmac_sha256(headers, body, secret, header)
        }
        VerificationKind::Ed25519 { header, .. } => {
            verify_ed25519(headers, body, header)
        }
        VerificationKind::Custom { .. } => {
            // Custom verification is handled by the connector
            Ok(())
        }
    }
}

fn verify_hmac_sha256(
    headers: &axum::http::HeaderMap,
    body: &[u8],
    secret: &[u8],
    header_name: &str,
) -> Result<(), ConnectError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let signature = headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .ok_or(ConnectError::WebhookSignatureInvalid)?;

    // Handle different signature formats
    let signature = signature
        .strip_prefix("sha256=")
        .or_else(|| signature.strip_prefix("v0="))
        .unwrap_or(signature);

    let expected_bytes = hex::decode(signature)
        .or_else(|_| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature)
        })
        .map_err(|_| ConnectError::WebhookSignatureInvalid)?;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .map_err(|_| ConnectError::Internal("Invalid HMAC key".to_string()))?;
    mac.update(body);
    
    mac.verify_slice(&expected_bytes)
        .map_err(|_| ConnectError::WebhookSignatureInvalid)?;

    Ok(())
}

fn verify_ed25519(
    _headers: &axum::http::HeaderMap,
    _body: &[u8],
    _header_name: &str,
) -> Result<(), ConnectError> {
    // TODO: Implement Ed25519 verification (used by GitHub)
    // Requires ed25519-dalek crate
    Ok(())
}
