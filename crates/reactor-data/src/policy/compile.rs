//! Policy compilation for reactor-data.
//!
//! This module provides the data-specific policy compilation that loads
//! policies from the database and delegates to the shared reactor-policy engine.

use reactor_policy::{
    evaluate, CompiledPolicy, EvalResult, PolicyDecision, PolicyExpr, PolicyExprKind, PolicyLiteral,
    PolicyScope,
};

use super::store::{PolicyStore, StoredPolicy};
use crate::error::DataError;
use crate::middleware::DataCtx;
use sqlx::PgPool;

/// Compile policies for a table and scope.
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `table` - Target table name
/// * `scope` - The operation scope (select, insert, update, delete)
/// * `ctx` - Authentication context
///
/// # Returns
///
/// A `PolicyDecision` indicating whether to allow, deny, or apply conditions.
pub async fn compile_for_scope(
    pool: &PgPool,
    table: &str,
    scope: PolicyScope,
    ctx: &DataCtx,
) -> Result<PolicyDecision, DataError> {
    // Check for superuser bypass (* permission)
    if ctx.has_permission("*") {
        tracing::debug!(
            user_id = ?ctx.user_id(),
            table = table,
            scope = ?scope,
            "superuser bypass - skipping policy evaluation"
        );
        return Ok(PolicyDecision::AlwaysAllow);
    }

    // Load policies for this table and scope
    let store = PolicyStore::new(pool);
    let policies = store.get_for_scope(&ctx.schema, table, scope).await?;

    if policies.is_empty() {
        // No policies defined - allow by default
        return Ok(PolicyDecision::AlwaysAllow);
    }

    // Convert StoredPolicy to CompiledPolicy for the shared engine
    let compiled: Vec<CompiledPolicy> = policies
        .iter()
        .map(|p| {
            let mut cp = CompiledPolicy::new(&p.name);
            if let Some(ref expr) = p.using_expr {
                cp = cp.with_using(expr.clone());
            }
            if let Some(ref expr) = p.check_expr {
                cp = cp.with_check(expr.clone());
            }
            cp
        })
        .collect();

    // Use the shared engine to compile
    // Note: We pass false for has_superuser_bypass since we already checked above
    let decision = reactor_policy::compile_policies(&compiled, ctx, false);

    // For INSERT/UPDATE, we need to also apply CHECK policies
    // The shared engine handles this internally
    Ok(decision)
}

/// Check a single row against CHECK policies.
///
/// Used for INSERT/UPDATE to validate the proposed row data.
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `table` - Target table name
/// * `scope` - The operation scope (insert or update)
/// * `ctx` - Authentication context
/// * `row_data` - The proposed row data as JSON
///
/// # Returns
///
/// `true` if the row passes all check policies, `false` otherwise.
pub async fn check_row(
    pool: &PgPool,
    table: &str,
    scope: PolicyScope,
    ctx: &DataCtx,
    row_data: &serde_json::Value,
) -> Result<bool, DataError> {
    // Superuser bypass
    if ctx.has_permission("*") {
        return Ok(true);
    }

    // Load CHECK policies
    let store = PolicyStore::new(pool);
    let policies = store.get_for_scope(&ctx.schema, table, scope).await?;
    let check_policies: Vec<&StoredPolicy> =
        policies.iter().filter(|p| p.check_expr.is_some()).collect();

    if check_policies.is_empty() {
        return Ok(true);
    }

    // Evaluate each CHECK policy
    for policy in check_policies {
        if let Some(ref check_expr) = policy.check_expr {
            let result = evaluate_with_row(check_expr, ctx, row_data);
            match result {
                EvalResult::Const(true) => continue,
                EvalResult::Const(false) => return Ok(false),
                EvalResult::Residual(_) => {
                    // Cannot fully evaluate - would need to execute SQL
                    // For now, we'll be conservative and allow it
                    // (the SQL predicate will be applied anyway)
                    continue;
                }
            }
        }
    }

    Ok(true)
}

