//! Policy persistence.
//!
//! Stores and retrieves policies from `_reactor_data.policies`.

use reactor_policy::{PolicyExpr, PolicyScope};
use sqlx::{PgPool, Row};
use thiserror::Error;

/// Errors during policy storage operations.
#[derive(Debug, Error)]
pub enum PolicyStoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// A stored policy record.
#[derive(Debug, Clone)]
pub struct StoredPolicy {
    pub id: i64,
    pub schema_name: String,
    pub table_name: String,
    pub name: String,
    pub scopes: Vec<PolicyScope>,
    pub using_expr: Option<PolicyExpr>,
    pub check_expr: Option<PolicyExpr>,
    pub migration_name: String,
}

/// Policy storage operations.
pub struct PolicyStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PolicyStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new policy.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        &self,
        schema_name: &str,
        table_name: &str,
        name: &str,
        scopes: &[PolicyScope],
        using_expr: Option<&PolicyExpr>,
        check_expr: Option<&PolicyExpr>,
        migration_name: &str,
    ) -> Result<i64, PolicyStoreError> {
        let scopes_json: Vec<String> = scopes
            .iter()
            .map(|s: &PolicyScope| s.as_str().to_string())
            .collect();
        let using_json = using_expr
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        let check_json = check_expr
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);

        let row = sqlx::query(
            r#"
            INSERT INTO _reactor_data.policies 
                (schema_name, table_name, name, scopes, using_ast, check_ast, migration_name, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            RETURNING id
            "#,
        )
        .bind(schema_name)
        .bind(table_name)
        .bind(name)
        .bind(&scopes_json)
        .bind(using_json)
        .bind(check_json)
        .bind(migration_name)
        .fetch_one(self.pool)
        .await?;

        Ok(row.get("id"))
    }

    /// Get all policies for a table.
    pub async fn get_for_table(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<StoredPolicy>, PolicyStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT id, schema_name, table_name, name, scopes, using_ast, check_ast, migration_name
            FROM _reactor_data.policies
            WHERE schema_name = $1 AND table_name = $2
            ORDER BY created_at
            "#,
        )
        .bind(schema_name)
        .bind(table_name)
        .fetch_all(self.pool)
        .await?;

        let mut policies = Vec::with_capacity(rows.len());
        for row in rows {
            let scopes_strs: Vec<String> = row.get("scopes");
            let scopes = scopes_strs
                .iter()
                .filter_map(|s| match s.as_str() {
                    "select" => Some(PolicyScope::Select),
                    "insert" => Some(PolicyScope::Insert),
                    "update" => Some(PolicyScope::Update),
                    "delete" => Some(PolicyScope::Delete),
                    _ => None,
                })
                .collect();

            let using_json: serde_json::Value = row.get("using_ast");
            let using_expr = if using_json.is_null() {
                None
            } else {
                Some(serde_json::from_value(using_json)?)
            };

            let check_json: serde_json::Value = row.get("check_ast");
            let check_expr = if check_json.is_null() {
                None
            } else {
                Some(serde_json::from_value(check_json)?)
            };

            policies.push(StoredPolicy {
                id: row.get("id"),
                schema_name: row.get("schema_name"),
                table_name: row.get("table_name"),
                name: row.get("name"),
                scopes,
                using_expr,
                check_expr,
                migration_name: row.get("migration_name"),
            });
        }

        Ok(policies)
    }

    /// Get all policies for a table and scope.
    pub async fn get_for_scope(
        &self,
        schema_name: &str,
        table_name: &str,
        scope: PolicyScope,
    ) -> Result<Vec<StoredPolicy>, PolicyStoreError> {
        let all = self.get_for_table(schema_name, table_name).await?;
        Ok(all
            .into_iter()
            .filter(|p| p.scopes.contains(&scope))
            .collect())
    }

    /// Delete all policies for a migration (used during migration rollback).
    pub async fn delete_for_migration(
        &self,
        migration_name: &str,
    ) -> Result<u64, PolicyStoreError> {
        let result = sqlx::query("DELETE FROM _reactor_data.policies WHERE migration_name = $1")
            .bind(migration_name)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Check if a policy exists.
    pub async fn exists(
        &self,
        schema_name: &str,
        table_name: &str,
        name: &str,
    ) -> Result<bool, PolicyStoreError> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM _reactor_data.policies 
                WHERE schema_name = $1 AND table_name = $2 AND name = $3
            ) as exists
            "#,
        )
        .bind(schema_name)
        .bind(table_name)
        .bind(name)
        .fetch_one(self.pool)
        .await?;

        Ok(row.get("exists"))
    }
}

#[cfg(test)]
mod tests {
    use reactor_policy::{PolicyBinaryOp, PolicyExpr};

    #[test]
    fn test_policy_expr_roundtrip() {
        let expr = PolicyExpr::binary(
            PolicyExpr::column("org_id"),
            PolicyBinaryOp::Eq,
            PolicyExpr::auth_builtin("org_id", vec![]),
        );

        let json = serde_json::to_string(&expr).unwrap();
        let parsed: PolicyExpr = serde_json::from_str(&json).unwrap();

        assert_eq!(expr, parsed);
    }
}
