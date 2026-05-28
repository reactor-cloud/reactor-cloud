//! Logout endpoint.

use crate::error::AppError;
use crate::extract::AuthBearer;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

/// POST /auth/v1/logout
///
/// Revokes the current session. Requires Bearer authentication.
#[utoipa::path(
    post,
    path = "/auth/v1/logout",
    tag = "auth",
    security(("bearer" = [])),
    responses(
        (status = 204, description = "Session revoked"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn logout<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    auth: AuthBearer<S>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(session_id) = auth.claims.session_id {
        service.logout(&session_id).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}
