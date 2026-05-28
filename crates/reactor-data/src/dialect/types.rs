//! Type mappings between Reactor types and backend-specific types.

use super::DataType;

/// Reactor's portable type.
pub type ReactorType = DataType;

/// Type mapping for a specific backend.
pub struct TypeMapping {
    /// Backend name.
    pub backend: &'static str,
}

impl TypeMapping {
    /// Postgres type mapping.
    pub const POSTGRES: Self = Self {
        backend: "postgres",
    };

    /// SQLite type mapping (v0.2).
    #[allow(dead_code)]
    pub const SQLITE: Self = Self { backend: "sqlite" };

    /// Map a Reactor type to the backend's native SQL type.
    pub fn to_sql(&self, ty: &DataType) -> &'static str {
        match self.backend {
            "postgres" => Self::postgres_type(ty),
            "sqlite" => Self::sqlite_type(ty),
            _ => panic!("unknown backend: {}", self.backend),
        }
    }

    fn postgres_type(ty: &DataType) -> &'static str {
        match ty {
            DataType::ReactorId => "UUID",
            DataType::Text => "TEXT",
            DataType::Bool => "BOOLEAN",
            DataType::Int => "INTEGER",
            DataType::BigInt => "BIGINT",
            DataType::Float => "DOUBLE PRECISION",
            DataType::TimestampTz => "TIMESTAMPTZ",
            DataType::Jsonb => "JSONB",
            DataType::Bytea => "BYTEA",
        }
    }

    fn sqlite_type(ty: &DataType) -> &'static str {
        match ty {
            DataType::ReactorId => "TEXT",
            DataType::Text => "TEXT",
            DataType::Bool => "INTEGER",
            DataType::Int => "INTEGER",
            DataType::BigInt => "INTEGER",
            DataType::Float => "REAL",
            DataType::TimestampTz => "TEXT",
            DataType::Jsonb => "TEXT",
            DataType::Bytea => "BLOB",
        }
    }
}
