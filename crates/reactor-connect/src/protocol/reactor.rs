//! Reactor protocol extensions.
//!
//! These extend the Airbyte protocol with Reactor-specific message types
//! for Actions and Webhooks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Action result message (Reactor extension).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Invocation ID.
    pub invocation_id: String,
    /// Action output (if successful).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// Error (if failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ActionError>,
    /// Execution mode.
    pub mode: ActionMode,
    /// Duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Action error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
    /// Suggested fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    /// Retry after (milliseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

/// Action execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionMode {
    /// Real execution.
    Real,
    /// Dry run (no side effects).
    DryRun,
}

/// Webhook event message (Reactor extension).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Receiver ID.
    pub receiver_id: String,
    /// Event ID (for deduplication).
    pub event_id: String,
    /// Event type (vendor-defined).
    pub event_type: String,
    /// Event payload.
    pub payload: serde_json::Value,
    /// When the event occurred at the vendor.
    pub occurred_at: DateTime<Utc>,
    /// When we received the event.
    pub received_at: DateTime<Utc>,
}

impl ActionResult {
    /// Create a successful action result.
    pub fn success(invocation_id: String, output: serde_json::Value, dry_run: bool) -> Self {
        Self {
            invocation_id,
            output: Some(output),
            error: None,
            mode: if dry_run {
                ActionMode::DryRun
            } else {
                ActionMode::Real
            },
            duration_ms: None,
        }
    }

    /// Create a failed action result.
    pub fn failure(invocation_id: String, error: ActionError, dry_run: bool) -> Self {
        Self {
            invocation_id,
            output: None,
            error: Some(error),
            mode: if dry_run {
                ActionMode::DryRun
            } else {
                ActionMode::Real
            },
            duration_ms: None,
        }
    }
}
