//! PostgREST order parsing.
//!
//! Parses order specifications like `?order=col.desc.nullsfirst`.

use crate::error::DataError;
use serde::{Deserialize, Serialize};

/// Order direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OrderDirection {
    #[default]
    Asc,
    Desc,
}

impl OrderDirection {
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

/// Nulls position in ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NullsPosition {
    First,
    Last,
}

impl NullsPosition {
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::First => "NULLS FIRST",
            Self::Last => "NULLS LAST",
        }
    }
}

/// An order column specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderColumn {
    pub column: String,
    pub direction: OrderDirection,
    pub nulls: Option<NullsPosition>,
}

/// Parse an order string.
///
/// Format: `col.desc.nullsfirst,col2.asc.nullslast`
pub fn parse_order(input: &str) -> Result<Vec<OrderColumn>, DataError> {
    if input.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut orders = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let segments: Vec<&str> = part.split('.').collect();
        if segments.is_empty() {
            continue;
        }

        let column = segments[0].to_string();
        let mut direction = OrderDirection::Asc;
        let mut nulls = None;

        for segment in segments.iter().skip(1) {
            let seg_lower = segment.to_lowercase();
            match seg_lower.as_str() {
                "asc" => direction = OrderDirection::Asc,
                "desc" => direction = OrderDirection::Desc,
                "nullsfirst" => nulls = Some(NullsPosition::First),
                "nullslast" => nulls = Some(NullsPosition::Last),
                _ => {
                    return Err(DataError::InvalidFilter(format!(
                        "unknown order modifier: {}",
                        segment
                    )))
                }
            }
        }

        orders.push(OrderColumn {
            column,
            direction,
            nulls,
        });
    }

    Ok(orders)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let orders = parse_order("name").unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].column, "name");
        assert_eq!(orders[0].direction, OrderDirection::Asc);
        assert_eq!(orders[0].nulls, None);
    }

    #[test]
    fn test_parse_desc() {
        let orders = parse_order("created_at.desc").unwrap();
        assert_eq!(orders[0].direction, OrderDirection::Desc);
    }

    #[test]
    fn test_parse_nullsfirst() {
        let orders = parse_order("name.asc.nullsfirst").unwrap();
        assert_eq!(orders[0].direction, OrderDirection::Asc);
        assert_eq!(orders[0].nulls, Some(NullsPosition::First));
    }

    #[test]
    fn test_parse_multiple() {
        let orders = parse_order("priority.desc,created_at.asc.nullslast").unwrap();
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].column, "priority");
        assert_eq!(orders[0].direction, OrderDirection::Desc);
        assert_eq!(orders[1].column, "created_at");
        assert_eq!(orders[1].nulls, Some(NullsPosition::Last));
    }

    #[test]
    fn test_parse_empty() {
        let orders = parse_order("").unwrap();
        assert!(orders.is_empty());
    }
}
