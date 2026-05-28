//! Wire protocol types.
//!
//! Reactor's connector wire protocol is the Airbyte Protocol with two
//! Reactor extensions (ActionResult, WebhookEvent).

mod airbyte;
mod reactor;

pub use airbyte::*;
pub use reactor::*;

use crate::descriptor::StreamDescriptor;
use serde::{Deserialize, Serialize};

/// Connector message — the wire protocol.
///
/// This enum covers all message types that flow between connectors and Reactor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorMessage {
    /// A data record.
    Record(AirbyteRecordMessage),
    /// State checkpoint.
    State(AirbyteStateMessage),
    /// Log message.
    Log(AirbyteLogMessage),
    /// Trace message.
    Trace(AirbyteTraceMessage),
    /// Connector specification.
    Spec(AirbyteSpecMessage),
    /// Stream catalog.
    Catalog(AirbyteCatalogMessage),
    /// Connection status.
    ConnectionStatus(AirbyteConnectionStatus),
    // Reactor extensions:
    /// Action result.
    ActionResult(ActionResult),
    /// Webhook event.
    WebhookEvent(WebhookEvent),
}

/// Connection status returned by check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    /// Whether the connection succeeded.
    pub status: ConnectionStatusEnum,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Connection status enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectionStatusEnum {
    /// Connection succeeded.
    Succeeded,
    /// Connection failed.
    Failed,
}

impl ConnectionStatus {
    /// Create a successful status.
    pub fn succeeded() -> Self {
        Self {
            status: ConnectionStatusEnum::Succeeded,
            message: None,
        }
    }

    /// Create a failed status.
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: ConnectionStatusEnum::Failed,
            message: Some(message.into()),
        }
    }
}

/// Discovered catalog from a connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredCatalog {
    /// Available streams.
    pub streams: Vec<StreamDescriptor>,
}

/// Configured catalog for a sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredCatalog {
    /// Configured streams to sync.
    pub streams: Vec<ConfiguredStream>,
}

/// A configured stream for sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredStream {
    /// Stream name.
    pub stream: String,
    /// Sync mode.
    pub sync_mode: crate::descriptor::SyncMode,
    /// Cursor field for incremental.
    #[serde(default)]
    pub cursor_field: Option<Vec<String>>,
    /// Primary key for dedup.
    #[serde(default)]
    pub primary_key: Option<Vec<Vec<String>>>,
}

/// State bundle for incremental sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateBundle {
    /// Per-stream state.
    #[serde(default)]
    pub stream_states: std::collections::HashMap<String, serde_json::Value>,
    /// Global state (if any).
    #[serde(default)]
    pub global_state: Option<serde_json::Value>,
}

/// Limits for a sync run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLimits {
    /// Maximum records to read.
    #[serde(default)]
    pub max_rows: Option<u64>,
    /// Maximum duration in seconds.
    #[serde(default)]
    pub max_duration_seconds: Option<u64>,
    /// Whether this is a sandbox run.
    #[serde(default)]
    pub sandbox: bool,
}

impl Default for SyncLimits {
    fn default() -> Self {
        Self {
            max_rows: None,
            max_duration_seconds: None,
            sandbox: false,
        }
    }
}

/// Outcome of a write operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteOutcome {
    /// Number of records written.
    pub records_written: u64,
    /// Number of records failed.
    pub records_failed: u64,
    /// Error messages for failed records.
    #[serde(default)]
    pub errors: Vec<String>,
}
