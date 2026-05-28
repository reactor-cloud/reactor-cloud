//! Postgres DDL emitter.
//!
//! Converts Reactor AST to native Postgres DDL.

use super::ast::*;
use super::types::TypeMapping;

/// Emit a complete migration as Postgres DDL.
pub fn emit_postgres(migration: &Migration) -> String {
    let mut output = String::new();

    for stmt in &migration.statements {
        output.push_str(&emit_statement(stmt));
        output.push_str(";\n\n");
    }

    output
}

fn emit_statement(stmt: &Statement) -> String {
    match stmt {
        Statement::CreateTable(ct) => emit_create_table(ct),
        Statement::CreateIndex(ci) => emit_create_index(ci),
        Statement::AlterTableAddColumn(a) => emit_alter_add_column(a),
        Statement::AlterTableDropColumn(a) => emit_alter_drop_column(a),
        Statement::AlterTableRename(a) => emit_alter_table_rename(a),
        Statement::AlterColumnRename(a) => emit_alter_column_rename(a),
        Statement::DropTable(d) => emit_drop_table(d),
        Statement::DropIndex(d) => emit_drop_index(d),
        Statement::CreateFunction(f) => emit_create_function(f),
        Statement::Policy(_) => {
            // Policies are stored as metadata, not as native DDL
            "-- Policy stored in _reactor_data.policies".to_string()
        }
    }
}

fn emit_create_table(ct: &CreateTable) -> String {
    let mut sql = String::new();

    sql.push_str("CREATE TABLE ");
    if ct.if_not_exists {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&ct.name);
    sql.push_str(" (\n");

    // Columns
    let col_defs: Vec<String> = ct.columns.iter().map(emit_column_def).collect();
    sql.push_str(&col_defs.join(",\n"));

    // Table constraints
    if !ct.constraints.is_empty() {
        sql.push_str(",\n");
        let constraints: Vec<String> = ct.constraints.iter().map(emit_table_constraint).collect();
        sql.push_str(&constraints.join(",\n"));
    }

    sql.push_str("\n)");
    sql
}

fn emit_column_def(col: &ColumnDef) -> String {
    let mapping = TypeMapping::POSTGRES;
    let mut parts = vec![
        format!("    {}", col.name),
        mapping.to_sql(&col.data_type).to_string(),
    ];

    // Add reactor_id check constraint for UUIDv7
    if matches!(col.data_type, DataType::ReactorId) {
        // UUIDv7 has version nibble = 7
    }

    if !col.nullable {
        parts.push("NOT NULL".to_string());
    }

    if let Some(default) = &col.default {
        parts.push(format!("DEFAULT {}", emit_expr(default)));
    }

    // Column constraints
    for constraint in &col.constraints {
        match constraint {
            ColumnConstraint::PrimaryKey => parts.push("PRIMARY KEY".to_string()),
            ColumnConstraint::Unique => parts.push("UNIQUE".to_string()),
            ColumnConstraint::NotNull => {} // Already handled above
            ColumnConstraint::Check { expr } => {
                parts.push(format!("CHECK ({})", emit_expr(expr)));
            }
            ColumnConstraint::References(fk) => {
                let mut fk_sql = format!("REFERENCES {}", fk.table);
                if !fk.columns.is_empty() {
                    fk_sql.push_str(&format!(" ({})", fk.columns.join(", ")));
                }
                if let Some(action) = &fk.on_delete {
                    fk_sql.push_str(&format!(" ON DELETE {}", emit_referential_action(action)));
                }
                if let Some(action) = &fk.on_update {
                    fk_sql.push_str(&format!(" ON UPDATE {}", emit_referential_action(action)));
                }
                parts.push(fk_sql);
            }
        }
    }

    parts.join(" ")
}

