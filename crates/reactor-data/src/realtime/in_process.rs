//! In-process realtime backend using tokio broadcast channels.
//!
//! Suitable for single-node deployments (G1/G2). Events do not persist
//! and do not cross process boundaries.

use async_trait::async_trait;
use reactor_core::ProjectRef;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::debug;

use super::{
    build_topic, DataChangeEvent, DataChangeOp, RealtimeBackend, RealtimeError,
    RealtimeSubscription,
};

/// In-process realtime backend using tokio broadcast channels.
#[derive(Debug)]
pub struct InProcessRealtime {
    channels: RwLock<HashMap<String, broadcast::Sender<DataChangeEvent>>>,
    channel_capacity: usize,
}

impl InProcessRealtime {
    /// Create a new in-process realtime backend with default capacity (1024 events).
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    /// Create a new in-process realtime backend with specified channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            channel_capacity: capacity,
        }
    }

    /// Get or create a channel for a topic.
    async fn get_or_create_channel(
        &self,
        topic: &str,
    ) -> broadcast::Sender<DataChangeEvent> {
        // Fast path: check if channel exists
        {
            let channels = self.channels.read().await;
            if let Some(sender) = channels.get(topic) {
                return sender.clone();
            }
        }

        // Slow path: create new channel
        let mut channels = self.channels.write().await;

        // Double-check after acquiring write lock
        if let Some(sender) = channels.get(topic) {
            return sender.clone();
        }

        let (sender, _) = broadcast::channel(self.channel_capacity);
        channels.insert(topic.to_string(), sender.clone());
        sender
    }

    /// Get all matching channels for a publish operation.
    ///
    /// For example, publishing to `reactor.proj.data.users.insert` should
    /// also notify subscribers to `reactor.proj.data.users.*` and
    /// `reactor.proj.data.*.insert` and `reactor.proj.data.*.*`.
    async fn get_matching_channels(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        op: DataChangeOp,
    ) -> Vec<broadcast::Sender<DataChangeEvent>> {
        let channels = self.channels.read().await;
        let mut results = Vec::new();

        // Exact match
        let exact = build_topic(project_ref, table, op);
        if let Some(sender) = channels.get(&exact) {
            results.push(sender.clone());
        }

        // Wildcard op match (table.*)
        let wildcard_op = build_topic(project_ref, table, DataChangeOp::All);
        if let Some(sender) = channels.get(&wildcard_op) {
            results.push(sender.clone());
        }

        // Wildcard table match (*.op)
        let wildcard_table = build_topic(project_ref, "*", op);
        if let Some(sender) = channels.get(&wildcard_table) {
            results.push(sender.clone());
        }

        // Full wildcard (*.*)
        let full_wildcard = build_topic(project_ref, "*", DataChangeOp::All);
        if let Some(sender) = channels.get(&full_wildcard) {
            results.push(sender.clone());
        }

        results
    }
}

impl Default for InProcessRealtime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RealtimeBackend for InProcessRealtime {
    async fn publish(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        event: &DataChangeEvent,
    ) -> Result<(), RealtimeError> {
        let channels = self.get_matching_channels(project_ref, table, event.op).await;

        if channels.is_empty() {
            debug!(
                project_ref = %project_ref,
                table = %table,
                op = %event.op,
                "no subscribers for data change event"
            );
            return Ok(());
        }

        for sender in channels {
            // send() only fails if there are no receivers, which is fine
            let _ = sender.send(event.clone());
        }

        debug!(
            project_ref = %project_ref,
            table = %table,
            op = %event.op,
            event_id = %event.id,
            "published data change event"
        );

        Ok(())
    }

    async fn subscribe(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        op: DataChangeOp,
    ) -> Result<RealtimeSubscription, RealtimeError> {
        let topic = build_topic(project_ref, table, op);
        let sender = self.get_or_create_channel(&topic).await;
        let receiver = sender.subscribe();

        debug!(
            topic = %topic,
            project_ref = %project_ref,
            table = %table,
            op = %op,
            "subscribed to data changes"
        );

        Ok(RealtimeSubscription::new(topic, receiver))
    }

    async fn unsubscribe(&self, _topic: &str) -> Result<(), RealtimeError> {
        // For in-process, unsubscribe is a no-op — just drop the subscription
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        true // Always connected for in-process
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reactor_core::ProjectId;

    fn test_project_ref() -> ProjectRef {
        let id = ProjectId::new();
        id.to_ref()
    }

    #[tokio::test]
    async fn test_publish_subscribe() {
        let realtime = InProcessRealtime::new();
        let project_ref = test_project_ref();
        let project_id = ProjectId::new();

        // Subscribe first
        let mut sub = realtime
            .subscribe(&project_ref, "users", DataChangeOp::Insert)
            .await
            .unwrap();

        // Publish an event
        let event = DataChangeEvent::insert(
            project_id,
            "public",
            "users",
            serde_json::json!({"id": 1}),
            None,
        );

        realtime.publish(&project_ref, "users", &event).await.unwrap();

        // Receive it
        let received = sub.recv().await.unwrap();
        assert_eq!(received.id, event.id);
        assert_eq!(received.table, "users");
        assert_eq!(received.op, DataChangeOp::Insert);
    }

    #[tokio::test]
    async fn test_wildcard_subscription() {
        let realtime = InProcessRealtime::new();
        let project_ref = test_project_ref();
        let project_id = ProjectId::new();

        // Subscribe to all operations on users table
        let mut sub = realtime
            .subscribe(&project_ref, "users", DataChangeOp::All)
            .await
            .unwrap();

        // Publish an insert
        let event = DataChangeEvent::insert(
            project_id,
            "public",
            "users",
            serde_json::json!({"id": 1}),
            None,
        );

        realtime.publish(&project_ref, "users", &event).await.unwrap();

        // Should receive it via wildcard
        let received = sub.recv().await.unwrap();
        assert_eq!(received.id, event.id);
    }

    #[tokio::test]
    async fn test_table_isolation() {
        let realtime = InProcessRealtime::new();
        let project_ref = test_project_ref();
        let project_id = ProjectId::new();

        // Subscribe to posts table
        let _posts_sub = realtime
            .subscribe(&project_ref, "posts", DataChangeOp::Insert)
            .await
            .unwrap();

        // Publish to users table — should not error even without subscribers
        let event = DataChangeEvent::insert(
            project_id,
            "public",
            "users",
            serde_json::json!({"id": 1}),
            None,
        );

        let result = realtime.publish(&project_ref, "users", &event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_is_connected() {
        let realtime = InProcessRealtime::new();
        assert!(realtime.is_connected().await);
    }
}
