//! Policy expression evaluation against authentication context.
//!
//! Evaluates `PolicyExpr` nodes eagerly against a `PolicyEvalContext`, producing
//! either a constant boolean or a residual SQL fragment.

use crate::ast::{PolicyBinaryOp, PolicyExpr, PolicyExprKind, PolicyLiteral, PolicyUnaryOp};
use crate::context::PolicyEvalContext;

/// Result of evaluating a policy expression.
#[derive(Debug, Clone)]
pub enum EvalResult {
    /// Constant boolean result (fully evaluated).
    Const(bool),
    /// Residual expression that must be evaluated in SQL/runtime.
    Residual(String),
}

impl EvalResult {
    /// Check if this is a constant true.
    pub fn is_always_true(&self) -> bool {
        matches!(self, EvalResult::Const(true))
    }

    /// Check if this is a constant false.
    pub fn is_always_false(&self) -> bool {
        matches!(self, EvalResult::Const(false))
    }
}

/// Evaluate a policy expression against the authentication context.
///
/// Auth builtins like `auth.user_id()`, `auth.org_id()`, and `auth.has_permission()`
/// are evaluated eagerly. Column references and other expressions that depend on
/// row data are left as residual SQL.
pub fn evaluate<C: PolicyEvalContext>(expr: &PolicyExpr, ctx: &C) -> EvalResult {
    match &expr.kind {
        PolicyExprKind::Literal(lit) => eval_literal(lit),

        PolicyExprKind::Column { name } => {
            // Column references become SQL fragments
            EvalResult::Residual(format!("\"{}\"", name))
        }

        PolicyExprKind::QualifiedColumn { table, column } => {
            EvalResult::Residual(format!("\"{}\".\"{}\"", table, column))
        }

        PolicyExprKind::AuthBuiltin { name, args } => eval_auth_builtin(name, args, ctx),

        PolicyExprKind::DomainBuiltin { domain, name, .. } => {
            // Domain builtins are always residual (evaluated at runtime)
            EvalResult::Residual(format!("{}.{}", domain, name))
        }

        PolicyExprKind::BinaryOp { op, left, right } => {
            let left_result = evaluate(left, ctx);
            let right_result = evaluate(right, ctx);
            eval_binary_op(op, left_result, right_result)
        }

        PolicyExprKind::UnaryOp { op, expr: operand } => {
            let operand_result = evaluate(operand, ctx);
            eval_unary_op(op, operand_result)
        }

        PolicyExprKind::IsNull {
            expr: operand,
            negated,
        } => {
            let operand_result = evaluate(operand, ctx);
            eval_is_null(operand_result, *negated)
        }

        PolicyExprKind::InList {
            expr: operand,
            list,
            negated,
        } => {
            let operand_result = evaluate(operand, ctx);
            let list_results: Vec<EvalResult> = list.iter().map(|e| evaluate(e, ctx)).collect();
            eval_in_list(operand_result, list_results, *negated)
        }

        PolicyExprKind::Subquery { sql } => {
            // Subqueries are always residual
            EvalResult::Residual(format!("({})", sql))
        }
    }
}

fn eval_literal(lit: &PolicyLiteral) -> EvalResult {
    match lit {
        PolicyLiteral::Bool(b) => EvalResult::Const(*b),
        PolicyLiteral::Null => EvalResult::Residual("NULL".to_string()),
        PolicyLiteral::String(s) => EvalResult::Residual(format!("'{}'", s.replace('\'', "''"))),
        PolicyLiteral::Int(n) => EvalResult::Residual(n.to_string()),
        PolicyLiteral::Float(n) => EvalResult::Residual(n.to_string()),
    }
}

