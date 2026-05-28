//! SQL parser wrapper using sqlparser-rs.
//!
//! Parses standard SQL DDL and the Reactor `policy` extension.

use super::ast::*;
use sqlparser::ast as sp;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use thiserror::Error;

/// Parse error.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("SQL parse error: {0}")]
    Sql(String),
    #[error("unsupported statement: {0}")]
    UnsupportedStatement(String),
    #[error("unsupported type: {0}")]
    UnsupportedType(String),
    #[error("invalid syntax: {0}")]
    InvalidSyntax(String),
}

/// Parse a migration file into Reactor AST.
pub fn parse_migration(sql: &str) -> Result<Migration, ParseError> {
    // First, extract policy statements (Reactor extension)
    let (sql_without_policies, policies) = extract_policies(sql)?;

    // Parse standard SQL
    let dialect = PostgreSqlDialect {};
    let sp_statements = Parser::parse_sql(&dialect, &sql_without_policies)
        .map_err(|e| ParseError::Sql(e.to_string()))?;

    let mut statements = Vec::new();

    for sp_stmt in sp_statements {
        if let Some(stmt) = convert_statement(sp_stmt)? {
            statements.push(stmt);
        }
    }

    // Add policy statements
    statements.extend(policies.into_iter().map(Statement::Policy));

    Ok(Migration { statements })
}

/// Extract policy statements from SQL (they're not standard SQL).
fn extract_policies(sql: &str) -> Result<(String, Vec<PolicyStmt>), ParseError> {
    let mut result_sql = String::new();
    let mut policies = Vec::new();

    let mut chars = sql.chars().peekable();
    let mut in_policy = false;
    let mut policy_text = String::new();

    while let Some(c) = chars.next() {
        if !in_policy {
            // Check for 'policy' keyword at start of statement
            if c.is_alphabetic() {
                let mut word = String::from(c);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if word.to_lowercase() == "policy" {
                    in_policy = true;
                    policy_text = word;
                } else {
                    result_sql.push_str(&word);
                }
            } else {
                result_sql.push(c);
            }
        } else {
            policy_text.push(c);
            if c == ';' {
                // End of policy statement
                let policy = parse_policy_stmt(&policy_text)?;
                policies.push(policy);
                in_policy = false;
                policy_text.clear();
            }
        }
    }

    Ok((result_sql, policies))
}

/// Parse a policy statement.
fn parse_policy_stmt(text: &str) -> Result<PolicyStmt, ParseError> {
    // policy <name> on <table> for <scope,...> using (...) check (...);
    let text = text.trim().trim_end_matches(';').trim();

    let parts: Vec<&str> = text.splitn(2, char::is_whitespace).collect();
    if parts.is_empty() || parts[0].to_lowercase() != "policy" {
        return Err(ParseError::InvalidSyntax("expected 'policy'".to_string()));
    }

    let rest = parts.get(1).unwrap_or(&"").trim();

    // Parse name
    let (name, rest) = parse_identifier(rest)?;

    // Parse 'on'
    let rest = rest.trim();
    if !rest.to_lowercase().starts_with("on ") {
        return Err(ParseError::InvalidSyntax("expected 'on'".to_string()));
    }
    let rest = rest[3..].trim();

    // Parse table
    let (table, rest) = parse_identifier(rest)?;

    // Parse 'for'
    let rest = rest.trim();
    if !rest.to_lowercase().starts_with("for ") {
        return Err(ParseError::InvalidSyntax("expected 'for'".to_string()));
    }
    let rest = rest[4..].trim();

    // Parse scopes
    let (scopes, rest) = parse_scopes(rest)?;

    // Parse optional 'using' and 'check'
    let (using, check, _rest) = parse_using_check(rest)?;

    Ok(PolicyStmt {
        name,
        table,
        scopes,
        using,
        check,
    })
}

fn parse_identifier(s: &str) -> Result<(String, &str), ParseError> {
    let s = s.trim();
    let mut chars = s.chars().peekable();
    let mut ident = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' {
            ident.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    if ident.is_empty() {
        return Err(ParseError::InvalidSyntax("expected identifier".to_string()));
    }

    let rest = &s[ident.len()..];
    Ok((ident, rest))
}