fn emit_table_constraint(c: &TableConstraint) -> String {
    match c {
        TableConstraint::PrimaryKey { name, columns } => {
            let mut sql = String::from("    ");
            if let Some(n) = name {
                sql.push_str(&format!("CONSTRAINT {} ", n));
            }
            sql.push_str(&format!("PRIMARY KEY ({})", columns.join(", ")));
            sql
        }
        TableConstraint::Unique { name, columns } => {
            let mut sql = String::from("    ");
            if let Some(n) = name {
                sql.push_str(&format!("CONSTRAINT {} ", n));
            }
            sql.push_str(&format!("UNIQUE ({})", columns.join(", ")));
            sql
        }
        TableConstraint::ForeignKey {
            name,
            columns,
            references,
        } => {
            let mut sql = String::from("    ");
            if let Some(n) = name {
                sql.push_str(&format!("CONSTRAINT {} ", n));
            }
            sql.push_str(&format!(
                "FOREIGN KEY ({}) REFERENCES {} ({})",
                columns.join(", "),
                references.table,
                references.columns.join(", ")
            ));
            if let Some(action) = &references.on_delete {
                sql.push_str(&format!(" ON DELETE {}", emit_referential_action(action)));
            }
            if let Some(action) = &references.on_update {
                sql.push_str(&format!(" ON UPDATE {}", emit_referential_action(action)));
            }
            sql
        }
        TableConstraint::Check { name, expr } => {
            let mut sql = String::from("    ");
            if let Some(n) = name {
                sql.push_str(&format!("CONSTRAINT {} ", n));
            }
            sql.push_str(&format!("CHECK ({})", emit_expr(expr)));
            sql
        }
    }
}

fn emit_referential_action(action: &ReferentialAction) -> &'static str {
    match action {
        ReferentialAction::Cascade => "CASCADE",
        ReferentialAction::SetNull => "SET NULL",
        ReferentialAction::SetDefault => "SET DEFAULT",
        ReferentialAction::Restrict => "RESTRICT",
        ReferentialAction::NoAction => "NO ACTION",
    }
}

fn emit_create_index(ci: &CreateIndex) -> String {
    let mut sql = String::from("CREATE ");
    if ci.unique {
        sql.push_str("UNIQUE ");
    }
    sql.push_str("INDEX ");
    if ci.if_not_exists {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&ci.name);
    sql.push_str(&format!(" ON {}", ci.table));

    let cols: Vec<String> = ci
        .columns
        .iter()
        .map(|c| {
            let mut col = c.name.clone();
            if let Some(order) = &c.order {
                col.push_str(match order {
                    SortOrder::Asc => " ASC",
                    SortOrder::Desc => " DESC",
                });
            }
            if let Some(nulls) = &c.nulls {
                col.push_str(match nulls {
                    NullsOrder::First => " NULLS FIRST",
                    NullsOrder::Last => " NULLS LAST",
                });
            }
            col
        })
        .collect();

    sql.push_str(&format!(" ({})", cols.join(", ")));

    if let Some(where_clause) = &ci.where_clause {
        sql.push_str(&format!(" WHERE {}", emit_expr(where_clause)));
    }

    sql
}

fn emit_alter_add_column(a: &AlterTableAddColumn) -> String {
    format!(
        "ALTER TABLE {} ADD COLUMN {}",
        a.table,
        emit_column_def(&a.column).trim()
    )
}

fn emit_alter_drop_column(a: &AlterTableDropColumn) -> String {
    let mut sql = format!("ALTER TABLE {} DROP COLUMN ", a.table);
    if a.if_exists {
        sql.push_str("IF EXISTS ");
    }
    sql.push_str(&a.column);
    sql
}

fn emit_alter_table_rename(a: &AlterTableRename) -> String {
    format!("ALTER TABLE {} RENAME TO {}", a.old_name, a.new_name)
}

fn emit_alter_column_rename(a: &AlterColumnRename) -> String {
    format!(
        "ALTER TABLE {} RENAME COLUMN {} TO {}",
        a.table, a.old_name, a.new_name
    )
}

fn emit_drop_table(d: &DropTable) -> String {
    let mut sql = String::from("DROP TABLE ");
    if d.if_exists {
        sql.push_str("IF EXISTS ");
    }
    sql.push_str(&d.name);
    sql
}

fn emit_drop_index(d: &DropIndex) -> String {
    let mut sql = String::from("DROP INDEX ");
    if d.if_exists {
        sql.push_str("IF EXISTS ");
    }
    sql.push_str(&d.name);
    sql
}