fn eval_auth_builtin<C: PolicyEvalContext>(name: &str, args: &[PolicyExpr], ctx: &C) -> EvalResult {
    match name {
        "user_id" => {
            if let Some(user_id) = ctx.user_id() {
                EvalResult::Residual(format!("'{}'", user_id))
            } else {
                EvalResult::Residual("NULL".to_string())
            }
        }

        "org_id" => {
            if let Some(org_id) = ctx.org_id() {
                EvalResult::Residual(format!("'{}'", org_id))
            } else {
                EvalResult::Residual("NULL".to_string())
            }
        }

        "role" => {
            // Role is not directly exposed in context - return NULL
            EvalResult::Residual("NULL".to_string())
        }

        "has_permission" => {
            // auth.has_permission('permission_name') -> true/false
            if let Some(PolicyExprKind::Literal(PolicyLiteral::String(permission))) =
                args.first().map(|e| &e.kind)
            {
                let has_it = ctx.has_permission(permission);
                EvalResult::Const(has_it)
            } else {
                // If argument is not a constant string, cannot evaluate eagerly
                EvalResult::Residual("false".to_string())
            }
        }

        "in_org" => {
            // auth.in_org(org_id_expr) -> compare with active org
            if let Some(arg) = args.first() {
                let arg_result = evaluate(arg, ctx);
                match arg_result {
                    EvalResult::Residual(sql) => {
                        if let Some(org_id) = ctx.org_id() {
                            // Compare the SQL expression with the current org
                            EvalResult::Residual(format!("({} = '{}')", sql, org_id))
                        } else {
                            // No active org, always false
                            EvalResult::Const(false)
                        }
                    }
                    EvalResult::Const(_) => {
                        // Constant arg doesn't make sense for in_org
                        EvalResult::Const(false)
                    }
                }
            } else {
                EvalResult::Const(false)
            }
        }

        "email" => {
            if let Some(email) = ctx.email() {
                EvalResult::Residual(format!("'{}'", email.replace('\'', "''")))
            } else {
                EvalResult::Residual("NULL".to_string())
            }
        }

        "session_id" => {
            if let Some(session_id) = ctx.session_id() {
                EvalResult::Residual(format!("'{}'", session_id))
            } else {
                EvalResult::Residual("NULL".to_string())
            }
        }

        "is_authenticated" => {
            // True if we have a user_id
            EvalResult::Const(ctx.is_authenticated())
        }

        _ => {
            // Unknown builtin - return NULL
            EvalResult::Residual("NULL".to_string())
        }
    }
}

fn eval_binary_op(op: &PolicyBinaryOp, left: EvalResult, right: EvalResult) -> EvalResult {
    use PolicyBinaryOp::*;

    // Short-circuit for AND/OR with constants
    match op {
        And => {
            if left.is_always_false() {
                return EvalResult::Const(false);
            }
            if right.is_always_false() {
                return EvalResult::Const(false);
            }
            if left.is_always_true() && right.is_always_true() {
                return EvalResult::Const(true);
            }
            if left.is_always_true() {
                return right;
            }
            if right.is_always_true() {
                return left;
            }
        }
        Or => {
            if left.is_always_true() {
                return EvalResult::Const(true);
            }
            if right.is_always_true() {
                return EvalResult::Const(true);
            }
            if left.is_always_false() && right.is_always_false() {
                return EvalResult::Const(false);
            }
            if left.is_always_false() {
                return right;
            }
            if right.is_always_false() {
                return left;
            }
        }
        _ => {}
    }

    // Combine as SQL
    let left_sql = match left {
        EvalResult::Const(true) => "TRUE".to_string(),
        EvalResult::Const(false) => "FALSE".to_string(),
        EvalResult::Residual(s) => s,
    };

    let right_sql = match right {
        EvalResult::Const(true) => "TRUE".to_string(),
        EvalResult::Const(false) => "FALSE".to_string(),
        EvalResult::Residual(s) => s,
    };

    EvalResult::Residual(format!("({} {} {})", left_sql, op.as_sql(), right_sql))
}

fn eval_unary_op(op: &PolicyUnaryOp, operand: EvalResult) -> EvalResult {
    match op {
        PolicyUnaryOp::Not => match operand {
            EvalResult::Const(b) => EvalResult::Const(!b),
            EvalResult::Residual(s) => EvalResult::Residual(format!("(NOT {})", s)),
        },
        PolicyUnaryOp::Minus => match operand {
            EvalResult::Const(_) => EvalResult::Residual("-".to_string()), // Can't negate a bool
            EvalResult::Residual(s) => EvalResult::Residual(format!("(-{})", s)),
        },
    }
}

