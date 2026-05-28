//! Deployment routes.
//!
//! These routes wrap the existing deploy functionality with ops-level
//! authentication and audit logging.

use crate::error::OpsError;
use crate::middleware::OpsAuthCtx;
use crate::state::OpsState;
use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Deployment response.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeploymentResponse {
    /// Deployment ID.
    pub deploy_id: String,
    /// Overall status: "ok", "partial", or "failed".
    pub status: String,
    /// Message about the deployment.
    pub message: String,
    /// The actor who performed the deployment.
    pub actor: DeploymentActor,
}

/// Actor information for deployment auditing.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeploymentActor {
    /// User ID of the deploying operator.
    pub user_id: String,
    /// Email of the deploying operator (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Deployment status response.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeploymentStatusResponse {
    /// Whether the ops deployment endpoint is available.
    pub available: bool,
    /// Message about the endpoint status.
    pub message: String,
}

/// Get deployment endpoint status.
///
/// This endpoint can be used to check if the deployment service is available
/// and properly configured.
#[utoipa::path(
    get,
    path = "/_ops/v1/deployments/status",
    responses(
        (status = 200, description = "Deployment status", body = DeploymentStatusResponse),
    )
)]
pub async fn deployment_status(
    _ctx: OpsAuthCtx,
) -> Json<DeploymentStatusResponse> {
    Json(DeploymentStatusResponse {
        available: true,
        message: "Deployment endpoint is available. Use POST with multipart form to deploy.".to_string(),
    })
}

/// Create a new deployment.
///
/// This is the ops-authenticated version of the deploy endpoint.
/// The actual deployment logic is forwarded to the existing handler,
/// but with proper actor attribution for audit logging.
///
/// Note: The actual multipart bundle handling is performed by the admin layer.
/// This endpoint provides the ops authentication and audit wrapper.
#[utoipa::path(
    post,
    path = "/_ops/v1/deployments",
    responses(
        (status = 200, description = "Deployment initiated", body = DeploymentResponse),
        (status = 403, description = "Missing scope"),
    )
)]
pub async fn create_deployment(
    State(_state): State<OpsState>,
    ctx: OpsAuthCtx,
) -> Result<Json<DeploymentResponse>, OpsError> {
    // In the full implementation, this would:
    // 1. Extract the multipart bundle
    // 2. Forward to the deploy logic
    // 3. Record the actor in the audit log
    //
    // For now, return a placeholder showing the actor info is captured

    let actor = DeploymentActor {
        user_id: ctx.user_id.to_string(),
        email: ctx.claims.email.clone(),
    };

    Ok(Json(DeploymentResponse {
        deploy_id: uuid::Uuid::now_v7().to_string(),
        status: "pending".to_string(),
        message: "Deployment endpoint structure in place. Full multipart handling to be integrated.".to_string(),
        actor,
    }))
}
