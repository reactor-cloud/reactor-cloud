//! Audit event writer for admin actions.

use crate::store::{AuditEventCreate, SitesStore};
use crate::SiteCtx;
use std::sync::Arc;

/// Audit event types for sites.
pub mod event_types {
    pub const SITE_CREATE: &str = "site.create";
    pub const SITE_DELETE: &str = "site.delete";
    pub const DEPLOYMENT_CREATE: &str = "deployment.create";
    pub const DEPLOYMENT_PROMOTE: &str = "deployment.promote";
    pub const DEPLOYMENT_ROLLBACK: &str = "deployment.rollback";
    pub const DEPLOYMENT_FAIL: &str = "deployment.fail";
    pub const DEPLOYMENT_DESTROY: &str = "deployment.destroy";
    pub const DOMAIN_CREATE: &str = "domain.create";
    pub const DOMAIN_VERIFY: &str = "domain.verify";
    pub const DOMAIN_ACTIVATE: &str = "domain.activate";
    pub const DOMAIN_DELETE: &str = "domain.delete";
    pub const POLICY_CREATE: &str = "policy.create";
    pub const POLICY_DELETE: &str = "policy.delete";
    pub const ISR_INVALIDATE: &str = "isr.invalidate";
}

/// Write an audit event.
pub async fn write_audit<S: SitesStore>(
    store: &Arc<S>,
    ctx: &SiteCtx,
    event_type: &str,
    site_id: Option<uuid::Uuid>,
    deployment_id: Option<uuid::Uuid>,
    domain_id: Option<uuid::Uuid>,
    details: serde_json::Value,
) -> Result<(), crate::SitesError> {
    let actor_apikey_id = if ctx.auth.claims.is_apikey() {
        ctx.auth
            .claims
            .sub
            .strip_prefix("apikey:")
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
    } else {
        None
    };

    let event = AuditEventCreate {
        actor_user_id: ctx.user_id().map(|id| id.into()),
        actor_apikey_id,
        org_id: Some(ctx.active_org()),
        site_id,
        deployment_id,
        domain_id,
        event_type: event_type.to_string(),
        details,
        request_id: ctx.request_id.clone(),
    };

    store.write_audit_event(&event).await
}