fn eval_is_null(operand: EvalResult, negated: bool) -> EvalResult {
    match operand {
        EvalResult::Residual(s) => {
            if negated {
                EvalResult::Residual(format!("({} IS NOT NULL)", s))
            } else {
                EvalResult::Residual(format!("({} IS NULL)", s))
            }
        }
        EvalResult::Const(_) => {
            // Constants are never null in our model
            EvalResult::Const(negated)
        }
    }
}

fn eval_in_list(operand: EvalResult, list: Vec<EvalResult>, negated: bool) -> EvalResult {
    let operand_sql = match operand {
        EvalResult::Const(true) => "TRUE".to_string(),
        EvalResult::Const(false) => "FALSE".to_string(),
        EvalResult::Residual(s) => s,
    };

    let list_sql: Vec<String> = list
        .into_iter()
        .map(|r| match r {
            EvalResult::Const(true) => "TRUE".to_string(),
            EvalResult::Const(false) => "FALSE".to_string(),
            EvalResult::Residual(s) => s,
        })
        .collect();

    let list_joined = list_sql.join(", ");

    if negated {
        EvalResult::Residual(format!("({} NOT IN ({}))", operand_sql, list_joined))
    } else {
        EvalResult::Residual(format!("({} IN ({}))", operand_sql, list_joined))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::TestPolicyContext;
    use reactor_core::id::{OrgId, UserId};

    fn make_test_ctx(permissions: Vec<String>) -> TestPolicyContext {
        TestPolicyContext::new(UserId::new(), OrgId::new()).with_permissions(permissions)
    }

    #[test]
    fn test_eval_literal_true() {
        let ctx = make_test_ctx(vec![]);
        let expr = PolicyExpr::bool_literal(true);
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(true)));
    }

    #[test]
    fn test_eval_literal_false() {
        let ctx = make_test_ctx(vec![]);
        let expr = PolicyExpr::bool_literal(false);
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(false)));
    }

    #[test]
    fn test_eval_has_permission_true() {
        let ctx = make_test_ctx(vec!["data:todos:read".to_string()]);
        let expr = PolicyExpr::auth_builtin(
            "has_permission",
            vec![PolicyExpr::string_literal("data:todos:read")],
        );
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(true)));
    }

    #[test]
    fn test_eval_has_permission_false() {
        let ctx = make_test_ctx(vec!["data:todos:read".to_string()]);
        let expr = PolicyExpr::auth_builtin(
            "has_permission",
            vec![PolicyExpr::string_literal("data:todos:write")],
        );
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(false)));
    }

    #[test]
    fn test_eval_wildcard_permission() {
        let ctx = make_test_ctx(vec!["*".to_string()]);
        let expr = PolicyExpr::auth_builtin(
            "has_permission",
            vec![PolicyExpr::string_literal("data:anything:anything")],
        );
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(true)));
    }

    #[test]
    fn test_eval_column_residual() {
        let ctx = make_test_ctx(vec![]);
        let expr = PolicyExpr::column("user_id");
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Residual(s) if s == "\"user_id\""));
    }

    #[test]
    fn test_eval_and_short_circuit() {
        let ctx = make_test_ctx(vec![]);
        // false AND anything = false
        let expr = PolicyExpr::and(PolicyExpr::bool_literal(false), PolicyExpr::column("x"));
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(false)));
    }

    #[test]
    fn test_eval_or_short_circuit() {
        let ctx = make_test_ctx(vec![]);
        // true OR anything = true
        let expr = PolicyExpr::or(PolicyExpr::bool_literal(true), PolicyExpr::column("x"));
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(true)));
    }

    #[test]
    fn test_eval_not() {
        let ctx = make_test_ctx(vec![]);
        let expr = PolicyExpr::not(PolicyExpr::bool_literal(true));
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Const(false)));
    }

    #[test]
    fn test_eval_domain_builtin() {
        let ctx = make_test_ctx(vec![]);
        let expr = PolicyExpr::domain_builtin("object", "key", vec![]);
        let result = evaluate(&expr, &ctx);
        assert!(matches!(result, EvalResult::Residual(s) if s == "object.key"));
    }
}
