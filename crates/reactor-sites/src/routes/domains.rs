//! Domain management routes.

use crate::audit::{event_types, write_audit};
use crate::domain::{generate_verification_instructions, VerificationInstructions};
use crate::error::SitesError;
use crate::state::{SiteCtx, SitesState};
use crate::store::{Domain, DomainStatus, NewDomain, PgSitesStore, SitesStore};
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Create domain request.
#[derive(Debug, Deserialize)]
pub struct CreateDomainRequest {
    /// Domain host.
    pub host: String,
    /// Verification method (optional, defaults to "dns").
    #[serde(default = "default_verification_method")]
    pub verification_method: String,
}

fn default_verification_method() -> String {
    "dns".to_string()
}

/// Domain response.
#[derive(Debug, Serialize)]
pub struct DomainResponse {
    /// Domain data.
    #[serde(flatten)]
    pub domain: Domain,
    /// Verification instructions.
    pub verification_instructions: VerificationInstructions,
}

/// Create a custom domain.
pub async fn create_domain(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
    Json(req): Json<CreateDomainRequest>,
) -> Result<(StatusCode, Json<DomainResponse>), SitesError> {
    let perm = format!("sites:{}:admin", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let new_domain = NewDomain {
        site_id: site.id,
        host: req.host.clone(),
        verification_method: req.verification_method.clone(),
    };

    let domain = store.create_domain(&new_domain).await?;

    let instructions = generate_verification_instructions(
        &domain.host,
        &domain.verification_token,
        &domain.verification_method,
    );

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::DOMAIN_CREATE,
        Some(site.id),
        None,
        Some(domain.id),
        serde_json::json!({ "host": domain.host }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(DomainResponse {
            domain,
            verification_instructions: instructions,
        }),
    ))
}

/// List domains for a site.
pub async fn list_domains(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path(name): Path<String>,
) -> Result<Json<Vec<Domain>>, SitesError> {
    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name))?;

    let domains = store.list_domains(&site.id).await?;
    Ok(Json(domains))
}

/// Delete a domain.
pub async fn delete_domain(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path((name, host)): Path<(String, String)>,
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

    let domain = store
        .get_domain(&host)
        .await?
        .ok_or_else(|| SitesError::DomainUnverified(host.clone()))?;

    if domain.site_id != site.id {
        return Err(SitesError::DomainUnverified(host));
    }

    store.delete_domain(&domain.id).await?;

    let store = Arc::new(store);
    write_audit(
        &store,
        &ctx,
        event_types::DOMAIN_DELETE,
        Some(site.id),
        None,
        Some(domain.id),
        serde_json::json!({ "host": domain.host }),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Trigger domain verification.
pub async fn verify_domain(
    State(state): State<SitesState>,
    Extension(ctx): Extension<SiteCtx>,
    Path((name, host)): Path<(String, String)>,
) -> Result<Json<Domain>, SitesError> {
    let perm = format!("sites:{}:admin", name);
    if !ctx.has_permission(&perm) {
        return Err(SitesError::PermissionDenied(perm));
    }

    let store = PgSitesStore::new(state.pool.clone());

    let site = store
        .get_site(&ctx.active_org(), &name)
        .await?
        .ok_or_else(|| SitesError::SiteNotFound(name.clone()))?;

    let domain = store
        .get_domain(&host)
        .await?
        .ok_or_else(|| SitesError::DomainUnverified(host.clone()))?;

    if domain.site_id != site.id {
        return Err(SitesError::DomainUnverified(host));
    }

    let verified = crate::domain::verify::verify_domain(
        &domain.host,
        &domain.verification_token,
        &domain.verification_method,
    )
    .await?;

    if verified {
        store
            .update_domain_status(&domain.id, DomainStatus::Verified, None)
            .await?;

        let updated_domain = store
            .get_domain(&host)
            .await?
            .ok_or_else(|| SitesError::DomainUnverified(host))?;

        let store = Arc::new(store);
        write_audit(
            &store,
            &ctx,
            event_types::DOMAIN_VERIFY,
            Some(site.id),
            None,
            Some(domain.id),
            serde_json::json!({ "host": domain.host }),
        )
        .await?;

        Ok(Json(updated_domain))
    } else {
        Err(SitesError::DomainVerificationFailed(domain.host))
    }
}
