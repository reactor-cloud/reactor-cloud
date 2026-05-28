//! Audit logging for analytics admin operations.

use crate::error::AnalyticsError;
use crate::state::AnalyticsCtx;
use crate::store::{AnalyticsStore, AuditEvent};
use std::sync::Arc;
use uuid::Uuid;

/// Audit event types for analytics.
pub mod event_types {
    /// Project created.
    pub const PROJECT_CREATE: &str = "project.create";
    /// Project deleted.
    pub const PROJECT_DELETE: &str = "project.delete";
    /// Project key issued.
    pub const KEY_ISSUE: &str = "key.issue";
    /// Project key revoked.
    pub const KEY_REVOKE: &str = "key.revoke";
    /// User data erased.
    pub const USER_ERASE: &str = "user.erase";
    /// Anonymous ID data erased.
    pub const ANON_ERASE: &str = "anon.erase";
    /// Consent opt-out.
    pub const CONSENT_OPT_OUT: &str = "consent.opt_out";
    /// Consent opt-in.
    pub const CONSENT_OPT_IN: &str = "consent.opt_in";
}

/// Write an audit event for an admin action.
pub async fn write_audit<S: AnalyticsStore>(
    store: &Arc<S>,
    ctx: &AnalyticsCtx,
    event_type: &str,
    project_id: Option<Uuid>,
    details: serde_json::Value,
) -> Result<(), AnalyticsError> {
    let event = AuditEvent {
        actor_user_id: ctx.user_id().map(Into::into),
        actor_apikey_id: None,
        org_id: Some(ctx.org_id.into()),
        project_id,
        event_type: event_type.to_string(),
        details,
        request_id: ctx.request_id.to_string(),
    };

    store.write_audit_event(&event).await
}
