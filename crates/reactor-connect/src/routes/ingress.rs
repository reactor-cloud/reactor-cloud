//! Webhook ingress endpoint (anonymous).

use crate::error::ConnectError;
use crate::state::ConnectState;
use crate::store::ConnectStore;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;

/// Webhook ingress response.
#[derive(Debug, Serialize)]
pub struct IngressResponse {
    /// Event ID for tracking.
    pub event_id: String,
}

/// POST /connect/v1/ingress/:receiver_token
pub async fn webhook_ingress<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Path(receiver_token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<IngressResponse>, ConnectError> {
    // Look up receiver by token
    let receiver = state
        .store
        .get_receiver_by_token(&receiver_token)
        .await?
        .ok_or(ConnectError::ReceiverNotFound)?;

    // Check if receiver is enabled
    if !receiver.enabled {
        return Err(ConnectError::ReceiverDisabled);
    }

    // Get instance and descriptor for signature verification
    let instance = state
        .store
        .get_instance_by_id(&receiver.instance_id)
        .await?
        .ok_or(ConnectError::InstanceNotFound(receiver.instance_id.to_string()))?;

    let descriptor = state.runtime.descriptor(&instance.type_id).await?;

    // Find webhook descriptor
    let webhook = descriptor
        .webhooks
        .iter()
        .find(|w| w.name == receiver.webhook_name)
        .ok_or(ConnectError::ReceiverNotFound)?;

    // Verify signature
    verify_signature(&webhook.verification, &headers, &body, &instance)?;

    // Check for replay
    let event_id = extract_event_id(&headers, &body)?;
    let replay_key = format!(
        "connect:replay:{}:{}",
        receiver.id, event_id
    );
    
    if let Ok(Some(_)) = state.cache.get(&replay_key).await {
        return Err(ConnectError::WebhookReplayDetected);
    }

    // Store replay protection key
    let _ = state
        .cache
        .set(&replay_key, &[], Some(std::time::Duration::from_secs(webhook.replay_window_seconds)))
        .await;

    // Dispatch based on configuration
    // TODO: Implement dispatch to job/stream/action/function
    tracing::info!(
        receiver_id = %receiver.id,
        event_id = %event_id,
        dispatch_kind = %receiver.dispatch_kind,
        "Received webhook event"
    );

    Ok(Json(IngressResponse { event_id }))
}

fn verify_signature(
    verification: &crate::descriptor::VerificationKind,
    headers: &HeaderMap,
    body: &Bytes,
    instance: &crate::store::Instance,
) -> Result<(), ConnectError> {
    use crate::descriptor::VerificationKind;
    
    match verification {
        VerificationKind::HmacSha256 { header, secret_field } => {
            let signature = headers
                .get(header.as_str())
                .and_then(|v| v.to_str().ok())
                .ok_or(ConnectError::WebhookSignatureInvalid)?;

            // TODO: Get secret from credentials and verify HMAC
            // For now, just check the header exists
            if signature.is_empty() {
                return Err(ConnectError::WebhookSignatureInvalid);
            }
            
            tracing::debug!(
                header = %header,
                secret_field = %secret_field,
                "HMAC-SHA256 verification (placeholder)"
            );
            Ok(())
        }
        VerificationKind::Ed25519 { header, key_id_header } => {
            let signature = headers
                .get(header.as_str())
                .and_then(|v| v.to_str().ok())
                .ok_or(ConnectError::WebhookSignatureInvalid)?;

            // TODO: Verify Ed25519 signature
            if signature.is_empty() {
                return Err(ConnectError::WebhookSignatureInvalid);
            }
            
            tracing::debug!(
                header = %header,
                key_id_header = ?key_id_header,
                "Ed25519 verification (placeholder)"
            );
            Ok(())
        }
        VerificationKind::Custom { docs_url } => {
            // Custom verification is handled by the connector
            tracing::debug!(docs_url = %docs_url, "Custom verification (skipped)");
            Ok(())
        }
    }
}

fn extract_event_id(headers: &HeaderMap, body: &Bytes) -> Result<String, ConnectError> {
    // Try common event ID headers
    for header in ["X-Request-ID", "X-Event-ID", "X-Delivery-ID", "X-GitHub-Delivery"] {
        if let Some(value) = headers.get(header).and_then(|v| v.to_str().ok()) {
            return Ok(value.to_string());
        }
    }

    // Try to parse body as JSON and extract an ID
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
        for field in ["id", "event_id", "eventId", "delivery_id"] {
            if let Some(id) = json.get(field).and_then(|v| v.as_str()) {
                return Ok(id.to_string());
            }
        }
    }

    // Generate a random ID as fallback
    Ok(uuid::Uuid::now_v7().to_string())
}
