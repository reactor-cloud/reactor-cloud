//! Connect policy evaluation.

use crate::error::ConnectError;
use crate::routes::conflicts::{ConflictPolicyType, ConflictResolution, ConflictRule};
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Facts about a conflict being resolved.
#[derive(Debug, Clone)]
pub struct ConflictFacts {
    /// Stream name.
    pub stream: String,
    /// Field name (if field-level conflict).
    pub field: Option<String>,
    /// Source A record.
    pub source_a: Value,
    /// Source B record.
    pub source_b: Value,
    /// Source A modified timestamp.
    pub source_a_modified: Option<DateTime<Utc>>,
    /// Source B modified timestamp.
    pub source_b_modified: Option<DateTime<Utc>>,
    /// Record primary key.
    pub record_key: Value,
}

impl ConflictFacts {
    /// Create new conflict facts.
    pub fn new(stream: impl Into<String>, source_a: Value, source_b: Value) -> Self {
        Self {
            stream: stream.into(),
            field: None,
            source_a,
            source_b,
            source_a_modified: None,
            source_b_modified: None,
            record_key: Value::Null,
        }
    }

    /// Set field name.
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set source A modified timestamp.
    pub fn with_source_a_modified(mut self, ts: DateTime<Utc>) -> Self {
        self.source_a_modified = Some(ts);
        self
    }

    /// Set source B modified timestamp.
    pub fn with_source_b_modified(mut self, ts: DateTime<Utc>) -> Self {
        self.source_b_modified = Some(ts);
        self
    }

    /// Set record key.
    pub fn with_record_key(mut self, key: Value) -> Self {
        self.record_key = key;
        self
    }
}

/// Context for conflict policy evaluation.
pub trait ConflictEvalContext {
    /// Get the org ID.
    fn org_id(&self) -> uuid::Uuid;
    /// Get the connection ID.
    fn connection_id(&self) -> uuid::Uuid;
}

/// Result of conflict policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictEvalResult {
    /// Use source A value.
    PreferSourceA,
    /// Use source B value.
    PreferSourceB,
    /// Merge non-null values.
    Merge,
    /// Skip (don't update).
    Skip,
}

/// Evaluate a conflict policy.
pub fn evaluate_conflict_policy(
    policy_type: ConflictPolicyType,
    rules: &[ConflictRule],
    facts: &ConflictFacts,
) -> ConflictEvalResult {
    match policy_type {
        ConflictPolicyType::SourceWins => ConflictEvalResult::PreferSourceA,
        ConflictPolicyType::DestWins => ConflictEvalResult::PreferSourceB,
        ConflictPolicyType::LatestWins => evaluate_latest_wins(facts),
        ConflictPolicyType::Custom => evaluate_custom_rules(rules, facts),
    }
}

/// Evaluate latest-wins policy.
fn evaluate_latest_wins(facts: &ConflictFacts) -> ConflictEvalResult {
    match (facts.source_a_modified, facts.source_b_modified) {
        (Some(a), Some(b)) => {
            if a >= b {
                ConflictEvalResult::PreferSourceA
            } else {
                ConflictEvalResult::PreferSourceB
            }
        }
        (Some(_), None) => ConflictEvalResult::PreferSourceA,
        (None, Some(_)) => ConflictEvalResult::PreferSourceB,
        (None, None) => ConflictEvalResult::PreferSourceA,
    }
}

/// Evaluate custom rules.
fn evaluate_custom_rules(rules: &[ConflictRule], facts: &ConflictFacts) -> ConflictEvalResult {
    for rule in rules {
        if matches_rule(rule, facts) {
            return match &rule.then {
                ConflictResolution::PreferSourceA => ConflictEvalResult::PreferSourceA,
                ConflictResolution::PreferSourceB => ConflictEvalResult::PreferSourceB,
                ConflictResolution::PreferLatest => evaluate_latest_wins(facts),
                ConflictResolution::Merge => ConflictEvalResult::Merge,
                ConflictResolution::Skip => ConflictEvalResult::Skip,
            };
        }
    }
    // Default: prefer source A
    ConflictEvalResult::PreferSourceA
}

/// Check if a rule matches the facts.
fn matches_rule(rule: &ConflictRule, facts: &ConflictFacts) -> bool {
    // Check stream pattern
    if !matches_pattern(&rule.stream, &facts.stream) {
        return false;
    }

    // Check field pattern (if specified)
    if let Some(field_pattern) = &rule.field {
        if let Some(fact_field) = &facts.field {
            if !matches_pattern(field_pattern, fact_field) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check condition (if specified)
    if let Some(condition) = &rule.when {
        if !evaluate_condition(condition, facts) {
            return false;
        }
    }

    true
}

/// Check if a pattern matches a value (glob-style).
fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return value.starts_with(prefix);
    }
    if pattern.starts_with('*') {
        let suffix = &pattern[1..];
        return value.ends_with(suffix);
    }
    pattern.eq_ignore_ascii_case(value)
}

/// Evaluate a condition.
fn evaluate_condition(
    condition: &crate::routes::conflicts::ConflictCondition,
    facts: &ConflictFacts,
) -> bool {
    use crate::routes::conflicts::ConflictCondition;

    match condition {
        ConflictCondition::Always => true,
        ConflictCondition::SourceAEquals { field, value } => {
            get_field(&facts.source_a, field) == Some(value)
        }
        ConflictCondition::SourceBEquals { field, value } => {
            get_field(&facts.source_b, field) == Some(value)
        }
        ConflictCondition::FieldIn { values } => {
            if let Some(field) = &facts.field {
                values.iter().any(|v| v.eq_ignore_ascii_case(field))
            } else {
                false
            }
        }
    }
}

/// Get a field from a JSON value.
fn get_field<'a>(record: &'a Value, field: &str) -> Option<&'a Value> {
    record.get(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_source_wins() {
        let facts = ConflictFacts::new("Lead", json!({"a": 1}), json!({"b": 2}));
        let result = evaluate_conflict_policy(ConflictPolicyType::SourceWins, &[], &facts);
        assert_eq!(result, ConflictEvalResult::PreferSourceA);
    }

    #[test]
    fn test_dest_wins() {
        let facts = ConflictFacts::new("Lead", json!({"a": 1}), json!({"b": 2}));
        let result = evaluate_conflict_policy(ConflictPolicyType::DestWins, &[], &facts);
        assert_eq!(result, ConflictEvalResult::PreferSourceB);
    }

    #[test]
    fn test_latest_wins() {
        let now = Utc::now();
        let earlier = now - chrono::Duration::hours(1);

        let facts = ConflictFacts::new("Lead", json!({"a": 1}), json!({"b": 2}))
            .with_source_a_modified(earlier)
            .with_source_b_modified(now);

        let result = evaluate_conflict_policy(ConflictPolicyType::LatestWins, &[], &facts);
        assert_eq!(result, ConflictEvalResult::PreferSourceB);
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("Lead", "Lead"));
        assert!(matches_pattern("lead", "Lead"));
        assert!(matches_pattern("Lead*", "LeadScore"));
        assert!(matches_pattern("*Score", "LeadScore"));
        assert!(!matches_pattern("Contact", "Lead"));
    }
}
