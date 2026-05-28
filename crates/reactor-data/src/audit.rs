//! Audit event logging for data mutations.
//!
//! All mutation operations (INSERT, UPDATE, DELETE, RPC) record audit events
//! in `_reactor_data.audit_events` within the same transaction as the mutation.

use crate::middleware::DataCtx;
use crate::store::SqlValue;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Event types for audit logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// Row insertion.
    #[serde(rename = "rows.insert")]
    RowsInsert,
    /// Row update.
    #[serde(rename = "rows.update")]
    RowsUpdate,
    /// Row deletion.
    #[serde(rename = "rows.delete")]
    RowsDelete,
    /// RPC invocation.
    #[serde(rename = "rpc.invoke")]
    RpcInvoke,
}

impl AuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RowsInsert => "rows.insert",
            Self::RowsUpdate => "rows.update",
            Self::RowsDelete => "rows.delete",
            Self::RpcInvoke => "rpc.invoke",
        }
    }
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An audit event to be recorded.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    /// Unique event ID.
    pub id: Uuid,
    /// User ID performing the action.
    pub actor_user_id: Option<Uuid>,
    /// API key ID performing the action (if any).
    pub actor_apikey_id: Option<Uuid>,
    /// Organization context.
    pub org_id: Option<Uuid>,
    /// Request ID for correlation.
    pub request_id: Uuid,
    /// Type of event.
    pub event_type: AuditEventType,
    /// Table name (for row operations).
    pub table_name: Option<String>,
    /// Number of rows affected.
    pub row_count: Option<i32>,
    /// Additional details.
    pub details: serde_json::Value,
}

impl AuditEvent {
    /// Create a new audit event from a data context.
    pub fn new(ctx: &DataCtx, event_type: AuditEventType) -> Self {
        // Extract API key ID from claims.sub if it's an API key token
        let actor_apikey_id = if ctx.auth.claims.is_apikey() {
            ctx.auth
                .claims
                .sub
                .strip_prefix("apikey:")
                .and_then(|id| id.parse::<Uuid>().ok())
        } else {
            None
        };

        Self {
            id: Uuid::now_v7(),
            actor_user_id: ctx.user_id().map(|id| *id.as_ref()),
            actor_apikey_id,
            org_id: ctx.org_id().map(|id| *id.as_ref()),
            request_id: ctx.request_id,
            event_type,
            table_name: None,
            row_count: None,
            details: serde_json::json!({}),
        }
    }

    /// Set the table name.
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table_name = Some(table.into());
        self
    }

    /// Set the row count.
    pub fn with_row_count(mut self, count: i32) -> Self {
        self.row_count = Some(count);
        self
    }

    /// Set additional details.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Add a detail field.
    pub fn with_detail(mut self, key: &str, value: impl Serialize) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.details {
            if let Ok(v) = serde_json::to_value(value) {
                map.insert(key.to_string(), v);
            }
        }
        self
    }

    /// Convert to SQL INSERT values.
    pub fn to_sql_values(&self) -> (String, Vec<SqlValue>) {
        let sql = r#"
            INSERT INTO _reactor_data.audit_events 
                (id, actor_user_id, actor_apikey_id, org_id, request_id, event_type, table_name, row_count, details)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#;

        let params = vec![
            SqlValue::Uuid(self.id),
            match self.actor_user_id {
                Some(id) => SqlValue::Uuid(id),
                None => SqlValue::NullUuid,
            },
            match self.actor_apikey_id {
                Some(id) => SqlValue::Uuid(id),
                None => SqlValue::NullUuid,
            },
            match self.org_id {
                Some(id) => SqlValue::Uuid(id),
                None => SqlValue::NullUuid,
            },
            SqlValue::Text(self.request_id.to_string()),
            SqlValue::Text(self.event_type.as_str().to_string()),
            match &self.table_name {
                Some(t) => SqlValue::Text(t.clone()),
                None => SqlValue::Null,
            },
            match self.row_count {
                Some(c) => SqlValue::Int(c as i64),
                None => SqlValue::NullInt,
            },
            SqlValue::Json(self.details.clone()),
        ];

        (sql.to_string(), params)
    }
}

/// Write an audit event within an existing transaction.
///
/// This should be called in the same transaction as the mutation to ensure
/// audit events are committed atomically with the data change.
pub async fn write_audit_event<T: crate::store::DataTx>(
    tx: &mut T,
    event: &AuditEvent,
) -> Result<(), crate::error::DataError> {
    let (sql, params) = event.to_sql_values();
    tx.execute_raw(&sql, &params).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reactor_core::auth::{AuthCtx, AuthMethod, Claims};
    use reactor_core::id::{OrgId, UserId};

    fn mock_ctx() -> DataCtx {
        let user_id = UserId::new();
        let org_id = OrgId::new();
        DataCtx {
            auth: AuthCtx {
                claims: Claims {
                    sub: format!("user_{}", user_id),
                    iss: "reactor-auth".to_string(),
                    aud: "reactor".to_string(),
                    exp: chrono::Utc::now().timestamp() + 3600,
                    iat: chrono::Utc::now().timestamp(),
                    nbf: None,
                    email: Some("test@example.com".to_string()),
                    amr: vec![AuthMethod::Pwd],
                    orgs: vec![org_id],
                    default_org: Some(org_id),
                    session_id: None,
                    scopes: vec![],
                    mfa_at: None,
                },
                active_org: Some(org_id),
                permissions: vec!["*".to_string()],
            },
            request_id: Uuid::now_v7(),
            schema: "public".to_string(),
        }
    }

    #[test]
    fn test_audit_event_builder() {
        let ctx = mock_ctx();
        let event = AuditEvent::new(&ctx, AuditEventType::RowsInsert)
            .with_table("todos")
            .with_row_count(5)
            .with_detail("policy_bypass", true);

        assert_eq!(event.event_type, AuditEventType::RowsInsert);
        assert_eq!(event.table_name, Some("todos".to_string()));
        assert_eq!(event.row_count, Some(5));
        assert_eq!(event.details["policy_bypass"], true);
    }

    #[test]
    fn test_event_type_serialization() {
        assert_eq!(AuditEventType::RowsInsert.as_str(), "rows.insert");
        assert_eq!(AuditEventType::RowsUpdate.as_str(), "rows.update");
        assert_eq!(AuditEventType::RowsDelete.as_str(), "rows.delete");
        assert_eq!(AuditEventType::RpcInvoke.as_str(), "rpc.invoke");
    }
}