/// Evaluate a policy expression with row data available for column references.
fn evaluate_with_row(
    expr: &PolicyExpr,
    ctx: &DataCtx,
    row_data: &serde_json::Value,
) -> EvalResult {
    match &expr.kind {
        PolicyExprKind::Column { name } => {
            // Try to get the value from row_data
            if let Some(value) = row_data.get(name) {
                value_to_eval_result(value)
            } else {
                // Column not in row data - leave as residual
                EvalResult::Residual(format!("\"{}\"", name))
            }
        }

        PolicyExprKind::QualifiedColumn { table: _, column } => {
            // For qualified columns, try the column name only
            if let Some(value) = row_data.get(column) {
                value_to_eval_result(value)
            } else {
                EvalResult::Residual(format!("\"{}\"", column))
            }
        }

        PolicyExprKind::BinaryOp { op, left, right } => {
            let left_result = evaluate_with_row(left, ctx, row_data);
            let right_result = evaluate_with_row(right, ctx, row_data);
            evaluate(
                &PolicyExpr {
                    kind: PolicyExprKind::BinaryOp {
                        op: *op,
                        left: Box::new(result_to_literal_expr(&left_result)),
                        right: Box::new(result_to_literal_expr(&right_result)),
                    },
                },
                ctx,
            )
        }

        // For other cases, delegate to the standard evaluate
        _ => evaluate(expr, ctx),
    }
}

fn value_to_eval_result(value: &serde_json::Value) -> EvalResult {
    match value {
        serde_json::Value::Null => EvalResult::Residual("NULL".to_string()),
        serde_json::Value::Bool(b) => EvalResult::Const(*b),
        serde_json::Value::Number(n) => EvalResult::Residual(n.to_string()),
        serde_json::Value::String(s) => {
            EvalResult::Residual(format!("'{}'", s.replace('\'', "''")))
        }
        _ => EvalResult::Residual("NULL".to_string()),
    }
}

fn result_to_literal_expr(result: &EvalResult) -> PolicyExpr {
    match result {
        EvalResult::Const(true) => PolicyExpr::bool_literal(true),
        EvalResult::Const(false) => PolicyExpr::bool_literal(false),
        EvalResult::Residual(s) => {
            // Parse the residual back - this is a simplification
            // In a real implementation, we'd preserve the AST
            PolicyExpr {
                kind: PolicyExprKind::Literal(PolicyLiteral::String(s.clone())),
            }
        }
    }
}

/// Result of evaluating CHECK policies for a batch of rows.
#[derive(Debug)]
pub struct BatchCheckResult {
    /// Indices of rows that passed all checks.
    pub allowed_indices: Vec<usize>,
    /// Indices of rows that failed checks, with reasons.
    pub denied: Vec<(usize, String)>,
}

impl BatchCheckResult {
    /// Check if all rows were allowed.
    pub fn all_allowed(&self) -> bool {
        self.denied.is_empty()
    }

    /// Check if any rows were denied.
    pub fn any_denied(&self) -> bool {
        !self.denied.is_empty()
    }
}

/// Check multiple rows against CHECK policies.
///
/// Returns a batch result indicating which rows passed and which failed.
pub async fn check_rows_batch(
    pool: &PgPool,
    table: &str,
    scope: PolicyScope,
    ctx: &DataCtx,
    rows: &[serde_json::Value],
) -> Result<BatchCheckResult, DataError> {
    let mut allowed_indices = Vec::new();
    let mut denied = Vec::new();

    for (idx, row_data) in rows.iter().enumerate() {
        if check_row(pool, table, scope, ctx, row_data).await? {
            allowed_indices.push(idx);
        } else {
            denied.push((idx, "check policy failed".to_string()));
        }
    }

    Ok(BatchCheckResult {
        allowed_indices,
        denied,
    })
}

#[cfg(test)]
mod tests {
    use reactor_policy::PolicyDecision;

    #[test]
    fn test_policy_decision_allows() {
        assert!(PolicyDecision::AlwaysAllow.allows());
        assert!(!PolicyDecision::AlwaysDeny {
            reason: "test".to_string()
        }
        .allows());
        assert!(!PolicyDecision::Conditional {
            sql_fragment: "1=1".to_string()
        }
        .allows());
    }

    #[test]
    fn test_policy_decision_denies() {
        assert!(!PolicyDecision::AlwaysAllow.denies());
        assert!(PolicyDecision::AlwaysDeny {
            reason: "test".to_string()
        }
        .denies());
        assert!(!PolicyDecision::Conditional {
            sql_fragment: "1=1".to_string()
        }
        .denies());
    }
}
