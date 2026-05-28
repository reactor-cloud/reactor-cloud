//! Action invocation endpoint.

use crate::error::ConnectError;
use crate::service::ConnectService;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;
use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};

/// Invoke action request.
#[derive(Debug, Deserialize)]
pub struct InvokeActionRequest {
    /// Action input.
    pub input: serde_json::Value,
}

/// Invoke action response.
#[derive(Debug, Serialize)]
pub struct InvokeActionResponse {
    /// Action output.
    pub output: serde_json::Value,
    /// Invocation ID.
    pub invocation_id: String,
}

/// POST /connect/v1/instances/:name/actions/:action/invoke
pub async fn invoke_action<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Extension(ctx): Extension<ConnectCtx>,
    Path((name, action)): Path<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<InvokeActionRequest>,
) -> Result<Json<InvokeActionResponse>, ConnectError> {
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let service = ConnectService::new(&state, &ctx);
    
    let output = service
        .invoke_action(&name, &action, req.input, idempotency_key, false)
        .await?;

    Ok(Json(InvokeActionResponse {
        output,
        invocation_id: uuid::Uuid::now_v7().to_string(),
    }))
}
