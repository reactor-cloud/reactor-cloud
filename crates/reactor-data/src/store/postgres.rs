//! Postgres implementation of DataStore.

use super::{
    ColumnInfo, DataError, DataStore, DataTx, ForeignKeyInfo, IndexInfo, Row, SchemaSnapshot,
    SqlValue, TableSnapshot,
};
use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{Column, PgPool, Postgres, Row as SqlxRow, Transaction, TypeInfo};
use std::collections::HashMap;

/// Postgres data store.
#[derive(Clone)]
pub struct PgDataStore {
    pool: PgPool,
}

impl PgDataStore {
    /// Create a new Postgres data store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl DataStore for PgDataStore {
    type Tx<'a> = PgDataTx<'a>;

    async fn begin(&self) -> Result<Self::Tx<'_>, DataError> {
        let tx = self.pool.begin().await.map_err(|e| {
            tracing::error!(error = %e, "failed to begin transaction");
            DataError::Database
        })?;
        Ok(PgDataTx { tx })
    }

    async fn introspect_schema(&self, user_schema: &str) -> Result<SchemaSnapshot, DataError> {
        let mut snapshot = SchemaSnapshot::default();

        // Get all tables in the schema
        let tables: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT table_name
            FROM information_schema.tables
            WHERE table_schema = $1 AND table_type = 'BASE TABLE'
            ORDER BY table_name
            "#,
        )
        .bind(user_schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list tables");
            DataError::Database
        })?;

        for (table_name,) in tables {
            let table = self.introspect_table(user_schema, &table_name).await?;
            snapshot.tables.insert(table_name, table);
        }

        Ok(snapshot)
    }

    async fn run_metadata_migrations(&self) -> Result<(), DataError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to run metadata migrations");
                DataError::Database
            })?;
        Ok(())
    }

    fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

impl PgDataStore {
    async fn introspect_table(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<TableSnapshot, DataError> {
        // Get columns
        let columns = self.get_columns(schema, table).await?;

        // Get primary key
        let primary_key = self.get_primary_key(schema, table).await?;

        // Get foreign keys
        let foreign_keys = self.get_foreign_keys(schema, table).await?;

        // Get indexes
        let indexes = self.get_indexes(schema, table).await?;

        Ok(TableSnapshot {
            schema: schema.to_string(),
            name: table.to_string(),
            columns,
            primary_key,
            foreign_keys,
            indexes,
        })
    }

    async fn get_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>, DataError> {
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT 
                column_name,
                data_type,
                is_nullable,
                column_default
            FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2
            ORDER BY ordinal_position
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get columns");
            DataError::Database
        })?;

        Ok(rows
            .into_iter()
            .map(|(name, sql_type, nullable, default)| ColumnInfo {
                name,
                sql_type,
                nullable: nullable == "YES",
                default,
            })
            .collect())
    }

    async fn get_primary_key(&self, schema: &str, table: &str) -> Result<Vec<String>, DataError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT a.attname
            FROM pg_index i
            JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            JOIN pg_class c ON c.oid = i.indrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE i.indisprimary
              AND n.nspname = $1
              AND c.relname = $2
            ORDER BY array_position(i.indkey, a.attnum)
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get primary key");
            DataError::Database
        })?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn get_foreign_keys(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, DataError> {
        let rows: Vec<FkRow> = sqlx::query_as(
            r#"
            SELECT
                tc.constraint_name,
                kcu.column_name,
                ccu.table_name AS ref_table,
                ccu.column_name AS ref_column,
                rc.delete_rule
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage ccu
                ON ccu.constraint_name = tc.constraint_name
                AND ccu.table_schema = tc.table_schema
            JOIN information_schema.referential_constraints rc
                ON rc.constraint_name = tc.constraint_name
                AND rc.constraint_schema = tc.table_schema
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema = $1
              AND tc.table_name = $2
            ORDER BY tc.constraint_name, kcu.ordinal_position
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get foreign keys");
            DataError::Database
        })?;

        // Group by constraint name
        let mut fk_map: HashMap<String, ForeignKeyInfo> = HashMap::new();
        for row in rows {
            let entry = fk_map
                .entry(row.constraint_name.clone())
                .or_insert_with(|| ForeignKeyInfo {
                    name: row.constraint_name,
                    columns: vec![],
                    ref_table: row.ref_table,
                    ref_columns: vec![],
                    on_delete: row.delete_rule,
                });
            entry.columns.push(row.column_name);
            entry.ref_columns.push(row.ref_column);
        }

        Ok(fk_map.into_values().collect())
    }

    async fn get_indexes(&self, schema: &str, table: &str) -> Result<Vec<IndexInfo>, DataError> {
        let rows: Vec<IdxRow> = sqlx::query_as(
            r#"
            SELECT
                i.relname AS index_name,
                a.attname AS column_name,
                ix.indisunique
            FROM pg_index ix
            JOIN pg_class t ON t.oid = ix.indrelid
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_namespace n ON n.oid = t.relnamespace
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            WHERE n.nspname = $1
              AND t.relname = $2
              AND NOT ix.indisprimary
            ORDER BY i.relname, array_position(ix.indkey, a.attnum)
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get indexes");
            DataError::Database
        })?;

        // Group by index name
        let mut idx_map: HashMap<String, IndexInfo> = HashMap::new();
        for row in rows {
            let entry = idx_map
                .entry(row.index_name.clone())
                .or_insert_with(|| IndexInfo {
                    name: row.index_name,
                    columns: vec![],
                    unique: row.is_unique,
                });
            entry.columns.push(row.column_name);
        }

        Ok(idx_map.into_values().collect())
    }
}

