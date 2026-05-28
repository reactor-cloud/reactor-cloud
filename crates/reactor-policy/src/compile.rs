//! Policy compilation for scope-based access control.
//!
//! Compiles policy expressions into SQL fragments or constant decisions.

use crate::ast::PolicyExpr;
use crate::context::PolicyEvalContext;
use crate::eval::{evaluate, EvalResult};

/// Result of compiling policies for a scope.
#[derive(Debug, Clone)]
pub enum PolicyDecision {
    /// Always allow access (no policies or all evaluate to true).
    AlwaysAllow,
    /// Always deny access (at least one policy evaluates to false).
    AlwaysDeny { reason: String },
    /// Conditional access (add SQL predicate to WHERE clause).
    Conditional { sql_fragment: String },
}

impl PolicyDecision {
    /// Check if this decision allows access unconditionally.
    pub fn allows(&self) -> bool {
        matches!(self, PolicyDecision::AlwaysAllow)
    }

    /// Check if this decision denies access unconditionally.
    pub fn denies(&self) -> bool {
        matches!(self, PolicyDecision::AlwaysDeny { .. })
    }

    /// Get the SQL fragment if conditional.
    pub fn sql_fragment(&self) -> Option<&str> {
        match self {
            PolicyDecision::Conditional { sql_fragment } => Some(sql_fragment),
            _ => None,
        }
    }
}

/// A stored policy with its expression and metadata.
#[derive(Debug, Clone)]
pub struct CompiledPolicy {
    pub name: String,
    pub using_expr: Option<PolicyExpr>,
    pub check_expr: Option<PolicyExpr>,
}

impl CompiledPolicy {
    /// Create a new compiled policy.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            using_expr: None,
            check_expr: None,
        }
    }

    /// Set the USING expression.
    pub fn with_using(mut self, expr: PolicyExpr) -> Self {
        self.using_expr = Some(expr);
        self
    }

    /// Set the CHECK expression.
    pub fn with_check(mut self, expr: PolicyExpr) -> Self {
        self.check_expr = Some(expr);
        self
    }
}

/// Compile multiple policies, producing a single decision.
///
/// # Arguments
///
/// * `policies` - The policies to compile
/// * `ctx` - Authentication context
/// * `has_superuser_bypass` - Whether superuser (*) permission should bypass all policies
///
/// # Returns
///
/// A `PolicyDecision` indicating whether to allow, deny, or apply conditions.
pub fn compile_policies<C: PolicyEvalContext>(
    policies: &[CompiledPolicy],
    ctx: &C,
    has_superuser_bypass: bool,
) -> PolicyDecision {
    // Check for superuser bypass (* permission)
    if has_superuser_bypass && ctx.has_permission("*") {
        tracing::debug!("superuser bypass - skipping policy evaluation");
        return PolicyDecision::AlwaysAllow;
    }

    if policies.is_empty() {
        // No policies defined - allow by default
        return PolicyDecision::AlwaysAllow;
    }

    // Separate USING and CHECK policies
    let using_policies: Vec<&CompiledPolicy> =
        policies.iter().filter(|p| p.using_expr.is_some()).collect();
    let check_policies: Vec<&CompiledPolicy> =
        policies.iter().filter(|p| p.check_expr.is_some()).collect();

    // Compile USING policies (combined with OR)
    let using_decision = if using_policies.is_empty() {
        PolicyDecision::AlwaysAllow
    } else {
        compile_using_policies(&using_policies, ctx)
    };

    // Compile CHECK policies (combined with AND)
    let check_decision = if check_policies.is_empty() {
        PolicyDecision::AlwaysAllow
    } else {
        compile_check_policies(&check_policies, ctx)
    };

    // Combine decisions
    combine_decisions(using_decision, check_decision)
}

/// Compile USING policies (OR semantics: at least one must match).
pub fn compile_using_policies<C: PolicyEvalContext>(
    policies: &[&CompiledPolicy],
    ctx: &C,
) -> PolicyDecision {
    let mut residuals: Vec<String> = Vec::new();
    let mut any_always_allow = false;

    for policy in policies {
        if let Some(ref using_expr) = policy.using_expr {
            let result = evaluate(using_expr, ctx);
            match result {
                EvalResult::Const(true) => {
                    // One policy always allows - entire OR is true
                    any_always_allow = true;
                    break;
                }
                EvalResult::Const(false) => {
                    // This policy never matches, skip it
                    continue;
                }
                EvalResult::Residual(sql) => {
                    residuals.push(sql);
                }
            }
        }
    }

    if any_always_allow {
        return PolicyDecision::AlwaysAllow;
    }

    if residuals.is_empty() {
        // All policies evaluated to false
        return PolicyDecision::AlwaysDeny {
            reason: "no matching policy".to_string(),
        };
    }

    // Combine residuals with OR
    let combined = if residuals.len() == 1 {
        residuals.pop().unwrap()
    } else {
        format!("({})", residuals.join(" OR "))
    };

    PolicyDecision::Conditional {
        sql_fragment: combined,
    }
}

/// Compile CHECK policies (AND semantics: all must match).
pub fn compile_check_policies<C: PolicyEvalContext>(
    policies: &[&CompiledPolicy],
    ctx: &C,
) -> PolicyDecision {
    let mut residuals: Vec<String> = Vec::new();

    for policy in policies {
        if let Some(ref check_expr) = policy.check_expr {
            let result = evaluate(check_expr, ctx);
            match result {
                EvalResult::Const(true) => {
                    // This check always passes, continue
                    continue;
                }
                EvalResult::Const(false) => {
                    // This check always fails - entire AND is false
                    return PolicyDecision::AlwaysDeny {
                        reason: format!("check policy '{}' failed", policy.name),
                    };
                }
                EvalResult::Residual(sql) => {
                    residuals.push(sql);
                }
            }
        }
    }

    if residuals.is_empty() {
        // All checks passed
        return PolicyDecision::AlwaysAllow;
    }

    // Combine residuals with AND
    let combined = if residuals.len() == 1 {
        residuals.pop().unwrap()
    } else {
        format!("({})", residuals.join(" AND "))
    };

    PolicyDecision::Conditional {
        sql_fragment: combined,
    }
}

