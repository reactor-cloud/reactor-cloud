//! Health and status routes.

use crate::error::OpsError;
use crate::state::OpsState;
use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

/// Doctor response showing system health.
#[derive(Debug, Serialize, ToSchema)]
pub struct DoctorResponse {
    /// Overall status.
    pub status: String,
    /// Database connectivity.
    pub database: ComponentStatus,
    /// Auth service.
    pub auth: ComponentStatus,
}

/// Status of a component.
#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentStatus {
    /// Whether the component is healthy.
    pub healthy: bool,
    /// Optional message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Latency in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

/// Version response.
#[derive(Debug, Serialize, ToSchema)]
pub struct VersionResponse {
    /// Version string.
    pub version: String,
    /// Git commit hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    /// Build timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_time: Option<String>,
}

/// Check system health.
#[utoipa::path(
    get,
    path = "/_ops/v1/doctor",
    responses(
        (status = 200, description = "Health status", body = DoctorResponse),
    )
)]
pub async fn doctor(
    State(state): State<OpsState>,
) -> Result<Json<DoctorResponse>, OpsError> {
    // Check database
    let db_start = std::time::Instant::now();
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let db_latency = db_start.elapsed().as_millis() as u64;

    let database = ComponentStatus {
        healthy: db_healthy,
        message: if db_healthy { None } else { Some("Database connection failed".to_string()) },
        latency_ms: Some(db_latency),
    };

    // Auth is assumed healthy if we got this far (middleware validated token)
    let auth = ComponentStatus {
        healthy: true,
        message: None,
        latency_ms: None,
    };

    let overall = if db_healthy { "healthy" } else { "degraded" };

    Ok(Json(DoctorResponse {
        status: overall.to_string(),
        database,
        auth,
    }))
}

/// Get version information.
#[utoipa::path(
    get,
    path = "/_ops/v1/version",
    responses(
        (status = 200, description = "Version info", body = VersionResponse),
    )
)]
pub async fn version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit: option_env!("GIT_COMMIT").map(String::from),
        build_time: option_env!("BUILD_TIME").map(String::from),
    })
}
