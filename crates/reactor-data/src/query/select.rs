//! PostgREST select parsing.
//!
//! Parses select projections like `?select=col1,col2,*` and embedded resources
//! like `?select=id,author(id,name),comments(body,user(name))`.

use crate::error::DataError;
use serde::{Deserialize, Serialize};

/// A select column specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectColumn {
    /// All columns.
    All,
    /// A specific column.
    Column(String),
    /// An aliased column: `alias:column`.
    Aliased { alias: String, column: String },
    /// An embedded resource with nested columns.
    Embed(EmbedSpec),
}

/// Specification for an embedded resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedSpec {
    /// The name of the embedded resource (usually a table name or FK hint).
    pub name: String,
    /// Optional FK hint for disambiguation (e.g., `posts_author_id_fkey`).
    pub fk_hint: Option<String>,
    /// Columns to select from the embedded resource.
    pub columns: Vec<SelectColumn>,
    /// Optional alias for the embed.
    pub alias: Option<String>,
}

impl SelectColumn {
    /// Get the output column name (alias or column name).
    pub fn output_name(&self) -> &str {
        match self {
            Self::All => "*",
            Self::Column(name) => name,
            Self::Aliased { alias, .. } => alias,
            Self::Embed(spec) => spec.alias.as_deref().unwrap_or(&spec.name),
        }
    }

    /// Get the source column name.
    pub fn source_name(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Column(name) => Some(name),
            Self::Aliased { column, .. } => Some(column),
            Self::Embed(_) => None,
        }
    }

    /// Check if this is an embedded resource.
    pub fn is_embed(&self) -> bool {
        matches!(self, Self::Embed(_))
    }
}

/// Parse a select string.
///
/// Format: `col1,col2,alias:col3,*,embed(col1,col2),alias:embed!hint(col1)`
pub fn parse_select(input: &str) -> Result<Vec<SelectColumn>, DataError> {
    parse_select_items(input)
}

