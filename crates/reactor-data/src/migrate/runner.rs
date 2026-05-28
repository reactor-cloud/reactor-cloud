//! Migration runner.
//!
//! Applies user-defined SQL migrations transactionally.

use crate::dialect::{
    emit_postgres, lint_statement, parse_migration, LintError, ParseError, PolicyStmt, Statement,
    PolicyScope as DialectPolicyScope,
};
use crate::error::DataError;
use crate::policy::{parse_policy_expr, PolicyParseError, PolicyStore, PolicyScope};
use crate::store::{DataStore, SchemaSnapshot};
use sqlx::{Executor, PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;
use thiserror::Error;
use tracing::info;

use super::source::{MigrationFile, MigrationSource};

/// Convert dialect PolicyScope to policy engine PolicyScope.
fn convert_scope(scope: &DialectPolicyScope) -> PolicyScope {
    match scope {
        DialectPolicyScope::Select => PolicyScope::Select,
        DialectPolicyScope::Insert => PolicyScope::Insert,
        DialectPolicyScope::Update => PolicyScope::Update,
        DialectPolicyScope::Delete => PolicyScope::Delete,
    }
}

/// Convert a dialect Expr to a raw SQL string for policy parsing.
fn expr_to_raw_sql(expr: &crate::dialect::Expr) -> String {
    use crate::dialect::Expr;

    match expr {
        Expr::Column { name } => name.clone(),
        Expr::QualifiedColumn { table, column } => format!("{}.{}", table, column),
        Expr::Literal(lit) => {
            use crate::dialect::Literal;
            match lit {
                Literal::Null => "NULL".to_string(),
                Literal::Bool(b) => b.to_string().to_uppercase(),
                Literal::Int(i) => i.to_string(),
                Literal::Float(f) => f.to_string(),
                Literal::String(s) => format!("'{}'", s.replace('\'', "''")),
            }
        }
        Expr::BinaryOp { left, op, right } => {
            use crate::dialect::BinaryOperator;
            let op_str = match op {
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
            };
            format!(
                "({} {} {})",
                expr_to_raw_sql(left),
                op_str,
                expr_to_raw_sql(right)
            )
        }
        Expr::UnaryOp { op, expr: inner } => {
            use crate::dialect::UnaryOperator;
            match op {
                UnaryOperator::Not => format!("NOT {}", expr_to_raw_sql(inner)),
                UnaryOperator::Minus => format!("-{}", expr_to_raw_sql(inner)),
            }
        }
        Expr::FunctionCall { name, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_raw_sql).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Expr::IsNull {
            expr: inner,
            negated,
        } => {
            if *negated {
                format!("{} IS NOT NULL", expr_to_raw_sql(inner))
            } else {
                format!("{} IS NULL", expr_to_raw_sql(inner))
            }
        }
        Expr::InList {
            expr: inner,
            list,
            negated,
        } => {
            let items: Vec<String> = list.iter().map(expr_to_raw_sql).collect();
            let op = if *negated { "NOT IN" } else { "IN" };
            format!("{} {} ({})", expr_to_raw_sql(inner), op, items.join(", "))
        }
        Expr::Subquery { sql } => format!("({})", sql),
        Expr::Raw { sql } => sql.clone(),
    }
}

/// Errors that can occur during migration.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error(
        "migration drift detected for '{name}': expected checksum '{expected}', got '{actual}'"
    )]
    Drift {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("parse error in migration '{name}': {source}")]
    Parse { name: String, source: ParseError },

    #[error("lint error in migration '{name}': {source}")]
    Lint { name: String, source: LintError },

    #[error("policy parse error in migration '{name}': {source}")]
    PolicyParse {
        name: String,
        source: PolicyParseError,
    },

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("data store error: {0}")]
    Store(#[from] DataError),

    #[error("policy store error: {0}")]
    PolicyStore(String),
}

/// Record of an applied migration.
#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub name: String,
    pub checksum: String,
    pub applied_at: chrono::DateTime<chrono::Utc>,
}

/// Migration runner.
pub struct MigrationRunner<S: MigrationSource> {
    source: S,
    user_schema: String,
}

impl<S: MigrationSource> MigrationRunner<S> {
    /// Create a new migration runner.
    pub fn new(source: S, user_schema: impl Into<String>) -> Self {
        Self {
            source,
            user_schema: user_schema.into(),
        }
    }

