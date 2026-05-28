//! Consent management endpoints.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::audit::{event_types, write_audit};
use crate::error::AnalyticsError;
use crate::state::{AnalyticsCtx, AnalyticsState};
use crate::store::AnalyticsStore;

// ---------------------- Request/Response types ----------------------

/// Consent opt-out request.
#[derive(Debug, Deserialize)]
pub struct OptOutRequest {
    /// Anonymous ID to opt out.
    pub anonymous_id: String,
}

/// Consent opt-in request.
#[derive(Debug, Deserialize)]
pub struct OptInRequest {
    /// Anonymous ID to opt in.
    pub anonymous_id: String,
}

/// Consent status request.
#[derive(Debug, Deserialize)]
pub struct StatusRequest {
    /// Anonymous ID to check.
    pub anonymous_id: String,
}

/// Consent response.
#[derive(Debug, Serialize)]
pub struct ConsentResponse {
    pub success: bool,
}

/// Consent status response.
#[derive(Debug, Serialize)]
pub struct ConsentStatusResponse {
    /// Anonymous ID.
    pub anonymous_id: String,
    /// Whether the user is opted out.
    pub opted_out: bool,
}

// ---------------------- Handlers ----------------------

/// Opt out an anonymous user from tracking.
///
/// POST /analytics/v1/consent/opt-out
pub async fn opt_out<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<OptOutRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Create tombstone
    state
        .store
        .create_tombstone(ctx.project_id, &req.anonymous_id, "opt_out")
        .await?;

    // Write audit log
    write_audit(
        &state.store,
        &ctx,
        event_types::CONSENT_OPT_OUT,
        Some(ctx.project_id),
        serde_json::json!({
            "anonymous_id": req.anonymous_id
        }),
    )
    .await?;

    let response = ConsentResponse { success: true };
    Ok((StatusCode::OK, Json(response)))
}

/// Opt in an anonymous user to tracking.
///
/// POST /analytics/v1/consent/opt-in
pub async fn opt_in<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<OptInRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Remove tombstone
    state
        .store
        .remove_tombstone(ctx.project_id, &req.anonymous_id)
        .await?;

    // Write audit log
    write_audit(
        &state.store,
        &ctx,
        event_types::CONSENT_OPT_IN,
        Some(ctx.project_id),
        serde_json::json!({
            "anonymous_id": req.anonymous_id
        }),
    )
    .await?;

    let response = ConsentResponse { success: true };
    Ok((StatusCode::OK, Json(response)))
}

/// Get consent status for an anonymous user.
///
/// POST /analytics/v1/consent/status
pub async fn status<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<StatusRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    let opted_out = state
        .store
        .is_tombstoned(ctx.project_id, &req.anonymous_id)
        .await?;

    let response = ConsentStatusResponse {
        anonymous_id: req.anonymous_id,
        opted_out,
    };

    Ok((StatusCode::OK, Json(response)))
}