fn parse_scopes(s: &str) -> Result<(Vec<PolicyScope>, &str), ParseError> {
    let mut scopes = Vec::new();
    let mut rest = s.trim();

    loop {
        let (word, after) = parse_identifier(rest)?;
        let scope = match word.to_lowercase().as_str() {
            "select" => PolicyScope::Select,
            "insert" => PolicyScope::Insert,
            "update" => PolicyScope::Update,
            "delete" => PolicyScope::Delete,
            other => {
                return Err(ParseError::InvalidSyntax(format!(
                    "unknown scope: {}",
                    other
                )))
            }
        };
        scopes.push(scope);
        rest = after.trim();

        if rest.starts_with(',') {
            rest = rest[1..].trim();
        } else {
            break;
        }
    }

    Ok((scopes, rest))
}

fn parse_using_check(s: &str) -> Result<(Option<Expr>, Option<Expr>, &str), ParseError> {
    let mut using = None;
    let mut check = None;
    let mut rest = s.trim();

    // Parse 'using (...)'
    if rest.to_lowercase().starts_with("using") {
        rest = rest[5..].trim();
        if !rest.starts_with('(') {
            return Err(ParseError::InvalidSyntax(
                "expected '(' after using".to_string(),
            ));
        }
        let (expr_str, after) = extract_parens(rest)?;
        using = Some(parse_expr(&expr_str)?);
        rest = after.trim();
    }

    // Parse 'check (...)'
    if rest.to_lowercase().starts_with("check") {
        rest = rest[5..].trim();
        if !rest.starts_with('(') {
            return Err(ParseError::InvalidSyntax(
                "expected '(' after check".to_string(),
            ));
        }
        let (expr_str, after) = extract_parens(rest)?;
        check = Some(parse_expr(&expr_str)?);
        rest = after.trim();
    }

    Ok((using, check, rest))
}

fn extract_parens(s: &str) -> Result<(String, &str), ParseError> {
    if !s.starts_with('(') {
        return Err(ParseError::InvalidSyntax("expected '('".to_string()));
    }

    let mut depth = 0;
    let mut end_idx = 0;

    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(ParseError::InvalidSyntax(
            "unmatched parentheses".to_string(),
        ));
    }

    let content = &s[1..end_idx];
    let rest = &s[end_idx + 1..];
    Ok((content.to_string(), rest))
}

fn parse_expr(s: &str) -> Result<Expr, ParseError> {
    // For now, store as raw SQL - full expression parsing comes in PR 6
    Ok(Expr::Raw {
        sql: s.trim().to_string(),
    })
}

/// Convert a sqlparser statement to Reactor AST.
fn convert_statement(stmt: sp::Statement) -> Result<Option<Statement>, ParseError> {
    match stmt {
        sp::Statement::CreateTable(ct) => Ok(Some(convert_create_table(ct)?)),
        sp::Statement::CreateIndex(ci) => Ok(Some(convert_create_index(ci)?)),
        sp::Statement::AlterTable {
            name, operations, ..
        } => convert_alter_table(&name, &operations),
        sp::Statement::Drop {
            object_type,
            if_exists,
            names,
            ..
        } => convert_drop(object_type, if_exists, names),
        sp::Statement::CreateFunction { .. } => {
            // Will be handled in PR 6
            Ok(None)
        }
        _ => Err(ParseError::UnsupportedStatement(format!("{:?}", stmt))),
    }
}

fn convert_create_table(ct: sp::CreateTable) -> Result<Statement, ParseError> {
    let name = ct.name.to_string();
    let mut columns = Vec::new();
    let mut constraints = Vec::new();

    for col in ct.columns {
        columns.push(convert_column_def(col)?);
    }

    for constraint in ct.constraints {
        constraints.push(convert_table_constraint(constraint)?);
    }

    Ok(Statement::CreateTable(CreateTable {
        name,
        columns,
        constraints,
        if_not_exists: ct.if_not_exists,
    }))
}

