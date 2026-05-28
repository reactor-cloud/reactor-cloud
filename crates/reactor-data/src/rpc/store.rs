//! RPC function storage.
//!
//! Manages the `_reactor_data.rpc_functions` table for storing function metadata.

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;

/// Security mode for RPC functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SecurityMode {
    /// Function runs with the permissions of the caller (invoker).
    /// Table policies are enforced inside the function.
    Invoker,
    /// Function runs with elevated permissions (definer).
    /// Table policies are bypassed, audit is recorded.
    #[default]
    Definer,
}

impl SecurityMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "invoker" => Self::Invoker,
            _ => Self::Definer,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Invoker => "invoker",
            Self::Definer => "definer",
        }
    }
}

/// A parameter in an RPC function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcParam {
    /// Parameter name.
    pub name: String,
    /// SQL type (e.g., "text", "integer", "uuid").
    pub sql_type: String,
    /// Whether this parameter has a default value.
    pub has_default: bool,
    /// Position in the parameter list (1-indexed).
    pub position: i32,
}

/// A registered RPC function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcFunction {
    /// Function ID.
    pub id: i64,
    /// Schema name.
    pub schema_name: String,
    /// Function name.
    pub name: String,
    /// Function parameters.
    pub params: Vec<RpcParam>,
    /// Return type (e.g., "setof", "record", "void", or specific type).
    pub return_type: String,
    /// Whether the function returns a set of rows.
    pub returns_set: bool,
    /// The SQL body of the function.
    pub body: String,
    /// Security mode (invoker or definer).
    pub security: SecurityMode,
    /// Migration that created this function.
    pub migration_name: String,
}

/// RPC store errors.
#[derive(Debug, Error)]
pub enum RpcStoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("function not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// RPC function store.
pub struct RpcStore<'a> {
    pool: &'a PgPool,
}

impl<'a> RpcStore<'a> {
    /// Create a new RPC store.
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Register a new RPC function.
    pub async fn register(
        &self,
        schema_name: &str,
        name: &str,
        params: &[RpcParam],
        return_type: &str,
        returns_set: bool,
        body: &str,
        security: SecurityMode,
        migration_name: &str,
    ) -> Result<i64, RpcStoreError> {
        let params_json = serde_json::to_value(params)?;

        let row = sqlx::query(
            r#"
            INSERT INTO _reactor_data.rpc_functions
                (schema_name, name, params, return_type, returns_set, body, security, migration_name, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            ON CONFLICT (schema_name, name) DO UPDATE SET
                params = EXCLUDED.params,
                return_type = EXCLUDED.return_type,
                returns_set = EXCLUDED.returns_set,
                body = EXCLUDED.body,
                security = EXCLUDED.security,
                migration_name = EXCLUDED.migration_name
            RETURNING id
            "#,
        )
        .bind(schema_name)
        .bind(name)
        .bind(params_json)
        .bind(return_type)
        .bind(returns_set)
        .bind(body)
        .bind(security.as_str())
        .bind(migration_name)
        .fetch_one(self.pool)
        .await?;

        Ok(row.get("id"))
    }

    /// Get a function by name.
    pub async fn get(&self, schema_name: &str, name: &str) -> Result<RpcFunction, RpcStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, schema_name, name, params, return_type, returns_set, body, security, migration_name
            FROM _reactor_data.rpc_functions
            WHERE schema_name = $1 AND name = $2
            "#,
        )
        .bind(schema_name)
        .bind(name)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(row) => {
                let params_json: serde_json::Value = row.get("params");
                let params: Vec<RpcParam> = serde_json::from_value(params_json)?;
                let security_str: String = row.get("security");

                Ok(RpcFunction {
                    id: row.get("id"),
                    schema_name: row.get("schema_name"),
                    name: row.get("name"),
                    params,
                    return_type: row.get("return_type"),
                    returns_set: row.get("returns_set"),
                    body: row.get("body"),
                    security: SecurityMode::from_str(&security_str),
                    migration_name: row.get("migration_name"),
                })
            }
            None => Err(RpcStoreError::NotFound(name.to_string())),
        }
    }

    /// List all functions in a schema.
    pub async fn list(&self, schema_name: &str) -> Result<Vec<RpcFunction>, RpcStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT id, schema_name, name, params, return_type, returns_set, body, security, migration_name
            FROM _reactor_data.rpc_functions
            WHERE schema_name = $1
            ORDER BY name
            "#,
        )
        .bind(schema_name)
        .fetch_all(self.pool)
        .await?;

        let mut functions = Vec::with_capacity(rows.len());
        for row in rows {
            let params_json: serde_json::Value = row.get("params");
            let params: Vec<RpcParam> = serde_json::from_value(params_json)?;
            let security_str: String = row.get("security");

            functions.push(RpcFunction {
                id: row.get("id"),
                schema_name: row.get("schema_name"),
                name: row.get("name"),
                params,
                return_type: row.get("return_type"),
                returns_set: row.get("returns_set"),
                body: row.get("body"),
                security: SecurityMode::from_str(&security_str),
                migration_name: row.get("migration_name"),
            });
        }

        Ok(functions)
    }

    /// Delete a function.
    pub async fn delete(&self, schema_name: &str, name: &str) -> Result<bool, RpcStoreError> {
        let result = sqlx::query(
            r#"
            DELETE FROM _reactor_data.rpc_functions
            WHERE schema_name = $1 AND name = $2
            "#,
        )
        .bind(schema_name)
        .bind(name)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_mode_roundtrip() {
        assert_eq!(SecurityMode::from_str("invoker"), SecurityMode::Invoker);
        assert_eq!(SecurityMode::from_str("INVOKER"), SecurityMode::Invoker);
        assert_eq!(SecurityMode::from_str("definer"), SecurityMode::Definer);
        assert_eq!(SecurityMode::from_str("anything"), SecurityMode::Definer);

        assert_eq!(SecurityMode::Invoker.as_str(), "invoker");
        assert_eq!(SecurityMode::Definer.as_str(), "definer");
    }

    #[test]
    fn test_rpc_param_serialization() {
        let param = RpcParam {
            name: "user_id".to_string(),
            sql_type: "uuid".to_string(),
            has_default: false,
            position: 1,
        };

        let json = serde_json::to_string(&param).unwrap();
        let parsed: RpcParam = serde_json::from_str(&json).unwrap();

        assert_eq!(param.name, parsed.name);
        assert_eq!(param.sql_type, parsed.sql_type);
    }
}
