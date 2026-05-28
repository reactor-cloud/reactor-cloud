//! Audit logging.

use crate::store::{AuditEvent, ConnectStore};
use uuid::Uuid;

/// Audit logger.
pub struct AuditLogger<S: ConnectStore> {
    store: S,
}

impl<S: ConnectStore> AuditLogger<S> {
    /// Create a new audit logger.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Log an event.
    pub async fn log(&self, event: AuditEvent) -> Result<(), crate::error::ConnectError> {
        self.store.write_audit_event(&event).await
    }

    /// Log an instance created event.
    pub async fn instance_created(
        &self,
        ctx: &crate::state::ConnectCtx,
        instance: &crate::store::Instance,
    ) -> Result<(), crate::error::ConnectError> {
        self.log(AuditEvent {
            id: Uuid::now_v7(),
            ts: chrono::Utc::now(),
            actor_user_id: ctx.user_id(),
            actor_apikey_id: None,
            org_id: Some(*ctx.active_org()),
            instance_id: Some(instance.id),
            connection_id: None,
            receiver_id: None,
            event_type: "instance.created".to_string(),
            details: serde_json::json!({
                "type_id": instance.type_id,
                "name": instance.name,
            }),
            request_id: ctx.request_id.clone(),
        })
        .await
    }

    /// Log an action invoked event.
    pub async fn action_invoked(
        &self,
        ctx: &crate::state::ConnectCtx,
        instance_id: crate::store::InstanceId,
        action: &str,
        dry_run: bool,
    ) -> Result<(), crate::error::ConnectError> {
        self.log(AuditEvent {
            id: Uuid::now_v7(),
            ts: chrono::Utc::now(),
            actor_user_id: ctx.user_id(),
            actor_apikey_id: None,
            org_id: Some(*ctx.active_org()),
            instance_id: Some(instance_id),
            connection_id: None,
            receiver_id: None,
            event_type: "action.invoked".to_string(),
            details: serde_json::json!({
                "action": action,
                "dry_run": dry_run,
            }),
            request_id: ctx.request_id.clone(),
        })
        .await
    }
}