fn emit_create_function(f: &CreateFunction) -> String {
    let mapping = TypeMapping::POSTGRES;

    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let mut param = format!("{} {}", p.name, mapping.to_sql(&p.data_type));
            if let Some(default) = &p.default {
                param.push_str(&format!(" DEFAULT {}", emit_expr(default)));
            }
            param
        })
        .collect();

    let returns = match &f.returns {
        FunctionReturn::Scalar { data_type } => mapping.to_sql(data_type).to_string(),
        FunctionReturn::SetOf { data_type } => {
            format!("SETOF {}", mapping.to_sql(data_type))
        }
        FunctionReturn::Table { columns } => {
            let cols: Vec<String> = columns
                .iter()
                .map(|(name, dt)| format!("{} {}", name, mapping.to_sql(dt)))
                .collect();
            format!("TABLE ({})", cols.join(", "))
        }
        FunctionReturn::Void => "VOID".to_string(),
    };

    let volatility = match f.volatility {
        Volatility::Volatile => "",
        Volatility::Stable => " STABLE",
        Volatility::Immutable => " IMMUTABLE",
    };

    let security = match f.security {
        SecurityMode::Definer => " SECURITY DEFINER",
        SecurityMode::Invoker => " SECURITY INVOKER",
    };

    format!(
        "CREATE OR REPLACE FUNCTION {}({}) RETURNS {}\nLANGUAGE sql{}{}\nAS $$\n{}\n$$",
        f.name,
        params.join(", "),
        returns,
        volatility,
        security,
        f.body
    )
}

fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::Column { name } => name.clone(),
        Expr::QualifiedColumn { table, column } => format!("{}.{}", table, column),
        Expr::Literal(lit) => emit_literal(lit),
        Expr::BinaryOp { left, op, right } => {
            format!(
                "({} {} {})",
                emit_expr(left),
                emit_binary_op(op),
                emit_expr(right)
            )
        }
        Expr::UnaryOp { op, expr } => {
            format!("{} {}", emit_unary_op(op), emit_expr(expr))
        }
        Expr::FunctionCall { name, args } => {
            let args_str: Vec<String> = args.iter().map(emit_expr).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Expr::IsNull { expr, negated } => {
            if *negated {
                format!("{} IS NOT NULL", emit_expr(expr))
            } else {
                format!("{} IS NULL", emit_expr(expr))
            }
        }
        Expr::InList {
            expr,
            list,
            negated,
        } => {
            let items: Vec<String> = list.iter().map(emit_expr).collect();
            let op = if *negated { "NOT IN" } else { "IN" };
            format!("{} {} ({})", emit_expr(expr), op, items.join(", "))
        }
        Expr::Subquery { sql } => format!("({})", sql),
        Expr::Raw { sql } => sql.clone(),
    }
}

fn emit_literal(lit: &Literal) -> String {
    match lit {
        Literal::Null => "NULL".to_string(),
        Literal::Bool(b) => b.to_string().to_uppercase(),
        Literal::Int(i) => i.to_string(),
        Literal::Float(f) => f.to_string(),
        Literal::String(s) => format!("'{}'", s.replace('\'', "''")),
    }
}

fn emit_binary_op(op: &BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Eq => "=",
        BinaryOperator::NotEq => "<>",
        BinaryOperator::Lt => "<",
        BinaryOperator::LtEq => "<=",
        BinaryOperator::Gt => ">",
        BinaryOperator::GtEq => ">=",
        BinaryOperator::And => "AND",
        BinaryOperator::Or => "OR",
        BinaryOperator::Like => "LIKE",
        BinaryOperator::ILike => "ILIKE",
    }
}

fn emit_unary_op(op: &UnaryOperator) -> &'static str {
    match op {
        UnaryOperator::Not => "NOT",
        UnaryOperator::Minus => "-",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_create_table() {
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
                ColumnDef {
                    name: "done".to_string(),
                    data_type: DataType::Bool,
                    nullable: true,
                    default: Some(Expr::Literal(Literal::Bool(false))),
                    constraints: vec![],
                },
            ],
            constraints: vec![],
            if_not_exists: false,
        };

        let migration = Migration {
            statements: vec![Statement::CreateTable(ct)],
        };
        let sql = emit_postgres(&migration);

        assert!(sql.contains("CREATE TABLE todos"));
        assert!(sql.contains("id UUID NOT NULL PRIMARY KEY"));
        assert!(sql.contains("title TEXT NOT NULL"));
        assert!(sql.contains("done BOOLEAN DEFAULT FALSE"));
    }
}
