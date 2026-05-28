//! PostgREST pagination parsing.
//!
//! Parses pagination from `?limit=N&offset=M` and `Range` header.

use crate::error::DataError;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pagination parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pagination {
    /// Maximum number of rows to return.
    pub limit: u32,
    /// Number of rows to skip.
    pub offset: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

/// Parse pagination from query parameters and headers.
pub fn parse_pagination(
    params: &HashMap<String, String>,
    headers: &HeaderMap,
    config: &crate::DataConfig,
) -> Result<Pagination, DataError> {
    // Try Range header first
    if let Some(range) = parse_range_header(headers)? {
        return Ok(clamp_pagination(range, config));
    }

    // Fall back to query params
    let limit = match params.get("limit") {
        Some(s) => s
            .parse()
            .map_err(|_| DataError::InvalidFilter(format!("invalid limit value: {}", s)))?,
        None => config.default_limit,
    };

    let offset = match params.get("offset") {
        Some(s) => s
            .parse()
            .map_err(|_| DataError::InvalidFilter(format!("invalid offset value: {}", s)))?,
        None => 0,
    };

    Ok(clamp_pagination(Pagination { limit, offset }, config))
}

/// Parse the Range header.
///
/// Format: `items=0-24` (returns first 25 items)
fn parse_range_header(headers: &HeaderMap) -> Result<Option<Pagination>, DataError> {
    let range_value = match headers.get("range") {
        Some(v) => v,
        None => return Ok(None),
    };

    let range_str = range_value
        .to_str()
        .map_err(|_| DataError::InvalidFilter("invalid Range header".to_string()))?;

    // Check Range-Unit header if present
    if let Some(unit) = headers.get("range-unit") {
        if let Ok(unit_str) = unit.to_str() {
            if !unit_str.eq_ignore_ascii_case("items") {
                return Err(DataError::InvalidFilter(format!(
                    "unsupported Range-Unit: {}",
                    unit_str
                )));
            }
        }
    }

    // Parse format: items=N-M or N-M
    let range_part = range_str.strip_prefix("items=").unwrap_or(range_str);

    let parts: Vec<&str> = range_part.split('-').collect();
    if parts.len() != 2 {
        return Err(DataError::InvalidFilter(format!(
            "invalid Range format: {}",
            range_str
        )));
    }

    let start: u32 = parts[0]
        .parse()
        .map_err(|_| DataError::InvalidFilter(format!("invalid Range start: {}", parts[0])))?;

    let end: u32 = parts[1]
        .parse()
        .map_err(|_| DataError::InvalidFilter(format!("invalid Range end: {}", parts[1])))?;

    if end < start {
        return Err(DataError::InvalidFilter(
            "Range end must be >= start".to_string(),
        ));
    }

    Ok(Some(Pagination {
        offset: start,
        limit: end - start + 1,
    }))
}

/// Clamp pagination values to configured limits.
fn clamp_pagination(mut pagination: Pagination, config: &crate::DataConfig) -> Pagination {
    if pagination.limit > config.max_limit {
        pagination.limit = config.max_limit;
    }
    pagination
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn test_config() -> crate::DataConfig {
        crate::DataConfig {
            database_url: "".to_string(),
            bind: "0.0.0.0:8002".parse().unwrap(),
            migrations_dir: None,
            run_migrations: false,
            user_schema: "public".to_string(),
            max_embed_depth: 5,
            max_limit: 1000,
            default_limit: 100,
            deployment: crate::Deployment::Monolith,
            auth_url: None,
            internal_secret: None,
            auth_database_url: Some("".to_string()),
            auth_data_key: Some("test".to_string()),
            log: "info".to_string(),
            metrics: false,
        }
    }

    #[test]
    fn test_defaults() {
        let config = test_config();
        let params = HashMap::new();
        let headers = HeaderMap::new();

        let p = parse_pagination(&params, &headers, &config).unwrap();
        assert_eq!(p.limit, 100);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn test_query_params() {
        let config = test_config();
        let mut params = HashMap::new();
        params.insert("limit".to_string(), "50".to_string());
        params.insert("offset".to_string(), "10".to_string());
        let headers = HeaderMap::new();

        let p = parse_pagination(&params, &headers, &config).unwrap();
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 10);
    }

    #[test]
    fn test_range_header() {
        let config = test_config();
        let params = HashMap::new();
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_static("0-24"));

        let p = parse_pagination(&params, &headers, &config).unwrap();
        assert_eq!(p.limit, 25);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn test_range_header_with_items() {
        let config = test_config();
        let params = HashMap::new();
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_static("items=10-19"));

        let p = parse_pagination(&params, &headers, &config).unwrap();
        assert_eq!(p.limit, 10);
        assert_eq!(p.offset, 10);
    }

    #[test]
    fn test_limit_clamped() {
        let config = test_config();
        let mut params = HashMap::new();
        params.insert("limit".to_string(), "5000".to_string());
        let headers = HeaderMap::new();

        let p = parse_pagination(&params, &headers, &config).unwrap();
        assert_eq!(p.limit, 1000); // Clamped to max_limit
    }
}
