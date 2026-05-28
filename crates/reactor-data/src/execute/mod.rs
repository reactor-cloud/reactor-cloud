//! Query execution engine.
//!
//! Translates QueryPlan into SQL and executes against the database.

pub mod embed;
mod format;

use crate::audit::{write_audit_event, AuditEvent, AuditEventType};
use crate::error::DataError;
use crate::middleware::DataCtx;
use crate::policy::{compile_for_scope, PolicyDecision, PolicyScope};
use crate::query::{CountMode, Prefer, QueryPlan, ReturnMode, SelectColumn};
use crate::store::{DataStore, DataTx, Row, SchemaSnapshot};
use crate::DataState;
use format::SqlBuilder;
use serde_json::Value;

/// Result of a query execution.
#[derive(Debug)]
pub struct QueryResult {
    /// Returned rows (for SELECT or RETURNING).
    pub rows: Vec<Row>,
    /// Number of rows returned.
    pub rows_returned: u32,
    /// Total count (if requested).
    pub total_count: Option<u64>,
    /// Number of affected rows (for mutations).
    pub affected_rows: Option<u64>,
}

/// Execute a SELECT query.
pub async fn execute_select<S: DataStore + Clone>(
    state: &DataState<S>,
    ctx: &DataCtx,
    table: &str,
    plan: &QueryPlan,
) -> Result<QueryResult, DataError> {
    // Introspect schema
    let snapshot = state.store.introspect_schema(&ctx.schema).await?;

    // Validate table exists
    let table_info = snapshot
        .get_table(table)
        .ok_or_else(|| DataError::TableNotFound(table.to_string()))?;

    // Validate columns
    validate_columns(plan, table_info, &snapshot)?;

    // Compile policy for SELECT scope
    let policy_decision =
        compile_for_scope(state.store.pool(), table, PolicyScope::Select, ctx).await?;

    // Handle policy decision
    let mut plan_with_policy = plan.clone();
    match policy_decision {
        PolicyDecision::AlwaysDeny { reason } => {
            return Err(DataError::PolicyDenied(reason));
        }
        PolicyDecision::AlwaysAllow => {
            // No predicate needed
        }
        PolicyDecision::Conditional { sql_fragment } => {
            plan_with_policy.policy_predicate = Some(sql_fragment);
        }
    }

    // Resolve embedded resources if any
    let resolved_embeds = if plan.has_embeds() {
        let embed_specs: Vec<_> = plan.embeds();
        embed::resolve_embeds(
            &embed_specs,
            &snapshot,
            table_info,
            ctx,
            state.config.max_embed_depth as usize,
            0,
        )?
    } else {
        vec![]
    };

    // Build SQL (with embeds if present)
    let builder = SqlBuilder::new(&ctx.schema, table);
    let (sql, params) = if resolved_embeds.is_empty() {
        builder.build_select(&plan_with_policy, table_info)?
    } else {
        builder.build_select_with_embeds(&plan_with_policy, table_info, &resolved_embeds, ctx, state.store.pool()).await?
    };

    // Execute
    let mut tx = state.store.begin().await?;
    let rows: Vec<Row> = tx.fetch_rows(&sql, &params).await?;
    let rows_returned = rows.len() as u32;

    // Get count if requested
    let total_count = if plan_with_policy.prefer.count != CountMode::None {
        let (count_sql, count_params) = builder.build_count(&plan_with_policy)?;
        let count_rows: Vec<Row> = tx.fetch_rows(&count_sql, &count_params).await?;
        count_rows
            .first()
            .and_then(|r: &Row| r.get("count"))
            .and_then(|v: &serde_json::Value| v.as_i64())
            .map(|c| c as u64)
    } else {
        None
    };

    tx.commit().await?;

    Ok(QueryResult {
        rows,
        rows_returned,
        total_count,
        affected_rows: None,
    })
}

