//! Storage policy evaluation.
//!
//! Extends the shared reactor-policy engine with storage-specific
//! domain builtins for object-level access control.

mod eval;

pub use eval::{check_object_access, ObjectFacts};