fn convert_column_def(col: sp::ColumnDef) -> Result<ColumnDef, ParseError> {
    let name = col.name.value.clone();
    let data_type = convert_data_type(&col.data_type)?;
    let mut nullable = true;
    let mut default = None;
    let mut constraints = Vec::new();

    for opt in col.options {
        match opt.option {
            sp::ColumnOption::NotNull => {
                nullable = false;
                constraints.push(ColumnConstraint::NotNull);
            }
            sp::ColumnOption::Null => {
                nullable = true;
            }
            sp::ColumnOption::Default(expr) => {
                default = Some(convert_expr(&expr)?);
            }
            sp::ColumnOption::Unique { is_primary, .. } => {
                if is_primary {
                    constraints.push(ColumnConstraint::PrimaryKey);
                } else {
                    constraints.push(ColumnConstraint::Unique);
                }
            }
            sp::ColumnOption::ForeignKey {
                foreign_table,
                referred_columns,
                on_delete,
                on_update,
                ..
            } => {
                constraints.push(ColumnConstraint::References(ForeignKeyRef {
                    table: foreign_table.to_string(),
                    columns: referred_columns.iter().map(|c| c.value.clone()).collect(),
                    on_delete: on_delete.map(convert_referential_action),
                    on_update: on_update.map(convert_referential_action),
                }));
            }
            sp::ColumnOption::Check(expr) => {
                constraints.push(ColumnConstraint::Check {
                    expr: convert_expr(&expr)?,
                });
            }
            _ => {}
        }
    }

    Ok(ColumnDef {
        name,
        data_type,
        nullable,
        default,
        constraints,
    })
}

fn convert_data_type(dt: &sp::DataType) -> Result<DataType, ParseError> {
    let type_str = format!("{}", dt);
    DataType::from_sql(&type_str).ok_or(ParseError::UnsupportedType(type_str))
}

fn convert_expr(expr: &sp::Expr) -> Result<Expr, ParseError> {
    match expr {
        sp::Expr::Identifier(ident) => Ok(Expr::Column {
            name: ident.value.clone(),
        }),
        sp::Expr::CompoundIdentifier(parts) if parts.len() == 2 => Ok(Expr::QualifiedColumn {
            table: parts[0].value.clone(),
            column: parts[1].value.clone(),
        }),
        sp::Expr::Value(v) => Ok(Expr::Literal(convert_value(v)?)),
        sp::Expr::BinaryOp { left, op, right } => Ok(Expr::BinaryOp {
            left: Box::new(convert_expr(left)?),
            op: convert_binary_op(op)?,
            right: Box::new(convert_expr(right)?),
        }),
        sp::Expr::UnaryOp { op, expr } => Ok(Expr::UnaryOp {
            op: convert_unary_op(op)?,
            expr: Box::new(convert_expr(expr)?),
        }),
        sp::Expr::Function(f) => {
            let args = match &f.args {
                sp::FunctionArguments::List(args) => args
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        sp::FunctionArg::Unnamed(sp::FunctionArgExpr::Expr(e)) => {
                            Some(convert_expr(e))
                        }
                        _ => None,
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => vec![],
            };
            Ok(Expr::FunctionCall {
                name: f.name.to_string(),
                args,
            })
        }
        sp::Expr::IsNull(e) => Ok(Expr::IsNull {
            expr: Box::new(convert_expr(e)?),
            negated: false,
        }),
        sp::Expr::IsNotNull(e) => Ok(Expr::IsNull {
            expr: Box::new(convert_expr(e)?),
            negated: true,
        }),
        sp::Expr::InList {
            expr,
            list,
            negated,
        } => Ok(Expr::InList {
            expr: Box::new(convert_expr(expr)?),
            list: list
                .iter()
                .map(convert_expr)
                .collect::<Result<Vec<_>, _>>()?,
            negated: *negated,
        }),
        _ => Ok(Expr::Raw {
            sql: format!("{}", expr),
        }),
    }
}

