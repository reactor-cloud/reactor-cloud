//! Embedded resource resolution and SQL generation.
//!
//! Handles nested resource embedding like `?select=id,author(name),comments(body,user(name))`.

use crate::error::DataError;
use crate::middleware::DataCtx;
use crate::query::EmbedSpec;
use crate::store::{ForeignKeyInfo, SchemaSnapshot, TableSnapshot};

/// Resolved embed with FK information.
#[derive(Debug, Clone)]
pub struct ResolvedEmbed {
    /// The embed specification from the query.
    pub spec: EmbedSpec,
    /// The foreign key used for the join.
    pub fk: ForeignKeyInfo,
    /// The referenced table.
    pub ref_table: TableSnapshot,
    /// Whether this is an "outward" embed (parent → child via FK on parent)
    /// or "inward" embed (child → parent via FK on child).
    pub direction: EmbedDirection,
    /// Nested embeds (recursively resolved).
    pub nested: Vec<ResolvedEmbed>,
    /// Unique alias for this embed in the generated SQL.
    pub sql_alias: String,
}

/// Direction of the embed relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedDirection {
    /// Parent table has FK pointing to child table (many-to-one).
    /// Example: posts.author_id → users.id
    /// Returns a single object.
    ToOne,
    /// Child table has FK pointing to parent table (one-to-many).
    /// Example: posts.id ← comments.post_id
    /// Returns an array.
    ToMany,
}

/// Resolve embeds against the schema.
pub fn resolve_embeds(
    embeds: &[&EmbedSpec],
    schema: &SchemaSnapshot,
    parent_table: &TableSnapshot,
    _ctx: &DataCtx,
    max_depth: usize,
    depth: usize,
) -> Result<Vec<ResolvedEmbed>, DataError> {
    if depth >= max_depth {
        return Err(DataError::InvalidQuery(format!(
            "maximum embed depth {} exceeded",
            max_depth
        )));
    }

    let mut resolved = Vec::new();
    let mut alias_counter = depth * 100; // Ensure unique aliases across nesting levels

    for embed in embeds {
        let (fk, ref_table, direction) =
            find_fk_for_embed(embed, schema, parent_table)?;

        alias_counter += 1;
        let sql_alias = format!("embed_{}_{}", embed.name, alias_counter);

        // Recursively resolve nested embeds
        let nested_specs: Vec<_> = embed
            .columns
            .iter()
            .filter_map(|c| match c {
                crate::query::SelectColumn::Embed(spec) => Some(spec),
                _ => None,
            })
            .collect();

        let nested = if nested_specs.is_empty() {
            vec![]
        } else {
            resolve_embeds(&nested_specs, schema, &ref_table, _ctx, max_depth, depth + 1)?
        };

        resolved.push(ResolvedEmbed {
            spec: (*embed).clone(),
            fk,
            ref_table,
            direction,
            nested,
            sql_alias,
        });
    }

    Ok(resolved)
}

/// Find the foreign key for an embed.
fn find_fk_for_embed(
    embed: &EmbedSpec,
    schema: &SchemaSnapshot,
    parent_table: &TableSnapshot,
) -> Result<(ForeignKeyInfo, TableSnapshot, EmbedDirection), DataError> {
    // If FK hint is provided, use it directly
    if let Some(ref hint) = embed.fk_hint {
        // Look for FK by name in parent table
        if let Some(fk) = parent_table.foreign_keys.iter().find(|fk| fk.name == *hint) {
            let ref_table = schema
                .get_table(&fk.ref_table)
                .ok_or_else(|| DataError::InvalidQuery(format!(
                    "referenced table '{}' not found for FK hint '{}'",
                    fk.ref_table, hint
                )))?;
            return Ok((fk.clone(), ref_table.clone(), EmbedDirection::ToOne));
        }

        // Look for FK by name in referenced table pointing back to us
        for (_, table) in &schema.tables {
            if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.name == *hint && fk.ref_table == parent_table.name) {
                return Ok((fk.clone(), table.clone(), EmbedDirection::ToMany));
            }
        }

        return Err(DataError::InvalidQuery(format!(
            "foreign key hint '{}' not found",
            hint
        )));
    }

    // Auto-resolve by embed name
    // First, check if parent table has FK pointing to a table with this name
    let mut candidates = Vec::new();

    // ToOne: parent has FK to embed table
    for fk in &parent_table.foreign_keys {
        if fk.ref_table == embed.name {
            if let Some(ref_table) = schema.get_table(&fk.ref_table) {
                candidates.push((fk.clone(), ref_table.clone(), EmbedDirection::ToOne));
            }
        }
    }

    // ToMany: another table has FK pointing to parent
    if let Some(ref_table) = schema.get_table(&embed.name) {
        for fk in &ref_table.foreign_keys {
            if fk.ref_table == parent_table.name {
                candidates.push((fk.clone(), ref_table.clone(), EmbedDirection::ToMany));
            }
        }
    }

    match candidates.len() {
        0 => Err(DataError::InvalidQuery(format!(
            "cannot resolve embed '{}': no foreign key relationship found with table '{}'",
            embed.name, parent_table.name
        ))),
        1 => Ok(candidates.into_iter().next().unwrap()),
        _ => {
            // Ambiguous - provide hints
            let hints: Vec<String> = candidates
                .iter()
                .map(|(fk, _, dir)| {
                    let dir_str = match dir {
                        EmbedDirection::ToOne => "→",
                        EmbedDirection::ToMany => "←",
                    };
                    format!("{}!{} ({})", embed.name, fk.name, dir_str)
                })
                .collect();
            Err(DataError::AmbiguousEmbed {
                name: embed.name.clone(),
                hints,
            })
        }
    }
}

