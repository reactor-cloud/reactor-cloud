//! Portable SQL dialect for Reactor.
//!
//! This module provides:
//! - A Reactor-specific SQL AST that represents the portable subset
//! - A parser wrapper around sqlparser-rs
//! - A lint pass that rejects forbidden constructs
//! - Type mappings (reactor_id <-> backend types)
//! - Postgres DDL emitter

mod ast;
mod emit_postgres;
mod lint;
mod parser;
mod types;

pub use ast::*;
pub use emit_postgres::emit_postgres;
pub use lint::{lint_statement, LintError};
pub use parser::{parse_migration, ParseError};
pub use types::{ReactorType, TypeMapping};
