//! PostgREST Prefer header parsing.
//!
//! Parses the Prefer header for return mode, count mode, and resolution.

use crate::error::DataError;
use serde::{Deserialize, Serialize};

/// Return mode for mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReturnMode {
    /// Return the affected rows.
    Representation,
    /// Return nothing (default, per PostgREST behavior).
    #[default]
    Minimal,
    /// Return only headers.
    HeadersOnly,
}

/// Count mode for queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CountMode {
    /// No count.
    #[default]
    None,
    /// Exact count.
    Exact,
    /// Planned count (from query plan).
    Planned,
    /// Estimated count.
    Estimated,
}

/// Resolution mode for upserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Resolution {
    /// Merge duplicates (update on conflict).
    #[default]
    MergeDuplicates,
    /// Ignore duplicates (skip on conflict).
    IgnoreDuplicates,
}

/// Parsed Prefer header.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prefer {
    pub return_mode: ReturnMode,
    pub count: CountMode,
    pub resolution: Resolution,
    pub missing: Option<MissingMode>,
}

/// Handling of missing columns in upserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingMode {
    /// Use defaults for missing columns.
    Default,
}

/// Parse the Prefer header value.
pub fn parse_prefer(input: &str) -> Result<Prefer, DataError> {
    let mut prefer = Prefer::default();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Split on '=' if present
        let (key, value) = match part.find('=') {
            Some(pos) => (&part[..pos], Some(&part[pos + 1..])),
            None => (part, None),
        };

        match key.to_lowercase().as_str() {
            "return" => {
                let mode = value.ok_or_else(|| {
                    DataError::InvalidFilter("Prefer: return requires a value".to_string())
                })?;
                prefer.return_mode = match mode.to_lowercase().as_str() {
                    "representation" => ReturnMode::Representation,
                    "minimal" => ReturnMode::Minimal,
                    "headers-only" => ReturnMode::HeadersOnly,
                    _ => {
                        return Err(DataError::InvalidFilter(format!(
                            "unknown return mode: {}",
                            mode
                        )))
                    }
                };
            }
            "count" => {
                let mode = value.ok_or_else(|| {
                    DataError::InvalidFilter("Prefer: count requires a value".to_string())
                })?;
                prefer.count = match mode.to_lowercase().as_str() {
                    "exact" => CountMode::Exact,
                    "planned" => CountMode::Planned,
                    "estimated" => CountMode::Estimated,
                    "none" => CountMode::None,
                    _ => {
                        return Err(DataError::InvalidFilter(format!(
                            "unknown count mode: {}",
                            mode
                        )))
                    }
                };
            }
            "resolution" => {
                let mode = value.ok_or_else(|| {
                    DataError::InvalidFilter("Prefer: resolution requires a value".to_string())
                })?;
                prefer.resolution = match mode.to_lowercase().as_str() {
                    "merge-duplicates" => Resolution::MergeDuplicates,
                    "ignore-duplicates" => Resolution::IgnoreDuplicates,
                    _ => {
                        return Err(DataError::InvalidFilter(format!(
                            "unknown resolution mode: {}",
                            mode
                        )))
                    }
                };
            }
            "missing" => {
                let mode = value.ok_or_else(|| {
                    DataError::InvalidFilter("Prefer: missing requires a value".to_string())
                })?;
                prefer.missing = match mode.to_lowercase().as_str() {
                    "default" => Some(MissingMode::Default),
                    _ => {
                        return Err(DataError::InvalidFilter(format!(
                            "unknown missing mode: {}",
                            mode
                        )))
                    }
                };
            }
            _ => {
                // Unknown preferences are ignored (per HTTP spec)
            }
        }
    }

    Ok(prefer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let p = parse_prefer("").unwrap();
        assert_eq!(p, Prefer::default());
    }

    #[test]
    fn test_parse_return() {
        let p = parse_prefer("return=representation").unwrap();
        assert_eq!(p.return_mode, ReturnMode::Representation);

        let p = parse_prefer("return=minimal").unwrap();
        assert_eq!(p.return_mode, ReturnMode::Minimal);
    }

    #[test]
    fn test_parse_count() {
        let p = parse_prefer("count=exact").unwrap();
        assert_eq!(p.count, CountMode::Exact);

        let p = parse_prefer("count=planned").unwrap();
        assert_eq!(p.count, CountMode::Planned);
    }

    #[test]
    fn test_parse_resolution() {
        let p = parse_prefer("resolution=merge-duplicates").unwrap();
        assert_eq!(p.resolution, Resolution::MergeDuplicates);

        let p = parse_prefer("resolution=ignore-duplicates").unwrap();
        assert_eq!(p.resolution, Resolution::IgnoreDuplicates);
    }

    #[test]
    fn test_parse_multiple() {
        let p = parse_prefer("return=representation, count=exact").unwrap();
        assert_eq!(p.return_mode, ReturnMode::Representation);
        assert_eq!(p.count, CountMode::Exact);
    }

    #[test]
    fn test_unknown_preference_ignored() {
        let p = parse_prefer("unknown=value, return=minimal").unwrap();
        assert_eq!(p.return_mode, ReturnMode::Minimal);
    }
}
