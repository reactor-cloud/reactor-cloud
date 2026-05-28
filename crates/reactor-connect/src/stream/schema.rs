//! Schema discovery and drift detection.

use crate::descriptor::StreamDescriptor;

/// Schema diff between discovered and current.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiff {
    /// Stream name.
    pub stream: String,
    /// Added columns.
    pub added_columns: Vec<ColumnDiff>,
    /// Removed columns.
    pub removed_columns: Vec<String>,
    /// Type changes.
    pub type_changes: Vec<TypeChange>,
}

/// Column diff.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColumnDiff {
    /// Column name.
    pub name: String,
    /// Column type.
    pub column_type: String,
}

/// Type change.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeChange {
    /// Column name.
    pub column: String,
    /// Old type.
    pub from: String,
    /// New type.
    pub to: String,
}

/// Detect schema drift between discovered and current schemas.
pub fn detect_drift(
    discovered: &[StreamDescriptor],
    current: &[StreamDescriptor],
) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();

    let current_by_name: std::collections::HashMap<&str, &StreamDescriptor> =
        current.iter().map(|s| (s.name.as_str(), s)).collect();

    for stream in discovered {
        let current_stream = current_by_name.get(stream.name.as_str());
        
        if let Some(curr) = current_stream {
            // Compare schemas
            let diff = compare_schemas(&stream.json_schema, &curr.json_schema, &stream.name);
            if !diff.added_columns.is_empty()
                || !diff.removed_columns.is_empty()
                || !diff.type_changes.is_empty()
            {
                diffs.push(diff);
            }
        } else {
            // New stream
            diffs.push(SchemaDiff {
                stream: stream.name.clone(),
                added_columns: extract_columns(&stream.json_schema),
                removed_columns: vec![],
                type_changes: vec![],
            });
        }
    }

    diffs
}

fn compare_schemas(
    new: &serde_json::Value,
    old: &serde_json::Value,
    stream: &str,
) -> SchemaDiff {
    let new_cols = extract_columns(new);
    let old_cols = extract_columns(old);

    let new_names: std::collections::HashSet<_> = new_cols.iter().map(|c| c.name.as_str()).collect();
    let old_names: std::collections::HashSet<_> = old_cols.iter().map(|c| c.name.as_str()).collect();

    let added: Vec<_> = new_cols
        .iter()
        .filter(|c| !old_names.contains(c.name.as_str()))
        .cloned()
        .collect();

    let removed: Vec<_> = old_names
        .difference(&new_names)
        .map(|s| s.to_string())
        .collect();

    let old_by_name: std::collections::HashMap<&str, &ColumnDiff> =
        old_cols.iter().map(|c| (c.name.as_str(), c)).collect();

    let type_changes: Vec<_> = new_cols
        .iter()
        .filter_map(|c| {
            old_by_name.get(c.name.as_str()).and_then(|old| {
                if old.column_type != c.column_type {
                    Some(TypeChange {
                        column: c.name.clone(),
                        from: old.column_type.clone(),
                        to: c.column_type.clone(),
                    })
                } else {
                    None
                }
            })
        })
        .collect();

    SchemaDiff {
        stream: stream.to_string(),
        added_columns: added,
        removed_columns: removed,
        type_changes,
    }
}

fn extract_columns(schema: &serde_json::Value) -> Vec<ColumnDiff> {
    let mut cols = Vec::new();
    
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, def) in props {
            let col_type = def
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            cols.push(ColumnDiff {
                name: name.clone(),
                column_type: col_type,
            });
        }
    }
    
    cols
}
