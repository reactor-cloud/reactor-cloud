//! Lint pass for Reactor SQL dialect.
//!
//! Rejects forbidden constructs that don't translate across backends.

use super::ast::*;
use thiserror::Error;

/// Lint error with source location.
#[derive(Debug, Error)]
pub enum LintError {
    #[error("unsupported type '{0}': use reactor_id, text, bool, int, bigint, float, timestamptz, jsonb, or bytea")]
    UnsupportedType(String),

    #[error("forbidden construct: {0}")]
    ForbiddenConstruct(String),

    #[error("cross-schema reference not allowed: {0}")]
    CrossSchemaReference(String),

    #[error("native CREATE POLICY not allowed; use Reactor's 'policy ... on ...' syntax")]
    NativePolicy,

    #[error("MATERIALIZED VIEW not supported (no SQLite equivalent)")]
    MaterializedView,

    #[error("PL/pgSQL not supported; use LANGUAGE sql only")]
    PlPgSql,

    #[error("CREATE EXTENSION not allowed; Reactor manages extensions")]
    Extension,

    #[error("WITH RECURSIVE not supported in v0")]
    WithRecursive,
}

/// Lint a single statement.
pub fn lint_statement(stmt: &Statement) -> Result<(), LintError> {
    match stmt {
        Statement::CreateTable(ct) => lint_create_table(ct),
        Statement::CreateIndex(ci) => lint_create_index(ci),
        Statement::AlterTableAddColumn(a) => lint_column_def(&a.column),
        Statement::CreateFunction(f) => lint_function(f),
        Statement::Policy(p) => lint_policy(p),
        // Other statements are OK
        _ => Ok(()),
    }
}

fn lint_create_table(ct: &CreateTable) -> Result<(), LintError> {
    // Check table name for schema prefix
    if ct.name.contains('.') {
        let parts: Vec<&str> = ct.name.split('.').collect();
        if parts.len() == 2 && parts[0] != "public" {
            return Err(LintError::CrossSchemaReference(ct.name.clone()));
        }
    }

    // Check columns
    for col in &ct.columns {
        lint_column_def(col)?;
    }

    Ok(())
}

fn lint_column_def(col: &ColumnDef) -> Result<(), LintError> {
    // Type is already validated during parsing
    // Just check default expressions
    if let Some(default) = &col.default {
        lint_expr(default)?;
    }

    // Check constraints
    for constraint in &col.constraints {
        if let ColumnConstraint::Check { expr } = constraint {
            lint_expr(expr)?;
        }
    }

    Ok(())
}

fn lint_create_index(ci: &CreateIndex) -> Result<(), LintError> {
    // Check WHERE clause
    if let Some(where_clause) = &ci.where_clause {
        lint_expr(where_clause)?;
    }
    Ok(())
}

fn lint_function(f: &CreateFunction) -> Result<(), LintError> {
    // Check function body for forbidden constructs
    let body_lower = f.body.to_lowercase();

    if body_lower.contains("with recursive") {
        return Err(LintError::WithRecursive);
    }

    // Check parameter types
    for param in &f.params {
        lint_data_type(&param.data_type)?;
    }

    // Check return type
    match &f.returns {
        FunctionReturn::Scalar { data_type } => lint_data_type(data_type)?,
        FunctionReturn::SetOf { data_type } => lint_data_type(data_type)?,
        FunctionReturn::Table { columns } => {
            for (_, dt) in columns {
                lint_data_type(dt)?;
            }
        }
        FunctionReturn::Void => {}
    }

    Ok(())
}

fn lint_data_type(dt: &DataType) -> Result<(), LintError> {
    // All DataType variants are valid (they wouldn't have parsed otherwise)
    let _ = dt;
    Ok(())
}

fn lint_policy(p: &PolicyStmt) -> Result<(), LintError> {
    // Check expressions
    if let Some(using) = &p.using {
        lint_expr(using)?;
    }
    if let Some(check) = &p.check {
        lint_expr(check)?;
    }
    Ok(())
}

fn lint_expr(expr: &Expr) -> Result<(), LintError> {
    match expr {
        Expr::BinaryOp { left, right, .. } => {
            lint_expr(left)?;
            lint_expr(right)?;
        }
        Expr::UnaryOp { expr, .. } => {
            lint_expr(expr)?;
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                lint_expr(arg)?;
            }
        }
        Expr::IsNull { expr, .. } => {
            lint_expr(expr)?;
        }
        Expr::InList { expr, list, .. } => {
            lint_expr(expr)?;
            for item in list {
                lint_expr(item)?;
            }
        }
        Expr::Subquery { sql } => {
            lint_raw_sql(sql)?;
        }
        Expr::Raw { sql } => {
            lint_raw_sql(sql)?;
        }
        _ => {}
    }
    Ok(())
}

fn lint_raw_sql(sql: &str) -> Result<(), LintError> {
    let lower = sql.to_lowercase();

    if lower.contains("with recursive") {
        return Err(LintError::WithRecursive);
    }

    // Check for Postgres-only operators that don't work in SQLite
    // (These are OK in v0 since we only target Postgres, but we warn)
    // For now, just pass through

    Ok(())
}

/// Check raw SQL for forbidden top-level constructs.
pub fn lint_raw_sql_statement(sql: &str) -> Result<(), LintError> {
    let lower = sql.to_lowercase().trim().to_string();

    if lower.starts_with("create policy") {
        return Err(LintError::NativePolicy);
    }

    if lower.starts_with("create materialized view") {
        return Err(LintError::MaterializedView);
    }

    if lower.starts_with("create extension") {
        return Err(LintError::Extension);
    }

    if lower.contains("language plpgsql") || lower.contains("language 'plpgsql'") {
        return Err(LintError::PlPgSql);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_valid_table() {
        let ct = CreateTable {
            name: "todos".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::ReactorId,
                    nullable: false,
                    default: None,
                    constraints: vec![ColumnConstraint::PrimaryKey],
                },
                ColumnDef {
                    name: "title".to_string(),
                    data_type: DataType::Text,
                    nullable: false,
                    default: None,
                    constraints: vec![],
                },
            ],
            constraints: vec![],
            if_not_exists: false,
        };

        assert!(lint_statement(&Statement::CreateTable(ct)).is_ok());
    }

    #[test]
    fn test_lint_cross_schema() {
        let ct = CreateTable {
            name: "other_schema.todos".to_string(),
            columns: vec![],
            constraints: vec![],
            if_not_exists: false,
        };

        assert!(matches!(
            lint_statement(&Statement::CreateTable(ct)),
            Err(LintError::CrossSchemaReference(_))
        ));
    }
}