/// Execute an INSERT query.
pub async fn execute_insert<S: DataStore + Clone>(
    state: &DataState<S>,
    ctx: &DataCtx,
    table: &str,
    body: Value,
    prefer: &Prefer,
    plan: &QueryPlan,
) -> Result<QueryResult, DataError> {
    use crate::policy::check_rows_batch;

    // Introspect schema
    let snapshot = state.store.introspect_schema(&ctx.schema).await?;

    // Validate table exists
    let table_info = snapshot
        .get_table(table)
        .ok_or_else(|| DataError::TableNotFound(table.to_string()))?;

    // Parse body as array of objects or single object
    let rows_to_insert: Vec<Value> = match body {
        Value::Array(arr) => arr,
        Value::Object(_) => vec![body],
        _ => {
            return Err(DataError::InvalidQuery(
                "body must be object or array".to_string(),
            ))
        }
    };

    if rows_to_insert.is_empty() {
        return Ok(QueryResult {
            rows: vec![],
            rows_returned: 0,
            total_count: None,
            affected_rows: Some(0),
        });
    }

    // Compile policy for INSERT scope
    let policy_decision =
        compile_for_scope(state.store.pool(), table, PolicyScope::Insert, ctx).await?;

    // Handle policy decision
    match &policy_decision {
        PolicyDecision::AlwaysDeny { reason } => {
            return Err(DataError::PolicyDenied(reason.clone()));
        }
        PolicyDecision::AlwaysAllow => {
            // No policy check needed, proceed
        }
        PolicyDecision::Conditional { .. } => {
            // For INSERT with conditional CHECK policies, validate each row
            let check_result =
                check_rows_batch(state.store.pool(), table, PolicyScope::Insert, ctx, &rows_to_insert)
                    .await?;

            if !check_result.denied.is_empty() {
                // Some rows failed CHECK policy
                // For now, deny the entire batch. TODO: implement 207 Multi-Status
                let first_denied = &check_result.denied[0];
                return Err(DataError::PolicyDenied(format!(
                    "row {} failed check policy: {}",
                    first_denied.0, first_denied.1
                )));
            }
        }
    }

    // Build SQL
    let builder = SqlBuilder::new(&ctx.schema, table);
    let (sql, params) = builder.build_insert(&rows_to_insert, table_info, prefer, plan)?;

    // Execute
    let mut tx = state.store.begin().await?;

    let result = if prefer.return_mode == ReturnMode::Representation {
        let rows: Vec<Row> = tx.fetch_rows(&sql, &params).await?;
        QueryResult {
            rows_returned: rows.len() as u32,
            affected_rows: Some(rows.len() as u64),
            rows,
            total_count: None,
        }
    } else {
        let affected = tx.execute_raw(&sql, &params).await?;
        QueryResult {
            rows: vec![],
            rows_returned: 0,
            total_count: None,
            affected_rows: Some(affected),
        }
    };

    // Write audit event in same transaction
    let audit_event = AuditEvent::new(ctx, AuditEventType::RowsInsert)
        .with_table(table)
        .with_row_count(result.affected_rows.unwrap_or(0) as i32);
    write_audit_event(&mut tx, &audit_event).await?;

    tx.commit().await?;

    Ok(result)
}

/// Execute an UPDATE query.
pub async fn execute_update<S: DataStore + Clone>(
    state: &DataState<S>,
    ctx: &DataCtx,
    table: &str,
    body: Value,
    prefer: &Prefer,
    plan: &QueryPlan,
) -> Result<QueryResult, DataError> {
    // Introspect schema
    let snapshot = state.store.introspect_schema(&ctx.schema).await?;

    // Validate table exists
    let table_info = snapshot
        .get_table(table)
        .ok_or_else(|| DataError::TableNotFound(table.to_string()))?;

    // Body must be an object
    let updates = match body {
        Value::Object(obj) => obj,
        _ => {
            return Err(DataError::InvalidQuery(
                "body must be an object".to_string(),
            ))
        }
    };

    if updates.is_empty() {
        return Ok(QueryResult {
            rows: vec![],
            rows_returned: 0,
            total_count: None,
            affected_rows: Some(0),
        });
    }

    // Compile policy for UPDATE scope
    let policy_decision =
        compile_for_scope(state.store.pool(), table, PolicyScope::Update, ctx).await?;

    // Handle policy decision
    let mut plan_with_policy = plan.clone();
    match policy_decision {
        PolicyDecision::AlwaysDeny { reason } => {
            return Err(DataError::PolicyDenied(reason));
        }
        PolicyDecision::AlwaysAllow => {
            // No predicate needed
        }
        PolicyDecision::Conditional { sql_fragment } => {
            plan_with_policy.policy_predicate = Some(sql_fragment);
        }
    }

    // Build SQL
    let builder = SqlBuilder::new(&ctx.schema, table);
    let (sql, params) = builder.build_update(&updates, table_info, prefer, &plan_with_policy)?;

    // Execute
    let mut tx = state.store.begin().await?;

    let result = if prefer.return_mode == ReturnMode::Representation {
        let rows: Vec<Row> = tx.fetch_rows(&sql, &params).await?;
        QueryResult {
            rows_returned: rows.len() as u32,
            affected_rows: Some(rows.len() as u64),
            rows,
            total_count: None,
        }
    } else {
        let affected = tx.execute_raw(&sql, &params).await?;
        QueryResult {
            rows: vec![],
            rows_returned: 0,
            total_count: None,
            affected_rows: Some(affected),
        }
    };

    // Write audit event in same transaction
    let audit_event = AuditEvent::new(ctx, AuditEventType::RowsUpdate)
        .with_table(table)
        .with_row_count(result.affected_rows.unwrap_or(0) as i32);
    write_audit_event(&mut tx, &audit_event).await?;

    tx.commit().await?;

    Ok(result)
}

