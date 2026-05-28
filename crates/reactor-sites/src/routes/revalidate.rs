//! ISR revalidation routes.

use crate::audit::{event_types, write_audit};
use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use crate::store::{PgSitesStore, SitesStore};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Revalidate request.
#[derive(Debug, Deserialize)]
pub struct RevalidateRequest {
    /// Paths to revalidate.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Tags to revalidate.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Revalidate response.
#[derive(Debug, Serialize)]
pub struct RevalidateResponse {
    /// Number of entries invalidated.
    pub invalidated_count: u32,
}

/// Revalidate ISR cache entries.
pub async fn revalidate(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Json(req): Json<RevalidateRequest>,
) -> Result<Json<RevalidateResponse>, SitesError> {
    let perm = format!("sites:{}:admin", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let mut total_invalidated = 0u32;

    for path in &req.paths {
        let count = store.invalidate_isr(&site.id, path).await?;
        total_invalidated += count;
    }

    for tag in &req.tags {
        let count = store.invalidate_isr(&site.id, tag).await?;
        total_invalidated += count;
    }

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::ISR_INVALIDATE,
        Some(site.id),
        None,
        None,
        serde_json::json!({
            "paths": req.paths,
            "tags": req.tags,
            "invalidated_count": total_invalidated,
        }),
    )
    .await?;

    Ok(Json(RevalidateResponse {
        invalidated_count: total_invalidated,
    }))
}
