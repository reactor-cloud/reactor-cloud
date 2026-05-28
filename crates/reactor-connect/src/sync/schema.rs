//! Schema discovery and drift detection.
//!
//! Detects changes in connector schemas that might require user approval
//! before proceeding with sync.

use crate::descriptor::StreamDescriptor;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A detected schema change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchemaChange {
    /// A new column/field was added.
    ColumnAdded {
        /// Column name.
        name: String,
        /// JSON Schema type.
        json_type: String,
    },
    /// A column/field was removed.
    ColumnRemoved {
        /// Column name.
        name: String,
    },
    /// A column type changed.
    ColumnTypeChanged {
        /// Column name.
        name: String,
        /// Old type.
        old_type: String,
        /// New type.
        new_type: String,
    },
    /// Primary key changed.
    PrimaryKeyChanged {
        /// Old primary key.
        old_key: Option<Vec<Vec<String>>>,
        /// New primary key.
        new_key: Option<Vec<Vec<String>>>,
    },
    /// A new stream was added.
    StreamAdded {
        /// Stream name.
        name: String,
    },
    /// A stream was removed.
    StreamRemoved {
        /// Stream name.
        name: String,
    },
}

/// Drift severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    /// Informational only, safe to continue.
    Info,
    /// Warning, should be reviewed but can continue.
    Warning,
    /// Breaking change, requires approval.
    Breaking,
}

impl SchemaChange {
    /// Get the severity of this change.
    pub fn severity(&self) -> DriftSeverity {
        match self {
            SchemaChange::ColumnAdded { .. } => DriftSeverity::Info,
            SchemaChange::ColumnRemoved { .. } => DriftSeverity::Warning,
            SchemaChange::ColumnTypeChanged { .. } => DriftSeverity::Breaking,
            SchemaChange::PrimaryKeyChanged { .. } => DriftSeverity::Breaking,
            SchemaChange::StreamAdded { .. } => DriftSeverity::Info,
            SchemaChange::StreamRemoved { .. } => DriftSeverity::Warning,
        }
    }
}

/// Schema diff result for a single stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamSchemaDiff {
    /// Stream name.
    pub stream: String,
    /// List of changes.
    pub changes: Vec<SchemaChange>,
    /// Maximum severity among all changes.
    pub max_severity: Option<DriftSeverity>,
}

impl StreamSchemaDiff {
    /// Check if there are any changes.
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Check if there are breaking changes.
    pub fn has_breaking_changes(&self) -> bool {
        self.max_severity == Some(DriftSeverity::Breaking)
    }
}

/// Schema diff result for a catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaDiff {
    /// Per-stream diffs.
    pub streams: HashMap<String, StreamSchemaDiff>,
    /// Maximum severity across all streams.
    pub max_severity: Option<DriftSeverity>,
    /// Total number of changes.
    pub total_changes: usize,
}

impl SchemaDiff {
    /// Check if there are any changes.
    pub fn has_changes(&self) -> bool {
        self.total_changes > 0
    }

    /// Check if there are breaking changes.
    pub fn has_breaking_changes(&self) -> bool {
        self.max_severity == Some(DriftSeverity::Breaking)
    }

    /// Get all streams with breaking changes.
    pub fn breaking_streams(&self) -> Vec<&str> {
        self.streams
            .iter()
            .filter(|(_, diff)| diff.has_breaking_changes())
            .map(|(name, _)| name.as_str())
            .collect()
    }
}

/// Detect schema drift between old and new catalogs.
pub fn detect_drift(
    old_streams: &[StreamDescriptor],
    new_streams: &[StreamDescriptor],
) -> SchemaDiff {
    let mut diff = SchemaDiff::default();

    let old_map: HashMap<&str, &StreamDescriptor> = old_streams
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    let new_map: HashMap<&str, &StreamDescriptor> = new_streams
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    let old_names: HashSet<&str> = old_map.keys().copied().collect();
    let new_names: HashSet<&str> = new_map.keys().copied().collect();

    // Check for removed streams
    for name in old_names.difference(&new_names) {
        let change = SchemaChange::StreamRemoved {
            name: name.to_string(),
        };
        let stream_diff = diff.streams.entry(name.to_string()).or_insert_with(|| {
            StreamSchemaDiff {
                stream: name.to_string(),
                ..Default::default()
            }
        });
        update_severity(stream_diff, change.severity());
        stream_diff.changes.push(change);
        diff.total_changes += 1;
    }

    // Check for added streams
    for name in new_names.difference(&old_names) {
        let change = SchemaChange::StreamAdded {
            name: name.to_string(),
        };
        let stream_diff = diff.streams.entry(name.to_string()).or_insert_with(|| {
            StreamSchemaDiff {
                stream: name.to_string(),
                ..Default::default()
            }
        });
        update_severity(stream_diff, change.severity());
        stream_diff.changes.push(change);
        diff.total_changes += 1;
    }

    // Check existing streams for changes
    for name in old_names.intersection(&new_names) {
        let old_stream = old_map[name];
        let new_stream = new_map[name];

        let changes = compare_stream_schemas(old_stream, new_stream);
        if !changes.is_empty() {
            let stream_diff = diff.streams.entry(name.to_string()).or_insert_with(|| {
                StreamSchemaDiff {
                    stream: name.to_string(),
                    ..Default::default()
                }
            });

            for change in changes {
                update_severity(stream_diff, change.severity());
                stream_diff.changes.push(change);
                diff.total_changes += 1;
            }
        }
    }

    // Calculate max severity across all streams
    diff.max_severity = diff.streams.values()
        .filter_map(|s| s.max_severity)
        .max();

    diff
}

