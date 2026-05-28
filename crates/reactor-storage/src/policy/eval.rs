//! Storage policy evaluation.

use crate::error::StorageError;
use crate::middleware::StorageCtx;
use crate::store::{MetadataStore, PgMetadataStore, StoredPolicy};
use reactor_policy::{
    PolicyEvalContext, PolicyExpr, PolicyExprKind, PolicyLiteral, PolicyScope, PolicyBinaryOp, PolicyUnaryOp,
};
use sqlx::PgPool;
use uuid::Uuid;

/// Facts about the object being accessed.
#[derive(Debug, Clone)]
pub struct ObjectFacts {
    /// Object key (path).
    pub key: String,
    /// Bucket slug.
    pub bucket: String,
    /// Content type (if known).
    pub content_type: Option<String>,
    /// Object owner user ID.
    pub owner_id: Option<Uuid>,
    /// Custom metadata.
    pub metadata: serde_json::Value,
}

impl ObjectFacts {
    /// Create new object facts.
    pub fn new(key: impl Into<String>, bucket: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            bucket: bucket.into(),
            content_type: None,
            owner_id: None,
            metadata: serde_json::json!({}),
        }
    }

    /// Set content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Set owner ID.
    pub fn with_owner(mut self, owner_id: Uuid) -> Self {
        self.owner_id = Some(owner_id);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Result of storage policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum StorageEvalResult {
    /// Constant boolean result.
    Const(bool),
    /// String value for comparisons.
    Value(serde_json::Value),
    /// Residual (cannot fully evaluate).
    Residual(String),
}

/// Evaluate a policy expression with storage-specific domain builtins.
fn evaluate_storage<C: PolicyEvalContext>(
    expr: &PolicyExpr,
    ctx: &C,
    facts: &ObjectFacts,
) -> StorageEvalResult {
    match &expr.kind {
        PolicyExprKind::Literal(lit) => match lit {
            PolicyLiteral::Bool(b) => StorageEvalResult::Const(*b),
            PolicyLiteral::Null => StorageEvalResult::Value(serde_json::Value::Null),
            PolicyLiteral::String(s) => StorageEvalResult::Value(serde_json::Value::String(s.clone())),
            PolicyLiteral::Int(n) => StorageEvalResult::Value(serde_json::json!(n)),
            PolicyLiteral::Float(n) => StorageEvalResult::Value(serde_json::json!(n)),
        },

        PolicyExprKind::Column { name } => {
            StorageEvalResult::Residual(format!("\"{}\"", name))
        }

        PolicyExprKind::QualifiedColumn { table, column } => {
            StorageEvalResult::Residual(format!("\"{}\".\"{}\"", table, column))
        }

        PolicyExprKind::AuthBuiltin { name, args } => {
            eval_auth_builtin_storage(name, args, ctx, facts)
        }

        PolicyExprKind::DomainBuiltin { domain, name, .. } => {
            eval_domain_builtin(domain, name, facts)
        }

        PolicyExprKind::BinaryOp { op, left, right } => {
            let left_result = evaluate_storage(left, ctx, facts);
            let right_result = evaluate_storage(right, ctx, facts);
            eval_binary_op_storage(op, left_result, right_result)
        }

        PolicyExprKind::UnaryOp { op, expr: operand } => {
            let operand_result = evaluate_storage(operand, ctx, facts);
            eval_unary_op_storage(op, operand_result)
        }

        PolicyExprKind::IsNull { expr: operand, negated } => {
            let operand_result = evaluate_storage(operand, ctx, facts);
            match operand_result {
                StorageEvalResult::Value(v) if v.is_null() => StorageEvalResult::Const(!negated),
                StorageEvalResult::Value(_) => StorageEvalResult::Const(*negated),
                StorageEvalResult::Const(b) => StorageEvalResult::Const(b),
                StorageEvalResult::Residual(s) => {
                    let op = if *negated { "IS NOT NULL" } else { "IS NULL" };
                    StorageEvalResult::Residual(format!("({}) {}", s, op))
                }
            }
        }

        PolicyExprKind::InList { expr: operand, list, negated } => {
            let operand_result = evaluate_storage(operand, ctx, facts);
            let list_results: Vec<StorageEvalResult> = list.iter()
                .map(|e| evaluate_storage(e, ctx, facts))
                .collect();
            eval_in_list_storage(operand_result, list_results, *negated)
        }

        PolicyExprKind::Subquery { sql } => {
            StorageEvalResult::Residual(format!("({})", sql))
        }
    }
}

fn eval_auth_builtin_storage<C: PolicyEvalContext>(
    name: &str,
    args: &[PolicyExpr],
    ctx: &C,
    facts: &ObjectFacts,
) -> StorageEvalResult {
    match name {
        "user_id" => {
            if let Some(user_id) = ctx.user_id() {
                StorageEvalResult::Value(serde_json::Value::String(user_id.to_string()))
            } else {
                StorageEvalResult::Value(serde_json::Value::Null)
            }
        }
        "org_id" => {
            if let Some(org_id) = ctx.org_id() {
                StorageEvalResult::Value(serde_json::Value::String(org_id.to_string()))
            } else {
                StorageEvalResult::Value(serde_json::Value::Null)
            }
        }
        "has_permission" => {
            if let Some(first_arg) = args.first() {
                let arg_result = evaluate_storage(first_arg, ctx, facts);
                if let StorageEvalResult::Value(serde_json::Value::String(perm)) = arg_result {
                    StorageEvalResult::Const(ctx.has_permission(&perm))
                } else {
                    StorageEvalResult::Const(false)
                }
            } else {
                StorageEvalResult::Const(false)
            }
        }
        "is_authenticated" => {
            StorageEvalResult::Const(ctx.user_id().is_some())
        }
        _ => StorageEvalResult::Residual(format!("auth.{}()", name)),
    }
}

fn eval_domain_builtin(domain: &str, field: &str, facts: &ObjectFacts) -> StorageEvalResult {
    match (domain, field) {
        ("object", "key") => StorageEvalResult::Value(serde_json::Value::String(facts.key.clone())),
        ("object", "content_type") => {
            StorageEvalResult::Value(facts.content_type.clone().map_or(
                serde_json::Value::Null,
                serde_json::Value::String,
            ))
        }
        ("object", "owner_id") => {
            StorageEvalResult::Value(facts.owner_id.map_or(
                serde_json::Value::Null,
                |id| serde_json::Value::String(id.to_string()),
            ))
        }
        ("object", "metadata") => StorageEvalResult::Value(facts.metadata.clone()),
        ("bucket", "name") => StorageEvalResult::Value(serde_json::Value::String(facts.bucket.clone())),
        _ => StorageEvalResult::Residual(format!("{}.{}", domain, field)),
    }
}

fn eval_binary_op_storage(op: &PolicyBinaryOp, left: StorageEvalResult, right: StorageEvalResult) -> StorageEvalResult {
    use StorageEvalResult::*;

    match (op, left, right) {
        // Boolean logic with constants
        (PolicyBinaryOp::And, Const(false), _) | (PolicyBinaryOp::And, _, Const(false)) => Const(false),
        (PolicyBinaryOp::And, Const(true), other) | (PolicyBinaryOp::And, other, Const(true)) => other,
        (PolicyBinaryOp::Or, Const(true), _) | (PolicyBinaryOp::Or, _, Const(true)) => Const(true),
        (PolicyBinaryOp::Or, Const(false), other) | (PolicyBinaryOp::Or, other, Const(false)) => other,

        // Equality comparisons with values
        (PolicyBinaryOp::Eq, Value(l), Value(r)) => Const(l == r),
        (PolicyBinaryOp::NotEq, Value(l), Value(r)) => Const(l != r),

        // String comparisons
        (PolicyBinaryOp::Like, Value(serde_json::Value::String(s)), Value(serde_json::Value::String(p))) => {
            Const(like_match(&s, &p))
        }

        // Numeric comparisons
        (PolicyBinaryOp::Lt, Value(l), Value(r)) => {
            if let (Some(l), Some(r)) = (l.as_f64(), r.as_f64()) {
                Const(l < r)
            } else {
                Residual(format!("{:?} < {:?}", l, r))
            }
        }
        (PolicyBinaryOp::Gt, Value(l), Value(r)) => {
            if let (Some(l), Some(r)) = (l.as_f64(), r.as_f64()) {
                Const(l > r)
            } else {
                Residual(format!("{:?} > {:?}", l, r))
            }
        }

        // Anything else becomes residual
        _ => Residual("(complex expression)".to_string()),
    }
}

fn eval_unary_op_storage(op: &PolicyUnaryOp, operand: StorageEvalResult) -> StorageEvalResult {
    match (op, operand) {
        (PolicyUnaryOp::Not, StorageEvalResult::Const(b)) => StorageEvalResult::Const(!b),
        (PolicyUnaryOp::Not, StorageEvalResult::Residual(s)) => {
            StorageEvalResult::Residual(format!("NOT ({})", s))
        }
        _ => StorageEvalResult::Residual("(complex expression)".to_string()),
    }
}

fn eval_in_list_storage(
    operand: StorageEvalResult,
    list: Vec<StorageEvalResult>,
    negated: bool,
) -> StorageEvalResult {
    if let StorageEvalResult::Value(v) = operand {
        let mut found = false;
        for item in &list {
            if let StorageEvalResult::Value(lv) = item {
                if *lv == v {
                    found = true;
                    break;
                }
            }
        }
        StorageEvalResult::Const(if negated { !found } else { found })
    } else {
        StorageEvalResult::Residual("(IN list)".to_string())
    }
}

/// Simple LIKE pattern matching (% is wildcard).
fn like_match(s: &str, pattern: &str) -> bool {
    if pattern == "%" {
        return true;
    }
    if !pattern.contains('%') {
        return s == pattern;
    }
    if pattern.starts_with('%') && pattern.ends_with('%') {
        let inner = &pattern[1..pattern.len() - 1];
        return s.contains(inner);
    }
    if pattern.starts_with('%') {
        let suffix = &pattern[1..];
        return s.ends_with(suffix);
    }
    if pattern.ends_with('%') {
        let prefix = &pattern[..pattern.len() - 1];
        return s.starts_with(prefix);
    }
    // More complex patterns - just do simple comparison
    s == pattern
}

/// Check if a policy expression allows access.
fn check_policy(
    expr: &PolicyExpr,
    ctx: &StorageCtx,
    facts: &ObjectFacts,
) -> Result<bool, StorageError> {
    let result = evaluate_storage(expr, ctx, facts);

    match result {
        StorageEvalResult::Const(b) => Ok(b),
        StorageEvalResult::Value(v) => Ok(v.as_bool().unwrap_or(false)),
        StorageEvalResult::Residual(_) => {
            // If we have residuals, we can't fully evaluate - deny by default
            Ok(false)
        }
    }
}

/// Check object access against bucket policies.
///
/// Returns Ok(()) if access is allowed, Err(StorageError::PermissionDenied) if denied.
pub async fn check_object_access(
    pool: &PgPool,
    bucket_id: Uuid,
    scope: PolicyScope,
    ctx: &StorageCtx,
    facts: &ObjectFacts,
) -> Result<(), StorageError> {
    let store = PgMetadataStore::new(pool.clone());

    // Load policies for this bucket
    let policies = store.list_policies(bucket_id).await?;

    // If no policies, allow by default (bucket-level permissions already checked)
    if policies.is_empty() {
        return Ok(());
    }

    // Filter policies by scope
    let scope_str = match scope {
        PolicyScope::Read => "read",
        PolicyScope::Write => "write",
        PolicyScope::Select => "read",
        PolicyScope::Insert => "write",
        PolicyScope::Update => "write",
        PolicyScope::Delete => "write",
    };

    let matching_policies: Vec<&StoredPolicy> = policies
        .iter()
        .filter(|p| p.scopes.iter().any(|s| s == scope_str))
        .collect();

    // If no matching policies for this scope, allow by default
    if matching_policies.is_empty() {
        return Ok(());
    }

    // Check each policy - ANY policy allowing grants access
    for policy in matching_policies {
        // Check USING clause (read filter)
        if let Some(ref using_ast) = policy.using_ast {
            let expr: PolicyExpr = serde_json::from_value(using_ast.clone())
                .map_err(|e| StorageError::Internal(format!("invalid policy AST: {}", e)))?;

            if check_policy(&expr, ctx, facts)? {
                return Ok(()); // Policy allows access
            }
        }

        // Check CHECK clause (write filter) - for write operations
        if scope == PolicyScope::Write || scope == PolicyScope::Insert || scope == PolicyScope::Update || scope == PolicyScope::Delete {
            if let Some(ref check_ast) = policy.check_ast {
                let expr: PolicyExpr = serde_json::from_value(check_ast.clone())
                    .map_err(|e| StorageError::Internal(format!("invalid policy AST: {}", e)))?;

                if check_policy(&expr, ctx, facts)? {
                    return Ok(()); // Policy allows access
                }
            }
        }
    }

    // No policy allowed access
    Err(StorageError::PermissionDenied(format!(
        "no policy grants {} access to {}/{}",
        scope_str, facts.bucket, facts.key
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_domain_builtin() {
        let facts = ObjectFacts::new("test/file.txt", "uploads")
            .with_content_type("text/plain")
            .with_owner(Uuid::nil());

        assert_eq!(
            eval_domain_builtin("object", "key", &facts),
            StorageEvalResult::Value(serde_json::json!("test/file.txt"))
        );

        assert_eq!(
            eval_domain_builtin("bucket", "name", &facts),
            StorageEvalResult::Value(serde_json::json!("uploads"))
        );

        assert_eq!(
            eval_domain_builtin("object", "content_type", &facts),
            StorageEvalResult::Value(serde_json::json!("text/plain"))
        );
    }

    #[test]
    fn test_like_match() {
        assert!(like_match("hello.txt", "hello.txt"));
        assert!(like_match("hello.txt", "%"));
        assert!(like_match("hello.txt", "%.txt"));
        assert!(like_match("hello.txt", "hello%"));
        assert!(like_match("hello.txt", "%llo%"));
        assert!(!like_match("hello.txt", "%.jpg"));
        assert!(!like_match("hello.txt", "world%"));
    }
}
