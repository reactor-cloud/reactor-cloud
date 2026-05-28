//! Connect-side policy evaluation for conflict resolution.
//!
//! Provides domain-specific builtins for connect policy expressions:
//! - `connect.source_a.*` - Source A record fields
//! - `connect.source_b.*` - Source B record fields
//! - `connect.stream` - Stream name
//! - `connect.field` - Field name in conflict

mod eval;

pub use eval::{
    evaluate_conflict_policy, ConflictEvalContext, ConflictEvalResult, ConflictFacts,
};