/// Parse select items, handling nested parentheses for embeds.
fn parse_select_items(input: &str) -> Result<Vec<SelectColumn>, DataError> {
    if input.trim().is_empty() {
        return Ok(vec![SelectColumn::All]);
    }

    let mut columns = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0;

    for ch in input.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth -= 1;
                current.push(ch);
            }
            ',' if paren_depth == 0 => {
                // Top-level comma: end of current item
                if !current.trim().is_empty() {
                    columns.push(parse_single_select_item(&current)?);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    // Handle last item
    if !current.trim().is_empty() {
        columns.push(parse_single_select_item(&current)?);
    }

    if columns.is_empty() {
        columns.push(SelectColumn::All);
    }

    Ok(columns)
}

/// Parse a single select item (column, aliased column, or embed).
fn parse_single_select_item(item: &str) -> Result<SelectColumn, DataError> {
    let item = item.trim();

    if item == "*" {
        return Ok(SelectColumn::All);
    }

    // Check if it's an embed: contains '(' and ')'
    if let Some(paren_start) = item.find('(') {
        if !item.ends_with(')') {
            return Err(DataError::InvalidFilter(format!(
                "malformed embed: missing closing parenthesis in '{}'",
                item
            )));
        }

        let name_part = &item[..paren_start];
        let inner = &item[paren_start + 1..item.len() - 1];

        // Parse name part: could be "name", "alias:name", or "name!hint", or "alias:name!hint"
        let (alias, name_with_hint) = if let Some(colon_pos) = name_part.find(':') {
            (
                Some(name_part[..colon_pos].trim().to_string()),
                name_part[colon_pos + 1..].trim(),
            )
        } else {
            (None, name_part.trim())
        };

        let (name, fk_hint) = if let Some(bang_pos) = name_with_hint.find('!') {
            (
                name_with_hint[..bang_pos].to_string(),
                Some(name_with_hint[bang_pos + 1..].to_string()),
            )
        } else {
            (name_with_hint.to_string(), None)
        };

        if name.is_empty() {
            return Err(DataError::InvalidFilter(format!(
                "empty embed name in '{}'",
                item
            )));
        }

        // Recursively parse inner columns
        let inner_columns = parse_select_items(inner)?;

        return Ok(SelectColumn::Embed(EmbedSpec {
            name,
            fk_hint,
            columns: inner_columns,
            alias,
        }));
    }

    // Check for alias: "alias:column"
    if let Some(colon_pos) = item.find(':') {
        let alias = item[..colon_pos].trim();
        let column = item[colon_pos + 1..].trim();

        if alias.is_empty() || column.is_empty() {
            return Err(DataError::InvalidFilter(format!(
                "invalid aliased column: {}",
                item
            )));
        }

        return Ok(SelectColumn::Aliased {
            alias: alias.to_string(),
            column: column.to_string(),
        });
    }

    // Simple column
    Ok(SelectColumn::Column(item.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_star() {
        let cols = parse_select("*").unwrap();
        assert_eq!(cols, vec![SelectColumn::All]);
    }

    #[test]
    fn test_parse_single_column() {
        let cols = parse_select("name").unwrap();
        assert_eq!(cols, vec![SelectColumn::Column("name".to_string())]);
    }

    #[test]
    fn test_parse_multiple_columns() {
        let cols = parse_select("id,name,email").unwrap();
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0], SelectColumn::Column("id".to_string()));
        assert_eq!(cols[1], SelectColumn::Column("name".to_string()));
        assert_eq!(cols[2], SelectColumn::Column("email".to_string()));
    }

    #[test]
    fn test_parse_aliased() {
        let cols = parse_select("user_name:name").unwrap();
        assert_eq!(
            cols,
            vec![SelectColumn::Aliased {
                alias: "user_name".to_string(),
                column: "name".to_string()
            }]
        );
    }

    #[test]
    fn test_parse_mixed() {
        let cols = parse_select("id,user_name:name,*").unwrap();
        assert_eq!(cols.len(), 3);
    }

    #[test]
    fn test_parse_empty() {
        let cols = parse_select("").unwrap();
        assert_eq!(cols, vec![SelectColumn::All]);
    }

    // Embed tests

    #[test]
    fn test_parse_simple_embed() {
        let cols = parse_select("id,author(id,name)").unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0], SelectColumn::Column("id".to_string()));

        if let SelectColumn::Embed(spec) = &cols[1] {
            assert_eq!(spec.name, "author");
            assert_eq!(spec.fk_hint, None);
            assert_eq!(spec.alias, None);
            assert_eq!(spec.columns.len(), 2);
        } else {
            panic!("Expected Embed variant");
        }
    }

    #[test]
    fn test_parse_nested_embed() {
        let cols = parse_select("id,comments(body,author(name))").unwrap();
        assert_eq!(cols.len(), 2);

        if let SelectColumn::Embed(spec) = &cols[1] {
            assert_eq!(spec.name, "comments");
            assert_eq!(spec.columns.len(), 2);

            if let SelectColumn::Embed(inner) = &spec.columns[1] {
                assert_eq!(inner.name, "author");
                assert_eq!(inner.columns.len(), 1);
            } else {
                panic!("Expected nested Embed variant");
            }
        } else {
            panic!("Expected Embed variant");
        }
    }

    #[test]
    fn test_parse_embed_with_fk_hint() {
        let cols = parse_select("id,author!posts_author_id_fkey(id,name)").unwrap();

        if let SelectColumn::Embed(spec) = &cols[1] {
            assert_eq!(spec.name, "author");
            assert_eq!(spec.fk_hint, Some("posts_author_id_fkey".to_string()));
        } else {
            panic!("Expected Embed variant");
        }
    }

    #[test]
    fn test_parse_embed_with_alias() {
        let cols = parse_select("id,writer:author(id,name)").unwrap();

        if let SelectColumn::Embed(spec) = &cols[1] {
            assert_eq!(spec.name, "author");
            assert_eq!(spec.alias, Some("writer".to_string()));
        } else {
            panic!("Expected Embed variant");
        }
    }

    #[test]
    fn test_parse_embed_with_alias_and_hint() {
        let cols = parse_select("id,writer:author!posts_author_fkey(id,name)").unwrap();

        if let SelectColumn::Embed(spec) = &cols[1] {
            assert_eq!(spec.name, "author");
            assert_eq!(spec.alias, Some("writer".to_string()));
            assert_eq!(spec.fk_hint, Some("posts_author_fkey".to_string()));
        } else {
            panic!("Expected Embed variant");
        }
    }

    #[test]
    fn test_parse_embed_star() {
        let cols = parse_select("id,author(*)").unwrap();

        if let SelectColumn::Embed(spec) = &cols[1] {
            assert_eq!(spec.columns, vec![SelectColumn::All]);
        } else {
            panic!("Expected Embed variant");
        }
    }

    #[test]
    fn test_parse_multiple_embeds() {
        let cols = parse_select("id,author(name),comments(body)").unwrap();
        assert_eq!(cols.len(), 3);
        assert!(cols[1].is_embed());
        assert!(cols[2].is_embed());
    }
}