/// Execute a DELETE query.
pub async fn execute_delete<S: DataStore + Clone>(
    state: &DataState<S>,
    ctx: &DataCtx,
    table: &str,
    prefer: &Prefer,
    plan: &QueryPlan,
) -> Result<QueryResult, DataError> {
    // Introspect schema
    let snapshot = state.store.introspect_schema(&ctx.schema).await?;

    // Validate table exists
    snapshot
        .get_table(table)
        .ok_or_else(|| DataError::TableNotFound(table.to_string()))?;

    // Compile policy for DELETE scope
    let policy_decision =
        compile_for_scope(state.store.pool(), table, PolicyScope::Delete, ctx).await?;

    // Handle policy decision
    let mut plan_with_policy = plan.clone();
    match policy_decision {
        PolicyDecision::AlwaysDeny { reason } => {
            return Err(DataError::PolicyDenied(reason));
        }
        PolicyDecision::AlwaysAllow => {
            // No predicate needed
        }
        PolicyDecision::Conditional { sql_fragment } => {
            plan_with_policy.policy_predicate = Some(sql_fragment);
        }
    }

    // Build SQL
    let builder = SqlBuilder::new(&ctx.schema, table);
    let (sql, params) = builder.build_delete(prefer, &plan_with_policy)?;

    // Execute
    let mut tx = state.store.begin().await?;

    let result = if prefer.return_mode == ReturnMode::Representation {
        let rows: Vec<Row> = tx.fetch_rows(&sql, &params).await?;
        QueryResult {
            rows_returned: rows.len() as u32,
            affected_rows: Some(rows.len() as u64),
            rows,
            total_count: None,
        }
    } else {
        let affected = tx.execute_raw(&sql, &params).await?;
        QueryResult {
            rows: vec![],
            rows_returned: 0,
            total_count: None,
            affected_rows: Some(affected),
        }
    };

    // Write audit event in same transaction
    let audit_event = AuditEvent::new(ctx, AuditEventType::RowsDelete)
        .with_table(table)
        .with_row_count(result.affected_rows.unwrap_or(0) as i32);
    write_audit_event(&mut tx, &audit_event).await?;

    tx.commit().await?;

    Ok(result)
}

/// Validate that columns referenced in the plan exist in the table.
fn validate_columns(
    plan: &QueryPlan,
    table_info: &crate::store::TableSnapshot,
    _snapshot: &SchemaSnapshot,
) -> Result<(), DataError> {
    let valid_columns: Vec<&str> = table_info.columns.iter().map(|c| c.name.as_str()).collect();

    // Validate select columns
    for col in &plan.select {
        if let SelectColumn::Column(name) | SelectColumn::Aliased { column: name, .. } = col {
            if !valid_columns.contains(&name.as_str()) {
                return Err(DataError::ColumnNotFound(name.clone()));
            }
        }
    }

    // Validate filter columns
    for filter in &plan.filters {
        if !valid_columns.contains(&filter.column.as_str()) {
            return Err(DataError::ColumnNotFound(filter.column.clone()));
        }
    }

    // Validate order columns
    for order in &plan.order {
        if !valid_columns.contains(&order.column.as_str()) {
            return Err(DataError::ColumnNotFound(order.column.clone()));
        }
    }

    Ok(())
}
