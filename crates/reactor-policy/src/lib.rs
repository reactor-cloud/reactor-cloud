//! Reactor Policy DSL.
//!
//! This crate provides the shared policy expression language used by
//! reactor-data and reactor-storage for row/object-level access control.
//!
//! Policies are boolean expressions that can reference:
//! - Column/field values
//! - Auth context via `auth.*` builtins
//! - Literal values
//! - Comparison and logical operators
//!
//! The policy engine can evaluate expressions eagerly against an auth context
//! (producing constant true/false) or produce residual SQL/expressions for
//! runtime evaluation.

mod ast;
mod builtins;
mod compile;
mod context;
mod eval;
mod grammar;
mod scope;

pub use ast::{PolicyBinaryOp, PolicyExpr, PolicyExprKind, PolicyLiteral, PolicyUnaryOp};
pub use builtins::{validate_builtin_call, AuthBuiltin, BuiltinError};
pub use compile::{
    compile_check_policies, compile_policies, compile_using_policies, combine_decisions,
    CompiledPolicy, PolicyDecision,
};
pub use context::PolicyEvalContext;
pub use eval::{evaluate, EvalResult};
pub use grammar::{parse_policy_expr, PolicyParseError};
pub use scope::PolicyScope;