/// Combine USING and CHECK decisions.
pub fn combine_decisions(using: PolicyDecision, check: PolicyDecision) -> PolicyDecision {
    match (&using, &check) {
        // Either deny = deny
        (PolicyDecision::AlwaysDeny { reason }, _) => PolicyDecision::AlwaysDeny {
            reason: reason.clone(),
        },
        (_, PolicyDecision::AlwaysDeny { reason }) => PolicyDecision::AlwaysDeny {
            reason: reason.clone(),
        },

        // Both allow = allow
        (PolicyDecision::AlwaysAllow, PolicyDecision::AlwaysAllow) => PolicyDecision::AlwaysAllow,

        // One conditional, one allow = conditional
        (PolicyDecision::AlwaysAllow, PolicyDecision::Conditional { sql_fragment }) => {
            PolicyDecision::Conditional {
                sql_fragment: sql_fragment.clone(),
            }
        }
        (PolicyDecision::Conditional { sql_fragment }, PolicyDecision::AlwaysAllow) => {
            PolicyDecision::Conditional {
                sql_fragment: sql_fragment.clone(),
            }
        }

        // Both conditional = combine with AND
        (
            PolicyDecision::Conditional {
                sql_fragment: using_sql,
            },
            PolicyDecision::Conditional {
                sql_fragment: check_sql,
            },
        ) => PolicyDecision::Conditional {
            sql_fragment: format!("({} AND {})", using_sql, check_sql),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::PolicyBinaryOp;
    use crate::context::TestPolicyContext;
    use reactor_core::id::{OrgId, UserId};

    fn make_test_ctx(permissions: Vec<String>) -> TestPolicyContext {
        TestPolicyContext::new(UserId::new(), OrgId::new()).with_permissions(permissions)
    }

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

    #[test]
    fn test_combine_both_allow() {
        let result = combine_decisions(PolicyDecision::AlwaysAllow, PolicyDecision::AlwaysAllow);
        assert!(matches!(result, PolicyDecision::AlwaysAllow));
    }

    #[test]
    fn test_combine_one_deny() {
        let result = combine_decisions(
            PolicyDecision::AlwaysAllow,
            PolicyDecision::AlwaysDeny {
                reason: "test".to_string(),
            },
        );
        assert!(matches!(result, PolicyDecision::AlwaysDeny { .. }));
    }

    #[test]
    fn test_combine_conditional() {
        let result = combine_decisions(
            PolicyDecision::Conditional {
                sql_fragment: "a = 1".to_string(),
            },
            PolicyDecision::AlwaysAllow,
        );
        assert!(
            matches!(result, PolicyDecision::Conditional { sql_fragment } if sql_fragment == "a = 1")
        );
    }

    #[test]
    fn test_combine_both_conditional() {
        let result = combine_decisions(
            PolicyDecision::Conditional {
                sql_fragment: "a = 1".to_string(),
            },
            PolicyDecision::Conditional {
                sql_fragment: "b = 2".to_string(),
            },
        );
        assert!(
            matches!(result, PolicyDecision::Conditional { sql_fragment } if sql_fragment == "(a = 1 AND b = 2)")
        );
    }

    #[test]
    fn test_compile_empty_policies() {
        let ctx = make_test_ctx(vec![]);
        let policies: Vec<CompiledPolicy> = vec![];
        let result = compile_policies(&policies, &ctx, true);
        assert!(matches!(result, PolicyDecision::AlwaysAllow));
    }

    #[test]
    fn test_compile_superuser_bypass() {
        let ctx = make_test_ctx(vec!["*".to_string()]);
        let policies = vec![CompiledPolicy::new("test")
            .with_using(PolicyExpr::bool_literal(false))];
        let result = compile_policies(&policies, &ctx, true);
        assert!(matches!(result, PolicyDecision::AlwaysAllow));
    }

    #[test]
    fn test_compile_using_policy_constant_true() {
        let ctx = make_test_ctx(vec!["test:read".to_string()]);
        let policies = vec![CompiledPolicy::new("test").with_using(PolicyExpr::auth_builtin(
            "has_permission",
            vec![PolicyExpr::string_literal("test:read")],
        ))];
        let result = compile_policies(&policies, &ctx, true);
        assert!(matches!(result, PolicyDecision::AlwaysAllow));
    }

    #[test]
    fn test_compile_using_policy_constant_false() {
        let ctx = make_test_ctx(vec![]);
        let policies = vec![CompiledPolicy::new("test").with_using(PolicyExpr::auth_builtin(
            "has_permission",
            vec![PolicyExpr::string_literal("test:read")],
        ))];
        let result = compile_policies(&policies, &ctx, false);
        assert!(matches!(result, PolicyDecision::AlwaysDeny { .. }));
    }

    #[test]
    fn test_compile_using_policy_residual() {
        let ctx = make_test_ctx(vec![]);
        let policies = vec![CompiledPolicy::new("test").with_using(PolicyExpr::binary(
            PolicyExpr::column("org_id"),
            PolicyBinaryOp::Eq,
            PolicyExpr::auth_builtin("org_id", vec![]),
        ))];
        let result = compile_policies(&policies, &ctx, false);
        assert!(matches!(result, PolicyDecision::Conditional { .. }));
    }
}
