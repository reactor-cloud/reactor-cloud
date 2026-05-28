//! Per-site policy routes.

use crate::audit::{event_types, write_audit};
use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use crate::store::{NewSitePolicy, PgSitesStore, SitePolicy, SitesStore};
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Create policy request.
#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    /// Policy name.
    pub name: String,
    /// Policy text (using expression).
    pub using: String,
}

/// List policies for a site.
pub async fn list_policies(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
) -> Result<Json<Vec<SitePolicy>>, SitesError> {
    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name))?;

    let policies = store.get_site_policies(&site.id).await?;
    Ok(Json(policies))
}

/// Create a policy.
pub async fn create_policy(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Json(req): Json<CreatePolicyRequest>,
) -> Result<(StatusCode, Json<SitePolicy>), SitesError> {
    let perm = format!("sites:{}:admin", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let mut hasher = Sha256::new();
    hasher.update(req.using.as_bytes());
    let sha256 = hasher.finalize().to_vec();

    let new_policy = NewSitePolicy {
        site_id: site.id,
        name: req.name.clone(),
        using_expr_json: None,
        raw_text: req.using.clone(),
        sha256,
    };

    let policy = store.upsert_policy(&new_policy).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::POLICY_CREATE,
        Some(site.id),
        None,
        None,
        serde_json::json!({ "policy_name": policy.name }),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(policy)))
}

/// Delete a policy.
pub async fn delete_policy(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path((name, policy_name)): Path<(String, String)>,
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

    let policies = store.get_site_policies(&site.id).await?;
    let policy = policies
        .iter()
        .find(|p| p.name == policy_name)
        .ok_or_else(|| SitesError::Internal(format!("policy not found: {}", policy_name)))?;

    store.delete_policy(&policy.id).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::POLICY_DELETE,
        Some(site.id),
        None,
        None,
        serde_json::json!({ "policy_name": policy_name }),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
