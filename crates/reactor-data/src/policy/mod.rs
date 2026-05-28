//! Policy DSL for reactor-data.
//!
//! This module re-exports the shared policy engine from `reactor-policy`
//! and provides reactor-data specific functionality like database storage
//! and scope-based compilation.

mod compile;
mod store;

// Re-export core policy types from reactor-policy
pub use reactor_policy::{
    AuthBuiltin, BuiltinError, EvalResult, PolicyBinaryOp, PolicyDecision, PolicyEvalContext,
    PolicyExpr, PolicyExprKind, PolicyLiteral, PolicyParseError, PolicyScope, PolicyUnaryOp,
    compile_check_policies, compile_policies, compile_using_policies, combine_decisions, evaluate,
    parse_policy_expr, validate_builtin_call,
};

// Re-export compile types with an alias for backward compatibility
pub use reactor_policy::CompiledPolicy;

// Export data-specific compilation functions
pub use compile::{compile_for_scope, check_row, check_rows_batch, BatchCheckResult};

// Export the data-specific store
pub use store::{PolicyStore, PolicyStoreError, StoredPolicy};