/// Build the LEFT JOIN LATERAL clause for an embed.
pub fn build_embed_lateral(
    embed: &ResolvedEmbed,
    parent_alias: &str,
    _parent_schema: &str,
    policy_predicate: Option<&str>,
) -> String {
    let ref_schema = &embed.ref_table.schema;
    let ref_table = &embed.ref_table.name;
    let alias = &embed.sql_alias;

    // Build column list for jsonb_build_object
    let columns_json = build_columns_json(&embed.spec.columns, &embed.nested);

    // Build join condition
    let join_condition = match embed.direction {
        EmbedDirection::ToOne => {
            // parent.fk_column = ref.pk_column
            let fk_col = embed.fk.columns.first().unwrap();
            let ref_col = embed.fk.ref_columns.first().unwrap();
            format!("\"{}\".\"{}\".\"{}\" = \"{}\".\"{}\"",
                ref_schema, ref_table, ref_col,
                parent_alias, fk_col)
        }
        EmbedDirection::ToMany => {
            // ref.fk_column = parent.pk_column
            let fk_col = embed.fk.columns.first().unwrap();
            let ref_col = embed.fk.ref_columns.first().unwrap();
            format!("\"{}\".\"{}\" = \"{}\".\"{}\"",
                ref_table, fk_col,
                parent_alias, ref_col)
        }
    };

    // Build full WHERE clause
    let mut where_parts = vec![join_condition];
    if let Some(policy) = policy_predicate {
        where_parts.push(format!("({})", policy));
    }
    let where_clause = where_parts.join(" AND ");

    // Generate the subquery
    match embed.direction {
        EmbedDirection::ToOne => {
            // Returns a single object or null
            format!(
                "LEFT JOIN LATERAL (\
                    SELECT jsonb_build_object({}) AS data \
                    FROM \"{}\".\"{}\"\
                    WHERE {} \
                    LIMIT 1\
                ) AS \"{}\" ON true",
                columns_json, ref_schema, ref_table, where_clause, alias
            )
        }
        EmbedDirection::ToMany => {
            // Returns an array
            format!(
                "LEFT JOIN LATERAL (\
                    SELECT COALESCE(jsonb_agg(jsonb_build_object({})), '[]'::jsonb) AS data \
                    FROM \"{}\".\"{}\" \
                    WHERE {}\
                ) AS \"{}\" ON true",
                columns_json, ref_schema, ref_table, where_clause, alias
            )
        }
    }
}

