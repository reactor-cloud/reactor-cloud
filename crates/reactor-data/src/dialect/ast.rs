//! Reactor SQL AST — the canonical representation for portable migrations.
//!
//! This AST represents the subset of SQL that Reactor supports across all backends.
//! It is the output of parsing and the input to emission.

use serde::{Deserialize, Serialize};

/// A complete migration file can contain multiple statements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub statements: Vec<Statement>,
}

/// A single DDL statement in the Reactor dialect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Statement {
    CreateTable(CreateTable),
    CreateIndex(CreateIndex),
    AlterTableAddColumn(AlterTableAddColumn),
    AlterTableDropColumn(AlterTableDropColumn),
    AlterTableRename(AlterTableRename),
    AlterColumnRename(AlterColumnRename),
    DropTable(DropTable),
    DropIndex(DropIndex),
    CreateFunction(CreateFunction),
    Policy(PolicyStmt),
}

/// CREATE TABLE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTable {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub constraints: Vec<TableConstraint>,
    pub if_not_exists: bool,
}

/// Column definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Expr>,
    pub constraints: Vec<ColumnConstraint>,
}

/// Column-level constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ColumnConstraint {
    PrimaryKey,
    Unique,
    NotNull,
    Check { expr: Expr },
    References(ForeignKeyRef),
}

/// Table-level constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TableConstraint {
    PrimaryKey {
        name: Option<String>,
        columns: Vec<String>,
    },
    Unique {
        name: Option<String>,
        columns: Vec<String>,
    },
    ForeignKey {
        name: Option<String>,
        columns: Vec<String>,
        references: ForeignKeyRef,
    },
    Check {
        name: Option<String>,
        expr: Expr,
    },
}

/// Foreign key reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyRef {
    pub table: String,
    pub columns: Vec<String>,
    pub on_delete: Option<ReferentialAction>,
    pub on_update: Option<ReferentialAction>,
}

/// Referential action for foreign keys.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReferentialAction {
    Cascade,
    SetNull,
    SetDefault,
    Restrict,
    NoAction,
}

/// Data types supported by Reactor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    /// Reactor's UUIDv7 type.
    ReactorId,
    /// Text/string.
    Text,
    /// Boolean.
    Bool,
    /// 32-bit integer.
    Int,
    /// 64-bit integer.
    BigInt,
    /// 64-bit floating point.
    Float,
    /// Timestamp with timezone.
    TimestampTz,
    /// JSON/JSONB.
    Jsonb,
    /// Binary data.
    Bytea,
}

impl DataType {
    /// Parse a SQL type name into a ReactorType.
    pub fn from_sql(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "reactor_id" | "reactorid" => Some(Self::ReactorId),
            "text" | "varchar" | "character varying" => Some(Self::Text),
            "bool" | "boolean" => Some(Self::Bool),
            "int" | "int4" | "integer" => Some(Self::Int),
            "bigint" | "int8" => Some(Self::BigInt),
            "float" | "float8" | "double precision" | "real" | "float4" => Some(Self::Float),
            "timestamptz" | "timestamp with time zone" => Some(Self::TimestampTz),
            "jsonb" | "json" => Some(Self::Jsonb),
            "bytea" | "blob" => Some(Self::Bytea),
            _ => None,
        }
    }
}

/// CREATE INDEX statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIndex {
    pub name: String,
    pub table: String,
    pub columns: Vec<IndexColumn>,
    pub unique: bool,
    pub if_not_exists: bool,
    pub where_clause: Option<Expr>,
}

/// Index column specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexColumn {
    pub name: String,
    pub order: Option<SortOrder>,
    pub nulls: Option<NullsOrder>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NullsOrder {
    First,
    Last,
}

/// ALTER TABLE ADD COLUMN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterTableAddColumn {
    pub table: String,
    pub column: ColumnDef,
}

/// ALTER TABLE DROP COLUMN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterTableDropColumn {
    pub table: String,
    pub column: String,
    pub if_exists: bool,
}

/// ALTER TABLE RENAME.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterTableRename {
    pub old_name: String,
    pub new_name: String,
}

/// ALTER COLUMN RENAME (via ALTER TABLE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterColumnRename {
    pub table: String,
    pub old_name: String,
    pub new_name: String,
}

/// DROP TABLE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropTable {
    pub name: String,
    pub if_exists: bool,
}

/// DROP INDEX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropIndex {
    pub name: String,
    pub if_exists: bool,
}

/// CREATE FUNCTION (SQL language only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFunction {
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub returns: FunctionReturn,
    pub body: String,
    pub security: SecurityMode,
    pub volatility: Volatility,
}

/// Function parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionParam {
    pub name: String,
    pub data_type: DataType,
    pub default: Option<Expr>,
}

/// Function return type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FunctionReturn {
    Scalar { data_type: DataType },
    Table { columns: Vec<(String, DataType)> },
    SetOf { data_type: DataType },
    Void,
}

/// Security mode for functions.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum SecurityMode {
    #[default]
    Definer,
    Invoker,
}

/// Function volatility.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Volatility {
    #[default]
    Volatile,
    Stable,
    Immutable,
}

/// Policy statement (Reactor extension).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyStmt {
    pub name: String,
    pub table: String,
    pub scopes: Vec<PolicyScope>,
    pub using: Option<Expr>,
    pub check: Option<Expr>,
}

/// Policy scope.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyScope {
    Select,
    Insert,
    Update,
    Delete,
}

impl PolicyScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// Expression (simplified for policies and defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    /// Column reference.
    Column { name: String },
    /// Qualified column reference.
    QualifiedColumn { table: String, column: String },
    /// Literal value.
    Literal(Literal),
    /// Binary operation.
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    /// Unary operation.
    UnaryOp { op: UnaryOperator, expr: Box<Expr> },
    /// Function call.
    FunctionCall { name: String, args: Vec<Expr> },
    /// IS NULL / IS NOT NULL.
    IsNull { expr: Box<Expr>, negated: bool },
    /// IN (list).
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
    /// Subquery (limited to same-table).
    Subquery { sql: String },
    /// Raw SQL (for complex defaults).
    Raw { sql: String },
}

/// Literal values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BinaryOperator {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Like,
    ILike,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Minus,
}