/// Compare two stream schemas and return changes.
fn compare_stream_schemas(
    old: &StreamDescriptor,
    new: &StreamDescriptor,
) -> Vec<SchemaChange> {
    let mut changes = Vec::new();

    // Compare primary keys
    if old.primary_key != new.primary_key {
        changes.push(SchemaChange::PrimaryKeyChanged {
            old_key: old.primary_key.clone(),
            new_key: new.primary_key.clone(),
        });
    }

    // Compare JSON schema properties
    let old_props = extract_properties(&old.json_schema);
    let new_props = extract_properties(&new.json_schema);

    let old_names: HashSet<&str> = old_props.keys().copied().collect();
    let new_names: HashSet<&str> = new_props.keys().copied().collect();

    // Removed columns
    for name in old_names.difference(&new_names) {
        changes.push(SchemaChange::ColumnRemoved {
            name: name.to_string(),
        });
    }

    // Added columns
    for name in new_names.difference(&old_names) {
        changes.push(SchemaChange::ColumnAdded {
            name: name.to_string(),
            json_type: new_props[name].clone(),
        });
    }

    // Type changes
    for name in old_names.intersection(&new_names) {
        let old_type = &old_props[name];
        let new_type = &new_props[name];
        if old_type != new_type {
            changes.push(SchemaChange::ColumnTypeChanged {
                name: name.to_string(),
                old_type: old_type.clone(),
                new_type: new_type.clone(),
            });
        }
    }

    changes
}

/// Extract property names and types from a JSON Schema.
fn extract_properties(schema: &serde_json::Value) -> HashMap<&str, String> {
    let mut props = HashMap::new();

    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop_schema) in properties {
            let type_str = extract_type(prop_schema);
            props.insert(name.as_str(), type_str);
        }
    }

    props
}

/// Extract type string from a JSON Schema property.
fn extract_type(schema: &serde_json::Value) -> String {
    if let Some(type_val) = schema.get("type") {
        match type_val {
            serde_json::Value::String(s) => return s.clone(),
            serde_json::Value::Array(arr) => {
                let types: Vec<&str> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect();
                return types.join("|");
            }
            _ => {}
        }
    }

    // Check for anyOf, oneOf, allOf
    for key in &["anyOf", "oneOf", "allOf"] {
        if let Some(arr) = schema.get(key).and_then(|v| v.as_array()) {
            let types: Vec<String> = arr
                .iter()
                .map(extract_type)
                .collect();
            return format!("{}({})", key, types.join(","));
        }
    }

    // Check for $ref
    if let Some(ref_val) = schema.get("$ref").and_then(|v| v.as_str()) {
        return format!("$ref:{}", ref_val);
    }

    "unknown".to_string()
}

/// Update the max severity of a stream diff.
fn update_severity(diff: &mut StreamSchemaDiff, severity: DriftSeverity) {
    match &diff.max_severity {
        None => diff.max_severity = Some(severity),
        Some(current) if severity > *current => diff.max_severity = Some(severity),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::SyncMode;

    fn make_stream(name: &str, schema: serde_json::Value) -> StreamDescriptor {
        StreamDescriptor {
            name: name.to_string(),
            json_schema: schema,
            supported_modes: vec![SyncMode::FullRefresh],
            cursor_field: None,
            primary_key: None,
            supports_outbound: false,
            source_defined: false,
        }
    }

    #[test]
    fn test_detect_added_stream() {
        let old = vec![];
        let new = vec![make_stream("users", serde_json::json!({"type": "object"}))];

        let diff = detect_drift(&old, &new);

        assert!(diff.has_changes());
        assert_eq!(diff.total_changes, 1);
        assert_eq!(diff.max_severity, Some(DriftSeverity::Info));
    }

    #[test]
    fn test_detect_removed_stream() {
        let old = vec![make_stream("users", serde_json::json!({"type": "object"}))];
        let new = vec![];

        let diff = detect_drift(&old, &new);

        assert!(diff.has_changes());
        assert_eq!(diff.total_changes, 1);
        assert_eq!(diff.max_severity, Some(DriftSeverity::Warning));
    }

    #[test]
    fn test_detect_added_column() {
        let old = vec![make_stream("users", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer"}
            }
        }))];
        let new = vec![make_stream("users", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer"},
                "name": {"type": "string"}
            }
        }))];

        let diff = detect_drift(&old, &new);

        assert!(diff.has_changes());
        assert_eq!(diff.total_changes, 1);
        assert_eq!(diff.max_severity, Some(DriftSeverity::Info));
    }

    #[test]
    fn test_detect_type_change() {
        let old = vec![make_stream("users", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer"}
            }
        }))];
        let new = vec![make_stream("users", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "string"}
            }
        }))];

        let diff = detect_drift(&old, &new);

        assert!(diff.has_changes());
        assert!(diff.has_breaking_changes());
        assert_eq!(diff.max_severity, Some(DriftSeverity::Breaking));
    }
}
