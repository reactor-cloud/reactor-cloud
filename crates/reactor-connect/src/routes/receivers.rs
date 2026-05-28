//! Receiver management endpoints.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::{ConnectStore, NewReceiver, Receiver};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono;
use serde::{Deserialize, Serialize};

/// Create receiver request.
#[derive(Debug, Deserialize)]
pub struct CreateReceiverRequest {
    /// Webhook name from descriptor.
    pub webhook: String,
    /// Dispatch configuration.
    pub dispatch: DispatchConfig,
}

/// Dispatch configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchConfig {
    /// Dispatch to a job.
    Job { name: String },
    /// Dispatch to a stream connection.
    Stream { connection: String },
    /// Dispatch to another action.
    Action { instance: String, action: String },
    /// Dispatch to a function.
    Function { name: String },
}

impl DispatchConfig {
    fn kind(&self) -> &'static str {
        match self {
            DispatchConfig::Job { .. } => "job",
            DispatchConfig::Stream { .. } => "stream",
            DispatchConfig::Action { .. } => "action",
            DispatchConfig::Function { .. } => "function",
        }
    }
}

/// Create receiver response.
#[derive(Debug, Serialize)]
pub struct CreateReceiverResponse {
    /// Created receiver.
    pub receiver: Receiver,
    /// Ingress URL.
    pub ingress_url: String,
}

/// POST /connect/v1/instances/:name/receivers
pub async fn create<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
    Json(req): Json<CreateReceiverRequest>,
) -> Result<Json<CreateReceiverResponse>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    // Validate webhook exists in descriptor
    let descriptor = state.runtime.descriptor(&instance.type_id).await?;
    if !descriptor.webhooks.iter().any(|w| w.name == req.webhook) {
        return Err(ConnectError::InvalidInput(format!(
            "Webhook '{}' not found in connector descriptor",
            req.webhook
        )));
    }

    let receiver = state
        .store
        .create_receiver(
            ctx.active_org(),
            &NewReceiver {
                instance_id: instance.id,
                webhook_name: req.webhook,
                dispatch_kind: req.dispatch.kind().to_string(),
                dispatch_config_json: serde_json::to_value(&req.dispatch)?,
            },
        )
        .await?;

    // Build ingress URL
    let ingress_url = format!(
        "{}/connect/v1/ingress/{}",
        state.config.data_url, // Use data_url as base for now
        receiver.token
    );

    Ok(Json(CreateReceiverResponse {
        receiver,
        ingress_url,
    }))
}

/// GET /connect/v1/instances/:name/receivers
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path(name): Path<String>,
) -> Result<Json<Vec<Receiver>>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let receivers = state.store.list_receivers(&instance.id).await?;
    Ok(Json(receivers))
}

/// GET /connect/v1/instances/:name/receivers/:receiver_id
pub async fn show<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, receiver_id)): Path<(String, String)>,
) -> Result<Json<Receiver>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let receivers = state.store.list_receivers(&instance.id).await?;
    let receiver = receivers
        .into_iter()
        .find(|r| r.id.to_string() == receiver_id)
        .ok_or(ConnectError::ReceiverNotFound)?;

    Ok(Json(receiver))
}

/// DELETE /connect/v1/instances/:name/receivers/:receiver_id
pub async fn delete<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, receiver_id)): Path<(String, String)>,
) -> Result<(), ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let receivers = state.store.list_receivers(&instance.id).await?;
    let receiver = receivers
        .into_iter()
        .find(|r| r.id.to_string() == receiver_id)
        .ok_or(ConnectError::ReceiverNotFound)?;

    state.store.delete_receiver(&receiver.id).await?;
    Ok(())
}

/// Rotate token request.
#[derive(Debug, Deserialize)]
pub struct RotateTokenRequest {
    /// Grace period in seconds for old token.
    #[serde(default = "default_grace_seconds")]
    pub grace_seconds: u64,
}

fn default_grace_seconds() -> u64 {
    300 // 5 minutes
}

/// Rotate token response.
#[derive(Debug, Serialize)]
pub struct RotateTokenResponse {
    /// New token.
    pub new_token: String,
    /// When the old token expires.
    pub old_token_expires_at: chrono::DateTime<chrono::Utc>,
}

/// POST /connect/v1/instances/:name/receivers/:receiver_id/rotate
pub async fn rotate<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, receiver_id)): Path<(String, String)>,
    Json(req): Json<RotateTokenRequest>,
) -> Result<Json<RotateTokenResponse>, ConnectError> {
    let instance = state
        .store
        .get_instance(ctx.active_org(), &name)
        .await?
        .ok_or_else(|| ConnectError::InstanceNotFound(name))?;

    let receivers = state.store.list_receivers(&instance.id).await?;
    let _receiver = receivers
        .iter()
        .find(|r| r.id.to_string() == receiver_id)
        .ok_or(ConnectError::ReceiverNotFound)?;

    let receiver_uuid: uuid::Uuid = receiver_id
        .parse()
        .map_err(|_| ConnectError::InvalidInput("invalid receiver ID".into()))?;

    // Generate new token
    let new_token = uuid::Uuid::new_v4().to_string();
    let old_token_expires_at =
        chrono::Utc::now() + chrono::Duration::seconds(req.grace_seconds as i64);

    // TODO: Update receiver with new token and set old_token_expires_at
    // state.store.rotate_receiver_token(&receiver_uuid, &new_token, old_token_expires_at).await?;

    Ok(Json(RotateTokenResponse {
        new_token,
        old_token_expires_at,
    }))
}
