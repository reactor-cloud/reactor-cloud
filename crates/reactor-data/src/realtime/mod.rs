//! Realtime backend for data change notifications.
//!
//! The realtime system broadcasts database change events (INSERT, UPDATE, DELETE)
//! to subscribers. In single-node deployments, this uses in-process broadcast
//! channels. In multi-tenant shared clusters, this uses NATS JetStream for
//! cross-node fanout.
//!
//! # Topic naming
//!
//! Topics are tenant-scoped:
//! ```text
//! reactor.{project_ref}.data.{table}.{op}
//! ```
//!
//! Where `op` is one of: `insert`, `update`, `delete`, `*` (all).
//!
//! # Usage
//!
//! ```ignore
//! // Publish a change event
//! realtime.publish(&tenant, "users", DataChangeOp::Insert, &payload).await?;
//!
//! // Subscribe to changes
//! let mut sub = realtime.subscribe(&tenant, "users", DataChangeOp::All).await?;
//! while let Some(event) = sub.recv().await? {
//!     println!("Change: {:?}", event);
//! }
//! ```

mod in_process;
#[cfg(feature = "nats")]
mod nats;

pub use in_process::InProcessRealtime;
#[cfg(feature = "nats")]
pub use nats::{NatsConfig, NatsRealtime};

use async_trait::async_trait;
use bytes::Bytes;
use reactor_core::{ProjectId, ProjectRef};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Error type for realtime operations.
#[derive(Debug, Error)]
pub enum RealtimeError {
    /// Failed to publish event.
    #[error("publish failed: {0}")]
    PublishFailed(String),

    /// Failed to subscribe.
    #[error("subscribe failed: {0}")]
    SubscribeFailed(String),

    /// Channel closed.
    #[error("channel closed")]
    ChannelClosed,

    /// Connection error.
    #[error("connection error: {0}")]
    Connection(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Data change operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataChangeOp {
    /// Row insertion.
    Insert,
    /// Row update.
    Update,
    /// Row deletion.
    Delete,
    /// All operations (for subscribing).
    #[serde(rename = "*")]
    All,
}

impl DataChangeOp {
    /// Returns the operation as a string for topic naming.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::All => "*",
        }
    }

    /// Check if this operation matches a filter.
    pub fn matches(&self, filter: DataChangeOp) -> bool {
        filter == DataChangeOp::All || *self == filter
    }
}

impl std::fmt::Display for DataChangeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A data change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChangeEvent {
    /// Unique event ID.
    pub id: Uuid,

    /// Project ID this event belongs to.
    pub project_id: Uuid,

    /// Table name.
    pub table: String,

    /// Operation type.
    pub op: DataChangeOp,

    /// Schema name.
    pub schema: String,

    /// Old row data (for update/delete).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<serde_json::Value>,

    /// New row data (for insert/update).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<serde_json::Value>,

    /// Timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Request ID for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Uuid>,
}

