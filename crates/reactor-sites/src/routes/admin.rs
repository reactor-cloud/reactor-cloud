//! Site admin CRUD routes.

use crate::audit::{event_types, write_audit};
use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use crate::store::{NewSite, PgSitesStore, Site, SitesStore};
use crate::Framework;
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Create site request.
#[derive(Debug, Deserialize)]
pub struct CreateSiteRequest {
    /// Site name.
    pub name: String,
    /// Framework type.
    pub framework: Framework,
}

/// Site response.
#[derive(Debug, Serialize)]
pub struct SiteResponse {
    /// Site data.
    #[serde(flatten)]
    pub site: Site,
}

/// Create a new site.
pub async fn create_site(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Json(req): Json<CreateSiteRequest>,
) -> Result<(StatusCode, Json<SiteResponse>), SitesError> {
    if !ctx.has_permission("sites:create") {
        return Err(SitesError::PermissionDenied("sites:create".to_string()));
    }

    if !crate::SITE_NAME_REGEX.is_match(&req.name) {
        return Err(SitesError::InvalidSiteName(req.name));
    }

    let store = PgSitesStore::new(state.pool.clone());

    if store.get_site(&ctx.active_org(), &req.name).await?.is_some() {
        return Err(SitesError::SiteAlreadyExists(req.name));
    }

    let new_site = NewSite {
        org_id: ctx.active_org(),
        name: req.name.clone(),
        framework: req.framework,
    };

    let site = store.create_site(&new_site).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::SITE_CREATE,
        Some(site.id),
        None,
        None,
        serde_json::json!({ "name": site.name, "framework": site.framework }),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(SiteResponse { site })))
}

/// List sites for the current org.
pub async fn list_sites(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
) -> Result<Json<Vec<Site>>, SitesError> {
    let store = PgSitesStore::new(state.pool.clone());
    let sites = store.list_sites(&ctx.active_org()).await?;
    Ok(Json(sites))
}

/// Get a site by name.
pub async fn get_site(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
) -> Result<Json<SiteResponse>, SitesError> {
    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name))?;

    Ok(Json(SiteResponse { site }))
}

/// Delete a site.
pub async fn delete_site(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
) -> Result<StatusCode, SitesError> {
    let perm = format!("sites:{}:admin", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    store.delete_site(&site.id).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::SITE_DELETE,
        Some(site.id),
        None,
        None,
        serde_json::json!({ "name": site.name }),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
