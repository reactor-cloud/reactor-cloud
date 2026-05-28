//! Data capability client (`/data/v1/*`).

use crate::error::ClientResult;
use crate::http::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Table inspection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<i64>,
}

/// Column information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
}

/// Query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: Option<i64>,
}

/// Data migration result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMigrateResult {
    pub applied: Vec<String>,
    pub pending: Vec<String>,
}

impl Client {
    /// Inspect a table.
    pub async fn data_inspect(&self, table: &str) -> ClientResult<TableInfo> {
        self.get(&format!("/data/v1/_admin/tables/{}", table)).await
    }

    /// List tables.
    pub async fn data_tables_list(&self) -> ClientResult<Vec<String>> {
        self.get("/data/v1/_admin/tables").await
    }

    /// Execute a query.
    pub async fn data_query(
        &self,
        sql: &str,
        params: Option<HashMap<String, serde_json::Value>>,
        write: bool,
    ) -> ClientResult<QueryResult> {
        #[derive(Serialize)]
        struct Query<'a> {
            sql: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            params: Option<HashMap<String, serde_json::Value>>,
            write: bool,
        }
        self.post("/data/v1/_admin/query", &Query { sql, params, write })
            .await
    }

    /// Run data migrations.
    pub async fn data_migrate(&self, dry_run: bool) -> ClientResult<DataMigrateResult> {
        let path = if dry_run {
            "/data/v1/_admin/migrate?dry_run=true"
        } else {
            "/data/v1/_admin/migrate"
        };
        self.post(path, &()).await
    }
}
