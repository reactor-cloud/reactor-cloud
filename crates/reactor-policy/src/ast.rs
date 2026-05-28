//! Policy expression AST.
//!
//! This AST represents the boolean expressions used in policy `using` and `check` clauses.
//! It is designed to be serializable to JSON for storage in metadata tables.

use serde::{Deserialize, Serialize};

/// A policy expression node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyExpr {
    pub kind: PolicyExprKind,
}

impl PolicyExpr {
    pub fn new(kind: PolicyExprKind) -> Self {
        Self { kind }
    }

    /// Create a column reference.
    pub fn column(name: impl Into<String>) -> Self {
        Self::new(PolicyExprKind::Column { name: name.into() })
    }

    /// Create a qualified column reference.
    pub fn qualified_column(table: impl Into<String>, column: impl Into<String>) -> Self {
        Self::new(PolicyExprKind::QualifiedColumn {
            table: table.into(),
            column: column.into(),
        })
    }

    /// Create a literal value.
    pub fn literal(value: PolicyLiteral) -> Self {
        Self::new(PolicyExprKind::Literal(value))
    }

    /// Create a binary operation.
    pub fn binary(left: PolicyExpr, op: PolicyBinaryOp, right: PolicyExpr) -> Self {
        Self::new(PolicyExprKind::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        })
    }

    /// Create a unary operation.
    pub fn unary(op: PolicyUnaryOp, expr: PolicyExpr) -> Self {
        Self::new(PolicyExprKind::UnaryOp {
            op,
            expr: Box::new(expr),
        })
    }

    /// Create an auth builtin call.
    pub fn auth_builtin(name: impl Into<String>, args: Vec<PolicyExpr>) -> Self {
        Self::new(PolicyExprKind::AuthBuiltin {
            name: name.into(),
            args,
        })
    }

    /// Create a domain-specific builtin call (e.g., object.key, bucket.name).
    pub fn domain_builtin(domain: impl Into<String>, name: impl Into<String>, args: Vec<PolicyExpr>) -> Self {
        Self::new(PolicyExprKind::DomainBuiltin {
            domain: domain.into(),
            name: name.into(),
            args,
        })
    }

    /// Create an IS NULL check.
    pub fn is_null(expr: PolicyExpr, negated: bool) -> Self {
        Self::new(PolicyExprKind::IsNull {
            expr: Box::new(expr),
            negated,
        })
    }

    /// Create an IN list check.
    pub fn in_list(expr: PolicyExpr, list: Vec<PolicyExpr>, negated: bool) -> Self {
        Self::new(PolicyExprKind::InList {
            expr: Box::new(expr),
            list,
            negated,
        })
    }

    /// Create a boolean literal.
    pub fn bool_literal(value: bool) -> Self {
        Self::literal(PolicyLiteral::Bool(value))
    }

    /// Create a string literal.
    pub fn string_literal(value: impl Into<String>) -> Self {
        Self::literal(PolicyLiteral::String(value.into()))
    }

    /// Create a number literal.
    pub fn number_literal(value: i64) -> Self {
        Self::literal(PolicyLiteral::Int(value))
    }

    /// Create an AND expression.
    pub fn and(left: PolicyExpr, right: PolicyExpr) -> Self {
        Self::binary(left, PolicyBinaryOp::And, right)
    }

    /// Create an OR expression.
    pub fn or(left: PolicyExpr, right: PolicyExpr) -> Self {
        Self::binary(left, PolicyBinaryOp::Or, right)
    }

    /// Create a NOT expression.
    pub fn not(expr: PolicyExpr) -> Self {
        Self::unary(PolicyUnaryOp::Not, expr)
    }
}