#[derive(sqlx::FromRow)]
struct FkRow {
    constraint_name: String,
    column_name: String,
    ref_table: String,
    ref_column: String,
    delete_rule: String,
}

#[derive(sqlx::FromRow)]
struct IdxRow {
    index_name: String,
    column_name: String,
    #[sqlx(rename = "indisunique")]
    is_unique: bool,
}

/// Postgres transaction wrapper.
pub struct PgDataTx<'a> {
    tx: Transaction<'a, Postgres>,
}

#[async_trait]
impl<'a> DataTx for PgDataTx<'a> {
    async fn execute_raw(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, DataError> {
        let mut query = sqlx::query(sql);
        for param in params {
            query = bind_value(query, param);
        }

        let result = query.execute(&mut *self.tx).await.map_err(|e| {
            tracing::error!(error = %e, sql = sql, "failed to execute SQL");
            DataError::Database
        })?;

        Ok(result.rows_affected())
    }

    async fn fetch_rows(&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>, DataError> {
        let mut query = sqlx::query(sql);
        for param in params {
            query = bind_value(query, param);
        }

        let rows: Vec<PgRow> = query.fetch_all(&mut *self.tx).await.map_err(|e| {
            tracing::error!(error = %e, sql = sql, "failed to fetch rows");
            DataError::Database
        })?;

        rows.into_iter().map(pg_row_to_map).collect()
    }

    async fn commit(self) -> Result<(), DataError> {
        self.tx.commit().await.map_err(|e| {
            tracing::error!(error = %e, "failed to commit transaction");
            DataError::Database
        })
    }

    async fn rollback(self) -> Result<(), DataError> {
        self.tx.rollback().await.map_err(|e| {
            tracing::error!(error = %e, "failed to rollback transaction");
            DataError::Database
        })
    }
}

fn bind_value<'q>(
    query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    value: &'q SqlValue,
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    match value {
        SqlValue::Null => query.bind(None::<String>),
        SqlValue::NullUuid => query.bind(None::<uuid::Uuid>),
        SqlValue::NullInt => query.bind(None::<i64>),
        SqlValue::NullJson => query.bind(None::<serde_json::Value>),
        SqlValue::Bool(v) => query.bind(*v),
        SqlValue::Int(v) => query.bind(*v),
        SqlValue::Float(v) => query.bind(*v),
        SqlValue::Text(v) => query.bind(v.as_str()),
        SqlValue::Uuid(v) => query.bind(*v),
        SqlValue::Timestamp(v) => query.bind(*v),
        SqlValue::Json(v) => query.bind(v.clone()),
        SqlValue::Bytes(v) => query.bind(v.as_slice()),
    }
}

fn pg_row_to_map(row: PgRow) -> Result<Row, DataError> {
    use sqlx::Column;

    let mut map = HashMap::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let value = get_column_value(&row, col)?;
        map.insert(name, value);
    }

    Ok(map)
}

fn get_column_value(
    row: &PgRow,
    col: &sqlx::postgres::PgColumn,
) -> Result<serde_json::Value, DataError> {
    let type_name = col.type_info().name();

    // Try to get the value based on type
    let value: serde_json::Value = match type_name {
        "BOOL" => {
            let v: Option<bool> = row.try_get(col.name()).ok();
            v.map(serde_json::Value::Bool)
                .unwrap_or(serde_json::Value::Null)
        }
        "INT2" | "INT4" => {
            let v: Option<i32> = row.try_get(col.name()).ok();
            v.map(|n| serde_json::Value::Number(n.into()))
                .unwrap_or(serde_json::Value::Null)
        }
        "INT8" => {
            let v: Option<i64> = row.try_get(col.name()).ok();
            v.map(|n| serde_json::Value::Number(n.into()))
                .unwrap_or(serde_json::Value::Null)
        }
        "FLOAT4" | "FLOAT8" => {
            let v: Option<f64> = row.try_get(col.name()).ok();
            v.and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        "UUID" => {
            let v: Option<uuid::Uuid> = row.try_get(col.name()).ok();
            v.map(|u| serde_json::Value::String(u.to_string()))
                .unwrap_or(serde_json::Value::Null)
        }
        "TIMESTAMPTZ" | "TIMESTAMP" => {
            let v: Option<chrono::DateTime<chrono::Utc>> = row.try_get(col.name()).ok();
            v.map(|t| serde_json::Value::String(t.to_rfc3339()))
                .unwrap_or(serde_json::Value::Null)
        }
        "JSON" | "JSONB" => {
            let v: Option<serde_json::Value> = row.try_get(col.name()).ok();
            v.unwrap_or(serde_json::Value::Null)
        }
        "BYTEA" => {
            use base64::Engine;
            let v: Option<Vec<u8>> = row.try_get(col.name()).ok();
            v.map(|b| {
                serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&b))
            })
            .unwrap_or(serde_json::Value::Null)
        }
        _ => {
            // Default to string
            let v: Option<String> = row.try_get(col.name()).ok();
            v.map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null)
        }
    };

    Ok(value)
}