    /// Get the list of applied migrations from the database.
    pub async fn get_applied_migrations(
        &self,
        pool: &PgPool,
    ) -> Result<Vec<AppliedMigration>, MigrationError> {
        let rows = sqlx::query(
            "SELECT name, checksum, applied_at FROM _reactor_data.migrations ORDER BY applied_at",
        )
        .fetch_all(pool)
        .await?;

        let mut applied = Vec::with_capacity(rows.len());
        for row in rows {
            applied.push(AppliedMigration {
                name: row.get("name"),
                checksum: row.get("checksum"),
                applied_at: row.get("applied_at"),
            });
        }

        Ok(applied)
    }

    /// Discover pending migrations (not yet applied).
    pub async fn get_pending_migrations(
        &self,
        pool: &PgPool,
    ) -> Result<Vec<MigrationFile>, MigrationError> {
        let applied = self.get_applied_migrations(pool).await?;
        let applied_map: HashMap<_, _> = applied.iter().map(|m| (&m.name, &m.checksum)).collect();

        let all_files = self.source.discover()?;
        let mut pending = Vec::new();

        for file in all_files {
            match applied_map.get(&file.name) {
                Some(expected_checksum) => {
                    if **expected_checksum != file.checksum {
                        return Err(MigrationError::Drift {
                            name: file.name.clone(),
                            expected: (*expected_checksum).clone(),
                            actual: file.checksum.clone(),
                        });
                    }
                    // Already applied with matching checksum, skip
                }
                None => {
                    pending.push(file);
                }
            }
        }

        Ok(pending)
    }

    /// Run all pending migrations.
    ///
    /// Returns the number of migrations applied.
    pub async fn run<D: DataStore>(
        &self,
        pool: &PgPool,
        store: &D,
    ) -> Result<usize, MigrationError> {
        let pending = self.get_pending_migrations(pool).await?;

        if pending.is_empty() {
            info!("No pending migrations");
            return Ok(0);
        }

        info!("Found {} pending migration(s)", pending.len());

        let mut count = 0;
        for file in pending {
            self.apply_single(pool, store, &file).await?;
            count += 1;
        }

        info!("Applied {} migration(s)", count);
        Ok(count)
    }

    /// Apply a single migration file.
    async fn apply_single<D: DataStore>(
        &self,
        pool: &PgPool,
        store: &D,
        file: &MigrationFile,
    ) -> Result<(), MigrationError> {
        info!("Applying migration: {}", file.name);

        // Parse the migration
        let migration = parse_migration(&file.content).map_err(|e| MigrationError::Parse {
            name: file.name.clone(),
            source: e,
        })?;

        // Lint each statement
        for stmt in &migration.statements {
            if let Err(e) = lint_statement(stmt) {
                return Err(MigrationError::Lint {
                    name: file.name.clone(),
                    source: e,
                });
            }
        }

        // Extract policy statements
        let policies: Vec<&PolicyStmt> = migration
            .statements
            .iter()
            .filter_map(|s| match s {
                Statement::Policy(p) => Some(p),
                _ => None,
            })
            .collect();

        // Emit Postgres DDL
        let ddl = emit_postgres(&migration);

        // Execute in a transaction
        let mut tx = pool.begin().await?;

        // Execute the DDL
        self.execute_ddl(&mut tx, &ddl).await?;

        // Record the migration
        self.record_migration(&mut tx, file).await?;

        // Commit the DDL transaction
        tx.commit().await?;

        // Store policies (after DDL is committed so the table exists)
        self.store_policies(pool, &file.name, &policies).await?;

        // Refresh the schema snapshot
        self.refresh_tables(pool, store).await?;

        info!("Migration applied: {}", file.name);
        Ok(())
    }