impl DataChangeEvent {
    /// Create a new INSERT event.
    pub fn insert(
        project_id: ProjectId,
        schema: &str,
        table: &str,
        new: serde_json::Value,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            project_id: project_id.into(),
            table: table.to_string(),
            op: DataChangeOp::Insert,
            schema: schema.to_string(),
            old: None,
            new: Some(new),
            timestamp: chrono::Utc::now(),
            request_id,
        }
    }

    /// Create a new UPDATE event.
    pub fn update(
        project_id: ProjectId,
        schema: &str,
        table: &str,
        old: serde_json::Value,
        new: serde_json::Value,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            project_id: project_id.into(),
            table: table.to_string(),
            op: DataChangeOp::Update,
            schema: schema.to_string(),
            old: Some(old),
            new: Some(new),
            timestamp: chrono::Utc::now(),
            request_id,
        }
    }

    /// Create a new DELETE event.
    pub fn delete(
        project_id: ProjectId,
        schema: &str,
        table: &str,
        old: serde_json::Value,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            project_id: project_id.into(),
            table: table.to_string(),
            op: DataChangeOp::Delete,
            schema: schema.to_string(),
            old: Some(old),
            new: None,
            timestamp: chrono::Utc::now(),
            request_id,
        }
    }

    /// Serialize to JSON bytes.
    pub fn to_bytes(&self) -> Result<Bytes, RealtimeError> {
        serde_json::to_vec(self)
            .map(Bytes::from)
            .map_err(|e| RealtimeError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RealtimeError> {
        serde_json::from_slice(bytes).map_err(|e| RealtimeError::Serialization(e.to_string()))
    }
}

/// A subscription to data change events.
pub struct RealtimeSubscription {
    topic: String,
    receiver: tokio::sync::broadcast::Receiver<DataChangeEvent>,
}

impl RealtimeSubscription {
    /// Create a new subscription.
    pub(crate) fn new(
        topic: String,
        receiver: tokio::sync::broadcast::Receiver<DataChangeEvent>,
    ) -> Self {
        Self { topic, receiver }
    }

    /// Receive the next event.
    pub async fn recv(&mut self) -> Result<DataChangeEvent, RealtimeError> {
        self.receiver
            .recv()
            .await
            .map_err(|_| RealtimeError::ChannelClosed)
    }

    /// Get the topic this subscription is for.
    pub fn topic(&self) -> &str {
        &self.topic
    }
}

/// Realtime backend trait for data change broadcasting.
///
/// Implementations:
/// - [`InProcessRealtime`] — In-process broadcast channels (single-node)
/// - [`NatsRealtime`] — NATS JetStream (multi-node, Phase 4+)
#[async_trait]
pub trait RealtimeBackend: Send + Sync {
    /// Publish a data change event.
    ///
    /// # Arguments
    /// * `project_ref` — Project reference for topic scoping
    /// * `table` — Table name
    /// * `event` — The change event to publish
    async fn publish(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        event: &DataChangeEvent,
    ) -> Result<(), RealtimeError>;

    /// Subscribe to data change events for a table.
    ///
    /// # Arguments
    /// * `project_ref` — Project reference for topic scoping
    /// * `table` — Table name (or "*" for all tables)
    /// * `op` — Operation filter (or `All` for all operations)
    async fn subscribe(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        op: DataChangeOp,
    ) -> Result<RealtimeSubscription, RealtimeError>;

    /// Unsubscribe from a topic.
    async fn unsubscribe(&self, topic: &str) -> Result<(), RealtimeError>;

    /// Check if connected to the realtime backend.
    async fn is_connected(&self) -> bool;
}

/// Build the topic name for a data change subscription.
pub fn build_topic(project_ref: &ProjectRef, table: &str, op: DataChangeOp) -> String {
    format!("reactor.{}.data.{}.{}", project_ref, table, op.as_str())
}

/// Create a shared in-process realtime backend.
pub fn in_process_realtime() -> Arc<dyn RealtimeBackend> {
    Arc::new(InProcessRealtime::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reactor_core::ProjectId;

    fn test_project_ref() -> ProjectRef {
        let id = ProjectId::new();
        id.to_ref()
    }

    #[test]
    fn test_data_change_op_as_str() {
        assert_eq!(DataChangeOp::Insert.as_str(), "insert");
        assert_eq!(DataChangeOp::Update.as_str(), "update");
        assert_eq!(DataChangeOp::Delete.as_str(), "delete");
        assert_eq!(DataChangeOp::All.as_str(), "*");
    }

    #[test]
    fn test_data_change_op_matches() {
        assert!(DataChangeOp::Insert.matches(DataChangeOp::All));
        assert!(DataChangeOp::Insert.matches(DataChangeOp::Insert));
        assert!(!DataChangeOp::Insert.matches(DataChangeOp::Update));
    }

    #[test]
    fn test_build_topic() {
        let ref_ = test_project_ref();
        let topic = build_topic(&ref_, "users", DataChangeOp::Insert);
        assert!(topic.starts_with("reactor."));
        assert!(topic.contains(".data.users.insert"));
    }

    #[test]
    fn test_data_change_event_serialization() {
        let project_id = ProjectId::new();
        let event = DataChangeEvent::insert(
            project_id,
            "public",
            "users",
            serde_json::json!({"id": 1, "name": "Alice"}),
            None,
        );

        let bytes = event.to_bytes().unwrap();
        let parsed = DataChangeEvent::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.table, "users");
        assert_eq!(parsed.op, DataChangeOp::Insert);
        assert_eq!(parsed.schema, "public");
    }
}
