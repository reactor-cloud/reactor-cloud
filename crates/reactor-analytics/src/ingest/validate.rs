//! Event validation.
//!
//! Validates incoming events against size limits, event name restrictions,
//! and batch constraints.

use crate::config::AnalyticsConfig;
use crate::error::AnalyticsError;
use crate::ingest::IngestEvent;

/// Maximum event name length.
const MAX_EVENT_NAME_LENGTH: usize = 256;

/// Maximum batch size in events.
const MAX_BATCH_SIZE: usize = 100;

/// Maximum batch size in bytes.
const MAX_BATCH_BYTES: usize = 1024 * 1024; // 1 MiB

/// System event names that clients can send.
const CLIENT_ALLOWED_SYSTEM_EVENTS: &[&str] = &[
    "$pageview",
    "$identify",
    "$alias",
    "$session_start",
    "$session_end",
    "$autocapture",
    "$error",
];

/// System event names reserved for server-side only.
const SERVER_ONLY_SYSTEM_EVENTS: &[&str] = &[
    "$internal",
];

/// Validation result.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the event is valid.
    pub valid: bool,
    /// Error code if invalid.
    pub error_code: Option<String>,
    /// Error message if invalid.
    pub error_message: Option<String>,
}

impl ValidationResult {
    /// Create a valid result.
    pub fn ok() -> Self {
        Self {
            valid: true,
            error_code: None,
            error_message: None,
        }
    }

    /// Create an invalid result.
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            valid: false,
            error_code: Some(code.to_string()),
            error_message: Some(message.into()),
        }
    }
}

/// Validate a single event.
pub fn validate_event(
    event: &IngestEvent,
    config: &AnalyticsConfig,
    is_anonymous: bool,
) -> ValidationResult {
    // Check event name length
    if event.event.len() > MAX_EVENT_NAME_LENGTH {
        return ValidationResult::error(
            "analytics.event.name_too_long",
            format!(
                "event name exceeds {} characters",
                MAX_EVENT_NAME_LENGTH
            ),
        );
    }

    // Check event name is not empty
    if event.event.is_empty() {
        return ValidationResult::error(
            "analytics.event.name_required",
            "event name is required",
        );
    }

    // Check system event names ($ prefix)
    if event.event.starts_with('$') {
        // Anonymous clients can only send allowed system events
        if is_anonymous && !CLIENT_ALLOWED_SYSTEM_EVENTS.contains(&event.event.as_str()) {
            return ValidationResult::error(
                "analytics.event.system_reserved",
                format!("event name '{}' is reserved for system events", event.event),
            );
        }

        // Server-only events cannot be sent by any client
        if SERVER_ONLY_SYSTEM_EVENTS.contains(&event.event.as_str()) {
            return ValidationResult::error(
                "analytics.event.system_reserved",
                format!("event name '{}' is reserved for internal use", event.event),
            );
        }
    }

    // Check properties size
    let props_size = serde_json::to_vec(&event.properties)
        .map(|v| v.len())
        .unwrap_or(0);
    let context_size = serde_json::to_vec(&event.context)
        .map(|v| v.len())
        .unwrap_or(0);
    let total_size = props_size + context_size;

    if total_size > config.max_properties_bytes {
        return ValidationResult::error(
            "analytics.event.too_large",
            format!(
                "event payload ({} bytes) exceeds limit ({} bytes)",
                total_size, config.max_properties_bytes
            ),
        );
    }

    ValidationResult::ok()
}

/// Batch validation result.
#[derive(Debug)]
pub struct BatchValidationResult {
    /// Valid events (indices in original batch).
    pub valid: Vec<usize>,
    /// Rejected events with reasons.
    pub rejected: Vec<RejectedEvent>,
    /// Whether the batch itself is valid (not exceeding size limits).
    pub batch_valid: bool,
    /// Batch-level error if invalid.
    pub batch_error: Option<AnalyticsError>,
}

/// Rejected event in a batch.
#[derive(Debug, Clone)]
pub struct RejectedEvent {
    /// Index in the original batch.
    pub index: usize,
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

/// Validate a batch of events.
pub fn validate_batch(
    events: &[IngestEvent],
    config: &AnalyticsConfig,
    is_anonymous: bool,
) -> BatchValidationResult {
    // Check batch count
    if events.len() > MAX_BATCH_SIZE {
        return BatchValidationResult {
            valid: vec![],
            rejected: vec![],
            batch_valid: false,
            batch_error: Some(AnalyticsError::BatchTooLarge {
                count: events.len(),
                size: 0,
            }),
        };
    }

    // Estimate batch size
    let batch_bytes: usize = events
        .iter()
        .map(|e| serde_json::to_vec(e).map(|v| v.len()).unwrap_or(0))
        .sum();

    if batch_bytes > MAX_BATCH_BYTES {
        return BatchValidationResult {
            valid: vec![],
            rejected: vec![],
            batch_valid: false,
            batch_error: Some(AnalyticsError::BatchTooLarge {
                count: events.len(),
                size: batch_bytes,
            }),
        };
    }

    // Validate individual events
    let mut valid = Vec::new();
    let mut rejected = Vec::new();

    for (i, event) in events.iter().enumerate() {
        let result = validate_event(event, config, is_anonymous);
        if result.valid {
            valid.push(i);
        } else {
            rejected.push(RejectedEvent {
                index: i,
                code: result.error_code.unwrap_or_default(),
                message: result.error_message.unwrap_or_default(),
            });
        }
    }

    BatchValidationResult {
        valid,
        rejected,
        batch_valid: true,
        batch_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> AnalyticsConfig {
        AnalyticsConfig::default()
    }

    fn test_event(name: &str) -> IngestEvent {
        IngestEvent {
            event: name.to_string(),
            anonymous_id: Some("test-anon".to_string()),
            user_id: None,
            session_id: None,
            timestamp: None,
            properties: serde_json::json!({}),
            context: Default::default(),
        }
    }

    #[test]
    fn test_validate_valid_event() {
        let event = test_event("button_click");
        let result = validate_event(&event, &default_config(), true);
        assert!(result.valid);
    }

    #[test]
    fn test_validate_empty_name() {
        let event = test_event("");
        let result = validate_event(&event, &default_config(), true);
        assert!(!result.valid);
        assert_eq!(result.error_code, Some("analytics.event.name_required".to_string()));
    }

    #[test]
    fn test_validate_long_name() {
        let event = test_event(&"x".repeat(300));
        let result = validate_event(&event, &default_config(), true);
        assert!(!result.valid);
        assert_eq!(result.error_code, Some("analytics.event.name_too_long".to_string()));
    }

    #[test]
    fn test_validate_allowed_system_event() {
        let event = test_event("$pageview");
        let result = validate_event(&event, &default_config(), true);
        assert!(result.valid);
    }

    #[test]
    fn test_validate_disallowed_system_event() {
        let event = test_event("$custom_system");
        let result = validate_event(&event, &default_config(), true);
        assert!(!result.valid);
        assert_eq!(result.error_code, Some("analytics.event.system_reserved".to_string()));
    }

    #[test]
    fn test_validate_batch_size() {
        let events: Vec<IngestEvent> = (0..150)
            .map(|i| test_event(&format!("event_{}", i)))
            .collect();

        let result = validate_batch(&events, &default_config(), true);
        assert!(!result.batch_valid);
        assert!(result.batch_error.is_some());
    }
}
