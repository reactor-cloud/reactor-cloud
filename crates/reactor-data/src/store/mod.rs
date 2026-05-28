//! Data store abstraction.
//!
//! This module provides the `DataStore` trait for database operations and
//! a Postgres implementation (`PgDataStore`).

mod postgres;

pub use postgres::PgDataStore;

use crate::error::DataError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single row returned from the database.
pub type Row = HashMap<String, serde_json::Value>;

/// SQL parameter value.
#[derive(Debug, Clone)]
pub enum SqlValue {
    /// Untyped null (binds as TEXT NULL) - avoid using for non-TEXT columns.
    Null,
    /// Typed UUID null.
    NullUuid,
    /// Typed INT null.
    NullInt,
    /// Typed JSON null.
    NullJson,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Uuid(uuid::Uuid),
    Timestamp(chrono::DateTime<chrono::Utc>),
    Json(serde_json::Value),
    Bytes(Vec<u8>),
}

/// Schema snapshot for a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSnapshot {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
}

/// Column information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub sql_type: String,
    pub nullable: bool,
    pub default: Option<String>,
}

/// Foreign key information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: String,
}

/// Index information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Complete schema snapshot (all tables).
#[derive(Debug, Clone, Default)]
pub struct SchemaSnapshot {
    pub tables: HashMap<String, TableSnapshot>,
}

impl SchemaSnapshot {
    /// Look up a table by name.
    pub fn get_table(&self, name: &str) -> Option<&TableSnapshot> {
        self.tables.get(name)
    }
}

/// Database store abstraction.
///
/// This trait is the low-level cursor for database operations.
/// Higher-level CRUD operations are built on top of this.
#[async_trait]
pub trait DataStore: Send + Sync + 'static {
    /// Transaction type.
    type Tx<'a>: DataTx
    where
        Self: 'a;

    /// Begin a new transaction.
    async fn begin(&self) -> Result<Self::Tx<'_>, DataError>;

    /// Introspect the schema for the user tables.
    async fn introspect_schema(&self, user_schema: &str) -> Result<SchemaSnapshot, DataError>;

    /// Run the metadata migrations (internal _reactor_data schema).
    async fn run_metadata_migrations(&self) -> Result<(), DataError>;

    /// Get a reference to the underlying connection pool.
    ///
    /// This is used for policy evaluation which needs direct database access.
    fn pool(&self) -> &sqlx::PgPool;
}

/// Database transaction.
#[async_trait]
pub trait DataTx: Send {
    /// Execute a raw SQL statement and return affected row count.
    async fn execute_raw(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, DataError>;

    /// Fetch rows from a query.
    async fn fetch_rows(&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>, DataError>;

    /// Commit the transaction.
    async fn commit(self) -> Result<(), DataError>;

    /// Roll back the transaction.
    async fn rollback(self) -> Result<(), DataError>;
}
