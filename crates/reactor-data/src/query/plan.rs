//! Query plan representation.
//!
//! The QueryPlan is the parsed representation of a PostgREST-style query
//! that can be executed against the database.

use super::filter::FilterExpr;
use super::order::OrderColumn;
use super::pagination::Pagination;
use super::prefer::Prefer;
use super::select::SelectColumn;
use serde::{Deserialize, Serialize};

/// A parsed query plan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryPlan {
    /// Selected columns.
    pub select: Vec<SelectColumn>,
    /// Filter expressions.
    pub filters: Vec<FilterExpr>,
    /// Order specifications.
    pub order: Vec<OrderColumn>,
    /// Pagination parameters.
    pub pagination: Pagination,
    /// Prefer header options.
    pub prefer: Prefer,
    /// Policy predicate (added in PR 10).
    pub policy_predicate: Option<String>,
}

impl QueryPlan {
    /// Create a new empty query plan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this query selects all columns.
    pub fn selects_all(&self) -> bool {
        self.select.iter().any(|s| matches!(s, SelectColumn::All))
    }

    /// Get the list of selected column names (excluding embeds).
    pub fn selected_columns(&self) -> Vec<&str> {
        self.select
            .iter()
            .filter_map(|s| match s {
                SelectColumn::Column(name) => Some(name.as_str()),
                SelectColumn::Aliased { column, .. } => Some(column.as_str()),
                SelectColumn::All | SelectColumn::Embed(_) => None,
            })
            .collect()
    }

    /// Get the list of embedded resources.
    pub fn embeds(&self) -> Vec<&crate::query::select::EmbedSpec> {
        self.select
            .iter()
            .filter_map(|s| match s {
                SelectColumn::Embed(spec) => Some(spec),
                _ => None,
            })
            .collect()
    }

    /// Check if this plan has any embedded resources.
    pub fn has_embeds(&self) -> bool {
        self.select.iter().any(|s| matches!(s, SelectColumn::Embed(_)))
    }

    /// Get the list of filtered column names.
    pub fn filtered_columns(&self) -> Vec<&str> {
        self.filters.iter().map(|f| f.column.as_str()).collect()
    }

    /// Get the list of ordered column names.
    pub fn ordered_columns(&self) -> Vec<&str> {
        self.order.iter().map(|o| o.column.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selects_all() {
        let plan = QueryPlan {
            select: vec![SelectColumn::All],
            ..Default::default()
        };
        assert!(plan.selects_all());

        let plan = QueryPlan {
            select: vec![SelectColumn::Column("id".to_string())],
            ..Default::default()
        };
        assert!(!plan.selects_all());
    }

    #[test]
    fn test_selected_columns() {
        let plan = QueryPlan {
            select: vec![
                SelectColumn::Column("id".to_string()),
                SelectColumn::Aliased {
                    alias: "user_name".to_string(),
                    column: "name".to_string(),
                },
            ],
            ..Default::default()
        };

        let cols = plan.selected_columns();
        assert_eq!(cols, vec!["id", "name"]);
    }
}
