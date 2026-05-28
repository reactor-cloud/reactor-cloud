//! PostgREST-compatible query parsing.
//!
//! Parses query strings into a QueryPlan that can be executed against the database.
//!
//! Supported features:
//! - Filter operators: eq, neq, gt, gte, lt, lte, like, ilike, in, is, not, cs, cd, ov, and, or
//! - Select projections: ?select=col1,col2,*
//! - Ordering: ?order=col.desc.nullsfirst
//! - Pagination: ?limit=N&offset=M, Range header
//! - Prefer header: return, count, resolution

mod filter;
mod order;
mod pagination;
mod plan;
mod prefer;
mod select;

pub use filter::{parse_filter, FilterExpr, FilterOp, FilterValue};
pub use order::{parse_order, NullsPosition, OrderColumn, OrderDirection};
pub use pagination::{parse_pagination, Pagination};
pub use plan::QueryPlan;
pub use prefer::{parse_prefer, CountMode, Prefer, Resolution, ReturnMode};
pub use select::{parse_select, EmbedSpec, SelectColumn};

use crate::error::DataError;
use axum::http::HeaderMap;
use std::collections::HashMap;

/// Parse a full query from query parameters and headers.
pub fn parse_query(
    params: &HashMap<String, String>,
    headers: &HeaderMap,
    config: &crate::DataConfig,
) -> Result<QueryPlan, DataError> {
    // Parse select
    let select = match params.get("select") {
        Some(s) => parse_select(s)?,
        None => vec![SelectColumn::All],
    };

    // Parse filters
    let mut filters = Vec::new();
    for (key, value) in params {
        // Skip non-filter params
        if matches!(key.as_str(), "select" | "order" | "limit" | "offset") {
            continue;
        }
        let filter = parse_filter(key, value)?;
        filters.push(filter);
    }

    // Parse order
    let order = match params.get("order") {
        Some(o) => parse_order(o)?,
        None => vec![],
    };

    // Parse pagination
    let pagination = parse_pagination(params, headers, config)?;

    // Parse prefer header
    let prefer = match headers.get("prefer") {
        Some(v) => match v.to_str() {
            Ok(s) => parse_prefer(s)?,
            Err(_) => Prefer::default(),
        },
        None => Prefer::default(),
    };

    Ok(QueryPlan {
        select,
        filters,
        order,
        pagination,
        prefer,
        policy_predicate: None,
    })
}