/// Build the columns argument for jsonb_build_object.
fn build_columns_json(
    columns: &[crate::query::SelectColumn],
    nested: &[ResolvedEmbed],
) -> String {
    use crate::query::SelectColumn;

    let mut parts = Vec::new();

    for col in columns {
        match col {
            SelectColumn::All => {
                // For *, we'd need to enumerate all columns from the table
                // For now, we'll skip this and let the caller handle it
                // This is a simplification - full implementation would need table info
            }
            SelectColumn::Column(name) => {
                parts.push(format!("'{}', \"{}\"", name, name));
            }
            SelectColumn::Aliased { alias, column } => {
                parts.push(format!("'{}', \"{}\"", alias, column));
            }
            SelectColumn::Embed(spec) => {
                // Find corresponding nested embed
                if let Some(nested_embed) = nested.iter().find(|n| n.spec.name == spec.name) {
                    let output_name = spec.alias.as_deref().unwrap_or(&spec.name);
                    parts.push(format!("'{}', \"{}\".data", output_name, nested_embed.sql_alias));
                }
            }
        }
    }

    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{ColumnInfo, ForeignKeyInfo, TableSnapshot};

    fn make_table(name: &str, columns: Vec<&str>, fks: Vec<ForeignKeyInfo>) -> TableSnapshot {
        TableSnapshot {
            schema: "public".to_string(),
            name: name.to_string(),
            columns: columns
                .into_iter()
                .map(|c| ColumnInfo {
                    name: c.to_string(),
                    sql_type: "text".to_string(),
                    nullable: false,
                    default: None,
                })
                .collect(),
            primary_key: vec!["id".to_string()],
            foreign_keys: fks,
            indexes: vec![],
        }
    }

    fn make_schema(tables: Vec<TableSnapshot>) -> SchemaSnapshot {
        let mut snapshot = SchemaSnapshot::default();
        for table in tables {
            snapshot.tables.insert(table.name.clone(), table);
        }
        snapshot
    }

    #[test]
    fn test_find_fk_to_one() {
        let users = make_table("users", vec!["id", "name"], vec![]);
        let posts = make_table(
            "posts",
            vec!["id", "title", "author_id"],
            vec![ForeignKeyInfo {
                name: "posts_author_id_fkey".to_string(),
                columns: vec!["author_id".to_string()],
                ref_table: "users".to_string(),
                ref_columns: vec!["id".to_string()],
                on_delete: "CASCADE".to_string(),
            }],
        );

        let schema = make_schema(vec![users, posts.clone()]);

        let embed = EmbedSpec {
            name: "users".to_string(),
            fk_hint: None,
            columns: vec![],
            alias: None,
        };

        let (fk, ref_table, direction) = find_fk_for_embed(&embed, &schema, &posts).unwrap();

        assert_eq!(fk.name, "posts_author_id_fkey");
        assert_eq!(ref_table.name, "users");
        assert_eq!(direction, EmbedDirection::ToOne);
    }

    #[test]
    fn test_find_fk_to_many() {
        let posts = make_table("posts", vec!["id", "title"], vec![]);
        let comments = make_table(
            "comments",
            vec!["id", "body", "post_id"],
            vec![ForeignKeyInfo {
                name: "comments_post_id_fkey".to_string(),
                columns: vec!["post_id".to_string()],
                ref_table: "posts".to_string(),
                ref_columns: vec!["id".to_string()],
                on_delete: "CASCADE".to_string(),
            }],
        );

        let schema = make_schema(vec![posts.clone(), comments]);

        let embed = EmbedSpec {
            name: "comments".to_string(),
            fk_hint: None,
            columns: vec![],
            alias: None,
        };

        let (fk, ref_table, direction) = find_fk_for_embed(&embed, &schema, &posts).unwrap();

        assert_eq!(fk.name, "comments_post_id_fkey");
        assert_eq!(ref_table.name, "comments");
        assert_eq!(direction, EmbedDirection::ToMany);
    }

    #[test]
    fn test_find_fk_with_hint() {
        let users = make_table("users", vec!["id", "name"], vec![]);
        let posts = make_table(
            "posts",
            vec!["id", "title", "author_id", "editor_id"],
            vec![
                ForeignKeyInfo {
                    name: "posts_author_id_fkey".to_string(),
                    columns: vec!["author_id".to_string()],
                    ref_table: "users".to_string(),
                    ref_columns: vec!["id".to_string()],
                    on_delete: "CASCADE".to_string(),
                },
                ForeignKeyInfo {
                    name: "posts_editor_id_fkey".to_string(),
                    columns: vec!["editor_id".to_string()],
                    ref_table: "users".to_string(),
                    ref_columns: vec!["id".to_string()],
                    on_delete: "SET NULL".to_string(),
                },
            ],
        );

        let schema = make_schema(vec![users, posts.clone()]);

        // Without hint - should be ambiguous
        let embed = EmbedSpec {
            name: "users".to_string(),
            fk_hint: None,
            columns: vec![],
            alias: None,
        };
        let result = find_fk_for_embed(&embed, &schema, &posts);
        assert!(matches!(result, Err(DataError::AmbiguousEmbed { .. })));

        // With hint - should resolve
        let embed_with_hint = EmbedSpec {
            name: "users".to_string(),
            fk_hint: Some("posts_author_id_fkey".to_string()),
            columns: vec![],
            alias: None,
        };
        let (fk, _, _) = find_fk_for_embed(&embed_with_hint, &schema, &posts).unwrap();
        assert_eq!(fk.name, "posts_author_id_fkey");
    }
}
