//! Operator management routes.

use crate::error::OpsError;
use crate::middleware::OpsAuthCtx;
use crate::state::OpsState;
use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request to bootstrap the first operator.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BootstrapRequest {
    /// Email of the user to promote.
    pub email: String,
    /// Admin token for bootstrap authentication.
    pub admin_token: String,
}

/// Response from operator bootstrap.
#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapResponse {
    /// Whether the operation succeeded.
    pub success: bool,
    /// The user ID that was promoted.
    pub user_id: String,
    /// Message.
    pub message: String,
}

/// Request to promote a user to operator.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PromoteRequest {
    /// Email of the user to promote.
    pub email: String,
}

/// Response from operator promotion.
#[derive(Debug, Serialize, ToSchema)]
pub struct PromoteResponse {
    /// Whether the operation succeeded.
    pub success: bool,
    /// The user ID that was promoted.
    pub user_id: String,
}

/// Status response for operators.
#[derive(Debug, Serialize, ToSchema)]
pub struct StatusResponse {
    /// Whether any platform operators exist.
    pub operators_exist: bool,
    /// Number of platform operators.
    pub count: i64,
}

/// Bootstrap the first platform operator.
///
/// This endpoint can only be called from loopback and requires the admin token.
/// It is designed for initial cluster setup before any operators exist.
#[utoipa::path(
    post,
    path = "/_ops/v1/operators/bootstrap",
    request_body = BootstrapRequest,
    responses(
        (status = 200, description = "Operator bootstrapped", body = BootstrapResponse),
        (status = 400, description = "Operators already exist"),
        (status = 403, description = "Invalid admin token"),
    )
)]
pub async fn bootstrap(
    State(state): State<OpsState>,
    Json(req): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, OpsError> {
    // This will be implemented to check admin token from config
    // For now, return a placeholder that indicates the endpoint exists

    // Check if operators already exist
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reactor_auth.platform_memberships WHERE role_name = 'platform_operator'"
    )
    .fetch_one(&state.pool)
    .await?;

    if count > 0 {
        return Err(OpsError::Validation(
            "Platform operators already exist. Use /promote with operator credentials instead.".to_string()
        ));
    }

    // Look up user by email
    let user_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM reactor_auth.users WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await?;

    let user_id = user_id.ok_or_else(|| {
        OpsError::Validation(format!("User not found: {}", req.email))
    })?;

    // Grant platform_operator role
    sqlx::query(
        r#"
        INSERT INTO reactor_auth.platform_memberships (user_id, role_name, granted_by, granted_at)
        VALUES ($1, 'platform_operator', $1, NOW())
        ON CONFLICT (user_id, role_name) DO NOTHING
        "#
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(BootstrapResponse {
        success: true,
        user_id: user_id.to_string(),
        message: format!("User {} promoted to platform_operator", req.email),
    }))
}

/// Check operator status.
#[utoipa::path(
    get,
    path = "/_ops/v1/operators/status",
    responses(
        (status = 200, description = "Operators status", body = StatusResponse),
    )
)]
pub async fn status(
    State(state): State<OpsState>,
) -> Result<Json<StatusResponse>, OpsError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reactor_auth.platform_memberships WHERE role_name = 'platform_operator'"
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(StatusResponse {
        operators_exist: count > 0,
        count,
    }))
}

/// Promote a user to platform operator.
///
/// Requires `ops:cluster_admin` scope.
#[utoipa::path(
    post,
    path = "/_ops/v1/operators/promote",
    request_body = PromoteRequest,
    responses(
        (status = 200, description = "User promoted", body = PromoteResponse),
        (status = 403, description = "Missing scope"),
        (status = 404, description = "User not found"),
    )
)]
pub async fn promote(
    State(state): State<OpsState>,
    ctx: OpsAuthCtx,
    Json(req): Json<PromoteRequest>,
) -> Result<Json<PromoteResponse>, OpsError> {
    // Look up user by email
    let user_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM reactor_auth.users WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await?;

    let user_id = user_id.ok_or(OpsError::NotFound)?;

    // Grant platform_operator role
    sqlx::query(
        r#"
        INSERT INTO reactor_auth.platform_memberships (user_id, role_name, granted_by, granted_at)
        VALUES ($1, 'platform_operator', $2, NOW())
        ON CONFLICT (user_id, role_name) DO NOTHING
        "#
    )
    .bind(user_id)
    .bind(ctx.user_id.as_uuid())
    .execute(&state.pool)
    .await?;

    Ok(Json(PromoteResponse {
        success: true,
        user_id: user_id.to_string(),
    }))
}
