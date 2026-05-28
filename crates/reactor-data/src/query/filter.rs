//! PostgREST filter parsing.
//!
//! Parses filter expressions like `col=eq.value` or `col=in.(a,b,c)`.

use crate::error::DataError;
use serde::{Deserialize, Serialize};

/// A parsed filter expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterExpr {
    pub column: String,
    pub op: FilterOp,
    pub value: FilterValue,
    pub negated: bool,
}

/// Filter operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    ILike,
    In,
    Is,
    Cs,  // Contains (array)
    Cd,  // Contained by (array)
    Ov,  // Overlaps (array)
    Fts, // Full-text search (deferred)
}

impl FilterOp {
    /// Parse operator from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "eq" => Some(Self::Eq),
            "neq" => Some(Self::Neq),
            "gt" => Some(Self::Gt),
            "gte" => Some(Self::Gte),
            "lt" => Some(Self::Lt),
            "lte" => Some(Self::Lte),
            "like" => Some(Self::Like),
            "ilike" => Some(Self::ILike),
            "in" => Some(Self::In),
            "is" => Some(Self::Is),
            "cs" => Some(Self::Cs),
            "cd" => Some(Self::Cd),
            "ov" => Some(Self::Ov),
            "fts" => Some(Self::Fts),
            _ => None,
        }
    }

    /// Get the SQL operator.
    pub fn to_sql(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Neq => "<>",
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Like => "LIKE",
            Self::ILike => "ILIKE",
            Self::In => "IN",
            Self::Is => "IS",
            Self::Cs => "@>",
            Self::Cd => "<@",
            Self::Ov => "&&",
            Self::Fts => "@@",
        }
    }
}

/// Filter value types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<FilterValue>),
}

impl FilterValue {
    /// Parse a value string.
    pub fn parse(s: &str) -> Self {
        // Null
        if s.eq_ignore_ascii_case("null") {
            return Self::Null;
        }

        // Bool
        if s.eq_ignore_ascii_case("true") {
            return Self::Bool(true);
        }
        if s.eq_ignore_ascii_case("false") {
            return Self::Bool(false);
        }

        // Try integer
        if let Ok(i) = s.parse::<i64>() {
            return Self::Int(i);
        }

        // Try float
        if let Ok(f) = s.parse::<f64>() {
            return Self::Float(f);
        }

        // Default to string
        Self::String(s.to_string())
    }

    /// Parse a list value like `(a,b,c)`.
    pub fn parse_list(s: &str) -> Result<Self, DataError> {
        let trimmed = s.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return Err(DataError::InvalidFilter(
                "list values must be wrapped in parentheses".to_string(),
            ));
        }

        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.is_empty() {
            return Ok(Self::List(vec![]));
        }

        let items: Vec<FilterValue> = inner.split(',').map(|s| Self::parse(s.trim())).collect();

        Ok(Self::List(items))
    }
}

/// Parse a filter from a query parameter.
///
/// Format: `column=op.value` or `column=not.op.value` or `column=in.(a,b,c)`
pub fn parse_filter(column: &str, value: &str) -> Result<FilterExpr, DataError> {
    let (negated, rest) = if let Some(stripped) = value.strip_prefix("not.") {
        (true, stripped)
    } else {
        (false, value)
    };

    // Find the operator
    let dot_pos = rest.find('.').ok_or_else(|| {
        DataError::InvalidFilter(format!("missing operator in filter: {}={}", column, value))
    })?;

    let op_str = &rest[..dot_pos];
    let value_str = &rest[dot_pos + 1..];

    let op = FilterOp::parse(op_str)
        .ok_or_else(|| DataError::InvalidFilter(format!("unknown filter operator: {}", op_str)))?;

    let filter_value = match op {
        FilterOp::In => FilterValue::parse_list(value_str)?,
        FilterOp::Is => {
            // IS only accepts null, true, false
            match value_str.to_lowercase().as_str() {
                "null" => FilterValue::Null,
                "true" => FilterValue::Bool(true),
                "false" => FilterValue::Bool(false),
                _ => {
                    return Err(DataError::InvalidFilter(format!(
                        "IS operator only accepts null, true, or false, got: {}",
                        value_str
                    )))
                }
            }
        }
        FilterOp::Cs | FilterOp::Cd | FilterOp::Ov => FilterValue::parse_list(value_str)?,
        _ => FilterValue::parse(value_str),
    };

    Ok(FilterExpr {
        column: column.to_string(),
        op,
        value: filter_value,
        negated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eq() {
        let f = parse_filter("name", "eq.John").unwrap();
        assert_eq!(f.column, "name");
        assert_eq!(f.op, FilterOp::Eq);
        assert!(!f.negated);
        assert_eq!(f.value, FilterValue::String("John".to_string()));
    }

    #[test]
    fn test_parse_neq() {
        let f = parse_filter("status", "neq.active").unwrap();
        assert_eq!(f.op, FilterOp::Neq);
    }

    #[test]
    fn test_parse_numeric() {
        let f = parse_filter("age", "gt.30").unwrap();
        assert_eq!(f.op, FilterOp::Gt);
        assert_eq!(f.value, FilterValue::Int(30));
    }

    #[test]
    fn test_parse_negated() {
        let f = parse_filter("name", "not.eq.John").unwrap();
        assert!(f.negated);
        assert_eq!(f.op, FilterOp::Eq);
    }

    #[test]
    fn test_parse_in() {
        let f = parse_filter("status", "in.(active,pending,done)").unwrap();
        assert_eq!(f.op, FilterOp::In);
        match f.value {
            FilterValue::List(items) => {
                assert_eq!(items.len(), 3);
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_parse_is_null() {
        let f = parse_filter("deleted_at", "is.null").unwrap();
        assert_eq!(f.op, FilterOp::Is);
        assert_eq!(f.value, FilterValue::Null);
    }

    #[test]
    fn test_parse_like() {
        let f = parse_filter("email", "like.*@example.com").unwrap();
        assert_eq!(f.op, FilterOp::Like);
        assert_eq!(f.value, FilterValue::String("*@example.com".to_string()));
    }
}