fn convert_value(v: &sp::Value) -> Result<Literal, ParseError> {
    match v {
        sp::Value::Null => Ok(Literal::Null),
        sp::Value::Boolean(b) => Ok(Literal::Bool(*b)),
        sp::Value::Number(n, _) => {
            if n.contains('.') {
                Ok(Literal::Float(n.parse().map_err(|_| {
                    ParseError::InvalidSyntax(format!("invalid float: {}", n))
                })?))
            } else {
                Ok(Literal::Int(n.parse().map_err(|_| {
                    ParseError::InvalidSyntax(format!("invalid int: {}", n))
                })?))
            }
        }
        sp::Value::SingleQuotedString(s) | sp::Value::DoubleQuotedString(s) => {
            Ok(Literal::String(s.clone()))
        }
        _ => Err(ParseError::InvalidSyntax(format!(
            "unsupported value: {:?}",
            v
        ))),
    }
}

fn convert_binary_op(op: &sp::BinaryOperator) -> Result<BinaryOperator, ParseError> {
    match op {
        sp::BinaryOperator::Eq => Ok(BinaryOperator::Eq),
        sp::BinaryOperator::NotEq => Ok(BinaryOperator::NotEq),
        sp::BinaryOperator::Lt => Ok(BinaryOperator::Lt),
        sp::BinaryOperator::LtEq => Ok(BinaryOperator::LtEq),
        sp::BinaryOperator::Gt => Ok(BinaryOperator::Gt),
        sp::BinaryOperator::GtEq => Ok(BinaryOperator::GtEq),
        sp::BinaryOperator::And => Ok(BinaryOperator::And),
        sp::BinaryOperator::Or => Ok(BinaryOperator::Or),
        _ => Err(ParseError::InvalidSyntax(format!(
            "unsupported operator: {:?}",
            op
        ))),
    }
}

fn convert_unary_op(op: &sp::UnaryOperator) -> Result<UnaryOperator, ParseError> {
    match op {
        sp::UnaryOperator::Not => Ok(UnaryOperator::Not),
        sp::UnaryOperator::Minus => Ok(UnaryOperator::Minus),
        _ => Err(ParseError::InvalidSyntax(format!(
            "unsupported operator: {:?}",
            op
        ))),
    }
}

fn convert_table_constraint(c: sp::TableConstraint) -> Result<TableConstraint, ParseError> {
    match c {
        sp::TableConstraint::PrimaryKey { name, columns, .. } => Ok(TableConstraint::PrimaryKey {
            name: name.map(|n| n.value),
            columns: columns.iter().map(|c| c.value.clone()).collect(),
        }),
        sp::TableConstraint::Unique { name, columns, .. } => Ok(TableConstraint::Unique {
            name: name.map(|n| n.value),
            columns: columns.iter().map(|c| c.value.clone()).collect(),
        }),
        sp::TableConstraint::ForeignKey {
            name,
            columns,
            foreign_table,
            referred_columns,
            on_delete,
            on_update,
            ..
        } => Ok(TableConstraint::ForeignKey {
            name: name.map(|n| n.value),
            columns: columns.iter().map(|c| c.value.clone()).collect(),
            references: ForeignKeyRef {
                table: foreign_table.to_string(),
                columns: referred_columns.iter().map(|c| c.value.clone()).collect(),
                on_delete: on_delete.map(convert_referential_action),
                on_update: on_update.map(convert_referential_action),
            },
        }),
        sp::TableConstraint::Check { name, expr } => Ok(TableConstraint::Check {
            name: name.map(|n| n.value),
            expr: convert_expr(&expr)?,
        }),
        _ => Err(ParseError::InvalidSyntax(
            "unsupported constraint".to_string(),
        )),
    }
}

fn convert_referential_action(action: sp::ReferentialAction) -> ReferentialAction {
    match action {
        sp::ReferentialAction::Cascade => ReferentialAction::Cascade,
        sp::ReferentialAction::SetNull => ReferentialAction::SetNull,
        sp::ReferentialAction::SetDefault => ReferentialAction::SetDefault,
        sp::ReferentialAction::Restrict => ReferentialAction::Restrict,
        sp::ReferentialAction::NoAction => ReferentialAction::NoAction,
    }
}

