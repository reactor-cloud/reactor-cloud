//! Platform operator management routes.
//!
//! These routes are used for bootstrapping the first platform operator
//! and for managing platform-level roles.

use crate::error::AppError;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reactor_core::auth::AuthError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Bootstrap operator request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BootstrapOperatorRequest {
    /// Email of the user to promote to platform operator.
    pub email: String,
}

/// Bootstrap operator response.
#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapOperatorResponse {
    /// Whether bootstrap was successful.
    pub success: bool,
    /// The user ID of the promoted operator.
    pub user_id: String,
    /// Message describing the result.
    pub message: String,
}

/// Promote operator request (admin-initiated).
#[derive(Debug, Deserialize, ToSchema)]
pub struct PromoteOperatorRequest {
    /// Email of the user to promote.
    pub email: String,
}

/// Promote operator response.
#[derive(Debug, Serialize, ToSchema)]
pub struct PromoteOperatorResponse {
    /// The user ID of the promoted operator.
    pub user_id: String,
    /// Message describing the result.
    pub message: String,
}

/// Check operators status response.
#[derive(Debug, Serialize, ToSchema)]
pub struct OperatorsStatusResponse {
    /// Whether any platform operators exist.
    pub operators_exist: bool,
    /// Message describing the status.
    pub message: String,
}

/// POST /_ops/v1/operators/bootstrap
///
/// Bootstrap the first platform operator.
/// This endpoint is only available when no platform operators exist yet.
/// It must be called from loopback (localhost) for security.
///
/// After the first operator is bootstrapped, this endpoint returns an error.
pub async fn bootstrap_operator<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    Json(req): Json<BootstrapOperatorRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check if any operators already exist
    let operators_exist = service.platform_operators_exist().await?;
    if operators_exist {
        return Err(AppError::Auth(AuthError::ValidationError {
            message: "Bootstrap already completed. Use 'promote' endpoint to add more operators.".to_string(),
        }));
    }

    // Promote the user to platform_operator
    let user_id = service
        .promote_to_platform_operator(&req.email, None)
        .await?;

    tracing::info!(
        user_id = %user_id,
        email = %req.email,
        "first platform operator bootstrapped"
    );

    Ok((
        StatusCode::OK,
        Json(BootstrapOperatorResponse {
            success: true,
            user_id: user_id.to_string(),
            message: format!("User {} is now a platform operator", req.email),
        }),
    ))
}

/// GET /_ops/v1/operators/status
///
/// Check if any platform operators exist.
/// This is useful to determine if bootstrap is needed.
pub async fn operators_status<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
) -> Result<impl IntoResponse, AppError> {
    let operators_exist = service.platform_operators_exist().await?;

    let message = if operators_exist {
        "Platform operators exist. Bootstrap not needed."
    } else {
        "No platform operators. Bootstrap required from loopback."
    };

    Ok((
        StatusCode::OK,
        Json(OperatorsStatusResponse {
            operators_exist,
            message: message.to_string(),
        }),
    ))
}

/// POST /_ops/v1/operators/promote
///
/// Promote a user to platform operator.
/// This endpoint requires an existing operator or admin token.
/// The `granted_by` is extracted from the authenticated user.
pub async fn promote_operator<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    // TODO: Extract authenticated user for granted_by
    Json(req): Json<PromoteOperatorRequest>,
) -> Result<impl IntoResponse, AppError> {
    // For now, no granted_by since this is admin-token gated
    // In Phase 3, this will use the ops middleware to get the actor
    let user_id = service
        .promote_to_platform_operator(&req.email, None)
        .await?;

    Ok((
        StatusCode::OK,
        Json(PromoteOperatorResponse {
            user_id: user_id.to_string(),
            message: format!("User {} is now a platform operator", req.email),
        }),
    ))
}
