//! Airbyte protocol message types.
//!
//! These match the Airbyte Protocol specification for interoperability
//! with Airbyte connectors.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A data record message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteRecordMessage {
    /// Stream name.
    pub stream: String,
    /// Record data.
    pub data: serde_json::Value,
    /// Emission timestamp.
    pub emitted_at: i64,
    /// Namespace (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

impl AirbyteRecordMessage {
    /// Create a new record message.
    pub fn new(stream: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            stream: stream.into(),
            data,
            emitted_at: Utc::now().timestamp_millis(),
            namespace: None,
        }
    }
}

/// A state checkpoint message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteStateMessage {
    /// State type.
    #[serde(rename = "type", default)]
    pub state_type: AirbyteStateType,
    /// Stream state (if stream-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<AirbyteStreamState>,
    /// Global state (if global).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<AirbyteGlobalState>,
    /// Legacy data field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// State type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirbyteStateType {
    /// Stream-level state.
    Stream,
    /// Global state.
    Global,
    /// Legacy state (pre-typed).
    #[default]
    Legacy,
}

/// Stream-level state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteStreamState {
    /// Stream descriptor.
    pub stream_descriptor: AirbyteStreamDescriptor,
    /// State data.
    pub stream_state: serde_json::Value,
}

/// Stream descriptor for state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteStreamDescriptor {
    /// Stream name.
    pub name: String,
    /// Namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Global state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteGlobalState {
    /// Shared state.
    pub shared_state: serde_json::Value,
    /// Per-stream states.
    #[serde(default)]
    pub stream_states: Vec<AirbyteStreamState>,
}

/// Log message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteLogMessage {
    /// Log level.
    pub level: AirbyteLogLevel,
    /// Log message.
    pub message: String,
    /// Stack trace (if error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirbyteLogLevel {
    /// Fatal error.
    Fatal,
    /// Error.
    Error,
    /// Warning.
    Warn,
    /// Info.
    Info,
    /// Debug.
    Debug,
    /// Trace.
    Trace,
}

/// Trace message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteTraceMessage {
    /// Trace type.
    #[serde(rename = "type")]
    pub trace_type: AirbyteTraceType,
    /// Emission timestamp.
    pub emitted_at: f64,
    /// Error details (if error type).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AirbyteErrorTraceMessage>,
    /// Estimate details (if estimate type).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<AirbyteEstimateTraceMessage>,
}

/// Trace type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirbyteTraceType {
    /// Error trace.
    Error,
    /// Estimate trace.
    Estimate,
}

/// Error trace message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteErrorTraceMessage {
    /// Error message.
    pub message: String,
    /// Internal error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_message: Option<String>,
    /// Stack trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,
    /// Failure type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_type: Option<String>,
}

/// Estimate trace message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteEstimateTraceMessage {
    /// Stream name.
    pub name: String,
    /// Namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Estimated row count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_estimate: Option<i64>,
    /// Estimated byte count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_estimate: Option<i64>,
}

/// Connector specification message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteSpecMessage {
    /// Connection specification.
    pub connection_specification: serde_json::Value,
    /// Documentation URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    /// Supports incremental.
    #[serde(default)]
    pub supports_incremental: bool,
    /// Supported destination sync modes.
    #[serde(default)]
    pub supported_destination_sync_modes: Vec<String>,
}

/// Catalog message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteCatalogMessage {
    /// Streams in the catalog.
    pub streams: Vec<AirbyteCatalogStream>,
}

/// A stream in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteCatalogStream {
    /// Stream name.
    pub name: String,
    /// JSON schema.
    pub json_schema: serde_json::Value,
    /// Supported sync modes.
    #[serde(default)]
    pub supported_sync_modes: Vec<String>,
    /// Default cursor field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_cursor_field: Option<Vec<String>>,
    /// Source-defined cursor.
    #[serde(default)]
    pub source_defined_cursor: bool,
    /// Source-defined primary key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_defined_primary_key: Option<Vec<Vec<String>>>,
    /// Namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Connection status message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirbyteConnectionStatus {
    /// Status.
    pub status: String,
    /// Message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