fn convert_create_index(ci: sp::CreateIndex) -> Result<Statement, ParseError> {
    let name = ci.name.map(|n| n.to_string()).unwrap_or_default();
    let table = ci.table_name.to_string();

    let columns = ci
        .columns
        .iter()
        .map(|col| IndexColumn {
            name: col.expr.to_string(),
            order: col
                .asc
                .map(|asc| if asc { SortOrder::Asc } else { SortOrder::Desc }),
            nulls: col.nulls_first.map(|first| {
                if first {
                    NullsOrder::First
                } else {
                    NullsOrder::Last
                }
            }),
        })
        .collect();

    let where_clause = ci.predicate.map(|e| convert_expr(&e)).transpose()?;

    Ok(Statement::CreateIndex(CreateIndex {
        name,
        table,
        columns,
        unique: ci.unique,
        if_not_exists: ci.if_not_exists,
        where_clause,
    }))
}

fn convert_alter_table(
    name: &sp::ObjectName,
    operations: &[sp::AlterTableOperation],
) -> Result<Option<Statement>, ParseError> {
    let table = name.to_string();

    for op in operations {
        match op {
            sp::AlterTableOperation::AddColumn { column_def, .. } => {
                return Ok(Some(Statement::AlterTableAddColumn(AlterTableAddColumn {
                    table,
                    column: convert_column_def(column_def.clone())?,
                })));
            }
            sp::AlterTableOperation::DropColumn {
                column_name,
                if_exists,
                ..
            } => {
                return Ok(Some(Statement::AlterTableDropColumn(
                    AlterTableDropColumn {
                        table,
                        column: column_name.value.clone(),
                        if_exists: *if_exists,
                    },
                )));
            }
            sp::AlterTableOperation::RenameTable { table_name } => {
                return Ok(Some(Statement::AlterTableRename(AlterTableRename {
                    old_name: table,
                    new_name: table_name.to_string(),
                })));
            }
            sp::AlterTableOperation::RenameColumn {
                old_column_name,
                new_column_name,
            } => {
                return Ok(Some(Statement::AlterColumnRename(AlterColumnRename {
                    table,
                    old_name: old_column_name.value.clone(),
                    new_name: new_column_name.value.clone(),
                })));
            }
            _ => {}
        }
    }

    Err(ParseError::UnsupportedStatement(format!(
        "ALTER TABLE {:?}",
        operations
    )))
}

fn convert_drop(
    object_type: sp::ObjectType,
    if_exists: bool,
    names: Vec<sp::ObjectName>,
) -> Result<Option<Statement>, ParseError> {
    if names.is_empty() {
        return Ok(None);
    }

    let name = names[0].to_string();

    match object_type {
        sp::ObjectType::Table => Ok(Some(Statement::DropTable(DropTable { name, if_exists }))),
        sp::ObjectType::Index => Ok(Some(Statement::DropIndex(DropIndex { name, if_exists }))),
        _ => Err(ParseError::UnsupportedStatement(format!(
            "DROP {:?}",
            object_type
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_table() {
        let sql = r#"
            CREATE TABLE todos (
                id reactor_id PRIMARY KEY,
                title TEXT NOT NULL,
                done BOOLEAN DEFAULT false
            );
        "#;

        let migration = parse_migration(sql).unwrap();
        assert_eq!(migration.statements.len(), 1);

        if let Statement::CreateTable(ct) = &migration.statements[0] {
            assert_eq!(ct.name, "todos");
            assert_eq!(ct.columns.len(), 3);
        } else {
            panic!("expected CreateTable");
        }
    }

    #[test]
    fn test_parse_policy() {
        let sql = r#"
            policy todos_tenant on todos for select, update using (org_id = auth.org_id());
        "#;

        let migration = parse_migration(sql).unwrap();
        assert_eq!(migration.statements.len(), 1);

        if let Statement::Policy(p) = &migration.statements[0] {
            assert_eq!(p.name, "todos_tenant");
            assert_eq!(p.table, "todos");
            assert_eq!(p.scopes.len(), 2);
            assert!(p.using.is_some());
            assert!(p.check.is_none());
        } else {
            panic!("expected Policy");
        }
    }
}