    /// Store policy statements in _reactor_data.policies.
    async fn store_policies(
        &self,
        pool: &PgPool,
        migration_name: &str,
        policies: &[&PolicyStmt],
    ) -> Result<(), MigrationError> {
        if policies.is_empty() {
            return Ok(());
        }

        let policy_store = PolicyStore::new(pool);

        for policy in policies {
            // Parse the using expression if present
            let using_expr = if let Some(ref using) = policy.using {
                let raw_sql = expr_to_raw_sql(using);
                Some(
                    parse_policy_expr(&raw_sql).map_err(|e| MigrationError::PolicyParse {
                        name: migration_name.to_string(),
                        source: e,
                    })?,
                )
            } else {
                None
            };

            // Parse the check expression if present
            let check_expr = if let Some(ref check) = policy.check {
                let raw_sql = expr_to_raw_sql(check);
                Some(
                    parse_policy_expr(&raw_sql).map_err(|e| MigrationError::PolicyParse {
                        name: migration_name.to_string(),
                        source: e,
                    })?,
                )
            } else {
                None
            };

            let converted_scopes: Vec<PolicyScope> = policy.scopes.iter().map(convert_scope).collect();
            policy_store
                .insert(
                    &self.user_schema,
                    &policy.table,
                    &policy.name,
                    &converted_scopes,
                    using_expr.as_ref(),
                    check_expr.as_ref(),
                    migration_name,
                )
                .await
                .map_err(|e| MigrationError::PolicyStore(e.to_string()))?;

            info!(
                "Stored policy '{}' on table '{}' (scopes: {:?})",
                policy.name,
                policy.table,
                policy.scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>()
            );
        }

        Ok(())
    }

    /// Execute DDL statements.
    async fn execute_ddl(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ddl: &str,
    ) -> Result<(), MigrationError> {
        // Set search path to user schema
        let set_path = format!("SET search_path TO {}", self.user_schema);
        tx.execute(set_path.as_ref()).await?;

        // Execute the DDL
        tx.execute(ddl).await?;

        Ok(())
    }

    /// Record that a migration was applied.
    async fn record_migration(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        file: &MigrationFile,
    ) -> Result<(), MigrationError> {
        sqlx::query(
            r#"
            INSERT INTO _reactor_data.migrations (name, checksum, applied_at)
            VALUES ($1, $2, NOW())
            "#,
        )
        .bind(&file.name)
        .bind(&file.checksum)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Refresh the _reactor_data.tables snapshot.
    async fn refresh_tables<D: DataStore>(
        &self,
        pool: &PgPool,
        store: &D,
    ) -> Result<(), MigrationError> {
        // Introspect the current schema
        let snapshot = store.introspect_schema(&self.user_schema).await?;

        // Store in _reactor_data.tables
        self.store_snapshot(pool, &self.user_schema, &snapshot)
            .await?;

        Ok(())
    }

    /// Store the schema snapshot in _reactor_data.tables.
    async fn store_snapshot(
        &self,
        pool: &PgPool,
        schema_name: &str,
        snapshot: &SchemaSnapshot,
    ) -> Result<(), MigrationError> {
        // Clear existing entries for this schema
        sqlx::query("DELETE FROM _reactor_data.tables WHERE schema_name = $1")
            .bind(schema_name)
            .execute(pool)
            .await?;

        // Insert each table
        for (table_name, table) in &snapshot.tables {
            let columns_json = serde_json::to_value(&table.columns).unwrap_or_default();
            let primary_key_json = serde_json::to_value(&table.primary_key).unwrap_or_default();
            let foreign_keys_json = serde_json::to_value(&table.foreign_keys).unwrap_or_default();
            let indexes_json = serde_json::to_value(&table.indexes).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO _reactor_data.tables 
                    (schema_name, table_name, columns, primary_key, foreign_keys, indexes, refreshed_at)
                VALUES ($1, $2, $3, $4, $5, $6, NOW())
                "#,
            )
            .bind(schema_name)
            .bind(table_name)
            .bind(columns_json)
            .bind(primary_key_json)
            .bind(foreign_keys_json)
            .bind(indexes_json)
            .execute(pool)
            .await?;
        }

        info!(
            "Refreshed schema snapshot: {} tables in schema '{}'",
            snapshot.tables.len(),
            schema_name
        );

        Ok(())
    }

    /// Check for drift without applying migrations.
    pub async fn check_drift(&self, pool: &PgPool) -> Result<(), MigrationError> {
        let applied = self.get_applied_migrations(pool).await?;
        let applied_map: HashMap<_, _> = applied.iter().map(|m| (&m.name, &m.checksum)).collect();

        let all_files = self.source.discover()?;

        for file in all_files {
            if let Some(expected_checksum) = applied_map.get(&file.name) {
                if **expected_checksum != file.checksum {
                    return Err(MigrationError::Drift {
                        name: file.name.clone(),
                        expected: (*expected_checksum).clone(),
                        actual: file.checksum.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}