/// The kind of policy expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PolicyExprKind {
    /// Column reference (unqualified).
    Column { name: String },

    /// Qualified column reference (table.column).
    QualifiedColumn { table: String, column: String },

    /// Literal value.
    Literal(PolicyLiteral),

    /// Binary operation.
    BinaryOp {
        left: Box<PolicyExpr>,
        op: PolicyBinaryOp,
        right: Box<PolicyExpr>,
    },

    /// Unary operation.
    UnaryOp {
        op: PolicyUnaryOp,
        expr: Box<PolicyExpr>,
    },

    /// Auth builtin function call (e.g., `auth.user_id()`, `auth.has_permission('read')`).
    AuthBuiltin { name: String, args: Vec<PolicyExpr> },

    /// Domain-specific builtin (e.g., `object.key`, `bucket.name` for storage).
    DomainBuiltin {
        domain: String,
        name: String,
        args: Vec<PolicyExpr>,
    },

    /// IS NULL / IS NOT NULL.
    IsNull {
        expr: Box<PolicyExpr>,
        negated: bool,
    },

    /// IN (list).
    InList {
        expr: Box<PolicyExpr>,
        list: Vec<PolicyExpr>,
        negated: bool,
    },

    /// Same-table subquery (limited scope).
    Subquery { sql: String },
}

/// Literal values in policy expressions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "literal_type", content = "value")]
pub enum PolicyLiteral {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl From<bool> for PolicyLiteral {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for PolicyLiteral {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<f64> for PolicyLiteral {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<String> for PolicyLiteral {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for PolicyLiteral {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

/// Binary operators in policy expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyBinaryOp {
    // Comparison
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // Logical
    And,
    Or,

    // Pattern matching
    Like,
    ILike,
}

impl PolicyBinaryOp {
    /// Get the SQL representation of this operator.
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::NotEq => "<>",
            Self::Lt => "<",
            Self::LtEq => "<=",
            Self::Gt => ">",
            Self::GtEq => ">=",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Like => "LIKE",
            Self::ILike => "ILIKE",
        }
    }

    /// Operator precedence (higher binds tighter).
    pub fn precedence(&self) -> u8 {
        match self {
            Self::Or => 1,
            Self::And => 2,
            Self::Eq | Self::NotEq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq => 3,
            Self::Like | Self::ILike => 4,
        }
    }
}

/// Unary operators in policy expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyUnaryOp {
    Not,
    Minus,
}

impl PolicyUnaryOp {
    /// Get the SQL representation of this operator.
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::Not => "NOT",
            Self::Minus => "-",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let expr = PolicyExpr::binary(
            PolicyExpr::column("org_id"),
            PolicyBinaryOp::Eq,
            PolicyExpr::auth_builtin("org_id", vec![]),
        );

        let json = serde_json::to_string(&expr).unwrap();
        let parsed: PolicyExpr = serde_json::from_str(&json).unwrap();

        assert_eq!(expr, parsed);
    }

    #[test]
    fn test_complex_expression() {
        // (org_id = auth.org_id()) AND (status = 'active' OR auth.has_permission('admin'))
        let expr = PolicyExpr::binary(
            PolicyExpr::binary(
                PolicyExpr::column("org_id"),
                PolicyBinaryOp::Eq,
                PolicyExpr::auth_builtin("org_id", vec![]),
            ),
            PolicyBinaryOp::And,
            PolicyExpr::binary(
                PolicyExpr::binary(
                    PolicyExpr::column("status"),
                    PolicyBinaryOp::Eq,
                    PolicyExpr::literal(PolicyLiteral::String("active".to_string())),
                ),
                PolicyBinaryOp::Or,
                PolicyExpr::auth_builtin(
                    "has_permission",
                    vec![PolicyExpr::literal(PolicyLiteral::String(
                        "admin".to_string(),
                    ))],
                ),
            ),
        );

        let json = serde_json::to_string_pretty(&expr).unwrap();
        let parsed: PolicyExpr = serde_json::from_str(&json).unwrap();
        assert_eq!(expr, parsed);
    }

    #[test]
    fn test_domain_builtin() {
        let expr = PolicyExpr::domain_builtin("object", "key", vec![]);
        match &expr.kind {
            PolicyExprKind::DomainBuiltin { domain, name, args } => {
                assert_eq!(domain, "object");
                assert_eq!(name, "key");
                assert!(args.is_empty());
            }
            _ => panic!("expected DomainBuiltin"),
        }
    }
}
