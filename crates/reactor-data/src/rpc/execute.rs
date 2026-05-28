//! RPC function execution.
//!
//! Executes registered SQL functions with JSON argument binding.

use crate::audit::{write_audit_event, AuditEvent, AuditEventType};
use crate::error::DataError;
use crate::middleware::DataCtx;
use crate::rpc::store::{RpcStore, SecurityMode};
use crate::store::{DataStore, DataTx, Row, SqlValue};
use crate::DataState;
use serde_json::Value;
use std::collections::HashMap;

/// Result of an RPC execution.
#[derive(Debug)]
pub struct RpcResult {
    /// Returned rows.
    pub rows: Vec<Row>,
    /// Number of rows returned.
    pub rows_returned: u32,
}

/// Execute an RPC function.
pub async fn execute_rpc<S: DataStore + Clone>(
    state: &DataState<S>,
    ctx: &DataCtx,
    name: &str,
    args: Value,
) -> Result<RpcResult, DataError> {
    // Check permission: data:rpc:{name}:invoke
    let permission = format!("data:rpc:{}:invoke", name);
    if !ctx.has_permission(&permission) && !ctx.has_permission("*") {
        return Err(DataError::PermissionDenied);
    }

    // Look up the function
    let store = RpcStore::new(state.store.pool());
    let func = store.get(&ctx.schema, name).await.map_err(|e| match e {
        crate::rpc::store::RpcStoreError::NotFound(n) => {
            DataError::InvalidQuery(format!("RPC function not found: {}", n))
        }
        crate::rpc::store::RpcStoreError::Database(e) => {
            tracing::error!(error = %e, "database error looking up RPC function");
            DataError::Database
        }
        crate::rpc::store::RpcStoreError::Serialization(e) => {
            tracing::error!(error = %e, "serialization error for RPC function");
            DataError::Internal
        }
    })?;

    // Parse args as object
    let args_map: HashMap<String, Value> = match args {
        Value::Object(map) => map.into_iter().collect(),
        Value::Null => HashMap::new(),
        _ => {
            return Err(DataError::InvalidQuery(
                "RPC args must be an object".to_string(),
            ))
        }
    };

    // Build parameter list
    let mut params: Vec<SqlValue> = Vec::with_capacity(func.params.len());
    for param in &func.params {
        let value = args_map.get(&param.name);
        if value.is_none() && !param.has_default {
            return Err(DataError::InvalidQuery(format!(
                "missing required parameter: {}",
                param.name
            )));
        }
        params.push(json_to_sql_value(value, &param.sql_type));
    }

    // Build the SQL call
    let placeholders: Vec<String> = (1..=params.len()).map(|i| format!("${}", i)).collect();
    let sql = if func.returns_set {
        format!(
            "SELECT * FROM \"{}\".\"{}\"({})",
            func.schema_name,
            func.name,
            placeholders.join(", ")
        )
    } else {
        format!(
            "SELECT \"{}\".\"{}\"({}) AS result",
            func.schema_name,
            func.name,
            placeholders.join(", ")
        )
    };

    // Log for audit (security mode)
    match func.security {
        SecurityMode::Definer => {
            tracing::info!(
                user_id = ?ctx.user_id(),
                function = name,
                security = "definer",
                "RPC invocation with definer security (policy bypass)"
            );
        }
        SecurityMode::Invoker => {
            tracing::debug!(
                user_id = ?ctx.user_id(),
                function = name,
                security = "invoker",
                "RPC invocation with invoker security"
            );
        }
    }

    // Execute
    let mut tx = state.store.begin().await?;
    let rows: Vec<Row> = tx.fetch_rows(&sql, &params).await?;
    let rows_returned = rows.len() as u32;

    // Write audit event in same transaction
    let audit_event = AuditEvent::new(ctx, AuditEventType::RpcInvoke)
        .with_detail("fn", name)
        .with_detail("args", &args_map)
        .with_detail("security", func.security.as_str());
    write_audit_event(&mut tx, &audit_event).await?;

    tx.commit().await?;

    Ok(RpcResult {
        rows,
        rows_returned,
    })
}

/// Convert JSON value to SQL value with type hint.
fn json_to_sql_value(value: Option<&Value>, sql_type: &str) -> SqlValue {
    match value {
        None | Some(Value::Null) => SqlValue::Null,
        Some(Value::Bool(b)) => SqlValue::Bool(*b),
        Some(Value::Number(n)) => {
            // Use type hint to determine int vs float
            if sql_type.contains("int") || sql_type == "bigint" || sql_type == "smallint" {
                if let Some(i) = n.as_i64() {
                    return SqlValue::Int(i);
                }
            }
            if let Some(f) = n.as_f64() {
                SqlValue::Float(f)
            } else if let Some(i) = n.as_i64() {
                SqlValue::Int(i)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        Some(Value::String(s)) => {
            // Try to parse based on type hint
            if sql_type == "uuid" {
                if let Ok(uuid) = s.parse::<uuid::Uuid>() {
                    return SqlValue::Uuid(uuid);
                }
            }
            if sql_type.contains("timestamp") {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(s) {
                    return SqlValue::Timestamp(ts.with_timezone(&chrono::Utc));
                }
            }
            SqlValue::Text(s.clone())
        }
        Some(Value::Array(_)) | Some(Value::Object(_)) => {
            SqlValue::Json(value.cloned().unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_sql_value_types() {
        // Int
        let v = serde_json::json!(42);
        let sql = json_to_sql_value(Some(&v), "integer");
        assert!(matches!(sql, SqlValue::Int(42)));

        // Float
        let v = serde_json::json!(3.14);
        let sql = json_to_sql_value(Some(&v), "float");
        assert!(matches!(sql, SqlValue::Float(_)));

        // String
        let v = serde_json::json!("hello");
        let sql = json_to_sql_value(Some(&v), "text");
        assert!(matches!(sql, SqlValue::Text(ref s) if s == "hello"));

        // UUID
        let v = serde_json::json!("550e8400-e29b-41d4-a716-446655440000");
        let sql = json_to_sql_value(Some(&v), "uuid");
        assert!(matches!(sql, SqlValue::Uuid(_)));

        // Null
        let sql = json_to_sql_value(None, "text");
        assert!(matches!(sql, SqlValue::Null));
    }
}
