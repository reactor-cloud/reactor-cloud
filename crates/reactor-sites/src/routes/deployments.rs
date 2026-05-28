//! Deployment routes.

use crate::audit::{event_types, write_audit};
use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use crate::store::{NewDeployment, PgSitesStore, SiteDeployment, SitesStore};
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Create deployment request.
#[derive(Debug, Deserialize)]
pub struct CreateDeploymentRequest {
    /// Bundle manifest.
    pub manifest: serde_json::Value,
}

/// Deployment response.
#[derive(Debug, Serialize)]
pub struct DeploymentResponse {
    /// Deployment data.
    #[serde(flatten)]
    pub deployment: SiteDeployment,
}

/// Promote deployment request.
#[derive(Debug, Deserialize)]
pub struct PromoteRequest {
    /// Deployment ID to promote.
    pub deployment_id: Uuid,
}

/// Rollback request.
#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    /// Target deployment ID (optional, defaults to previous).
    pub to_deployment_id: Option<Uuid>,
}

/// Create a new deployment.
pub async fn create_deployment(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Json(req): Json<CreateDeploymentRequest>,
) -> Result<(StatusCode, Json<DeploymentResponse>), SitesError> {
    let perm = format!("sites:{}:deploy", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let new_deployment = NewDeployment {
        site_id: site.id,
        manifest_json: req.manifest,
        deployed_by_user_id: ctx.user_id().map(|id| id.into()),
    };

    let deployment = store.create_deployment(&new_deployment).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::DEPLOYMENT_CREATE,
        Some(site.id),
        Some(deployment.id),
        None,
        serde_json::json!({ "version": deployment.version }),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(DeploymentResponse { deployment })))
}

/// List deployments for a site.
pub async fn list_deployments(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
) -> Result<Json<Vec<SiteDeployment>>, SitesError> {
    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name))?;

    let deployments = store.list_deployments(&site.id, 20).await?;
    Ok(Json(deployments))
}

/// Get a deployment by ID.
pub async fn get_deployment(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path((name, deployment_id)): Path<(String, Uuid)>,
) -> Result<Json<DeploymentResponse>, SitesError> {
    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name))?;

    let deployment = store
        .get_deployment(&deployment_id)
        .await?
        .ok_or_else(|| SitesError::DeploymentNotFound(deployment_id.to_string()))?;

    if deployment.site_id != site.id {
        return Err(SitesError::DeploymentNotFound(deployment_id.to_string()));
    }

    Ok(Json(DeploymentResponse { deployment }))
}

/// Promote a deployment.
pub async fn promote_deployment(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Json(req): Json<PromoteRequest>,
) -> Result<Json<serde_json::Value>, SitesError> {
    let perm = format!("sites:{}:deploy", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let deployment = store
        .get_deployment(&req.deployment_id)
        .await?
        .ok_or_else(|| SitesError::DeploymentNotFound(req.deployment_id.to_string()))?;

    if deployment.site_id != site.id {
        return Err(SitesError::DeploymentNotFound(req.deployment_id.to_string()));
    }

    if deployment.status != "ready" {
        return Err(SitesError::DeploymentNotReady(req.deployment_id.to_string()));
    }

    store.promote_deployment(&req.deployment_id).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::DEPLOYMENT_PROMOTE,
        Some(site.id),
        Some(deployment.id),
        None,
        serde_json::json!({ "version": deployment.version }),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "site": name,
        "deployment_id": deployment.id,
        "version": deployment.version,
    })))
}

/// Rollback to a previous deployment.
pub async fn rollback_deployment(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Json(req): Json<RollbackRequest>,
) -> Result<Json<serde_json::Value>, SitesError> {
    let perm = format!("sites:{}:admin", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let target_id = if let Some(id) = req.to_deployment_id {
        id
    } else {
        let deployments = store.list_deployments(&site.id, 2).await?;
        if deployments.len() < 2 {
            return Err(SitesError::DeploymentNotFound(
                "no previous deployment".to_string(),
            ));
        }
        deployments[1].id
    };

    let deployment = store
        .get_deployment(&target_id)
        .await?
        .ok_or_else(|| SitesError::DeploymentNotFound(target_id.to_string()))?;

    if deployment.site_id != site.id {
        return Err(SitesError::DeploymentNotFound(target_id.to_string()));
    }

    store.promote_deployment(&target_id).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::DEPLOYMENT_ROLLBACK,
        Some(site.id),
        Some(deployment.id),
        None,
        serde_json::json!({ "version": deployment.version }),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "site": name,
        "deployment_id": deployment.id,
        "version": deployment.version,
    })))
}
