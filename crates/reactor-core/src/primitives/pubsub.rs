//! Publish/subscribe primitive for realtime message fanout.
//!
//! Used by capabilities that need to broadcast messages across multiple
//! processes or connections (e.g., realtime data subscriptions).

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};

/// Error type for pub/sub operations.
#[derive(Debug, Error)]
pub enum PubSubError {
    /// Failed to publish message.
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
}

/// A subscription to a topic.
pub struct Subscription {
    /// The topic subscribed to.
    pub topic: String,
    /// Receiver for messages.
    receiver: broadcast::Receiver<Bytes>,
}

impl Subscription {
    /// Create a new subscription.
    pub(crate) fn new(topic: String, receiver: broadcast::Receiver<Bytes>) -> Self {
        Self { topic, receiver }
    }

    /// Receive the next message.
    pub async fn recv(&mut self) -> Result<Bytes, PubSubError> {
        self.receiver
            .recv()
            .await
            .map_err(|_| PubSubError::ChannelClosed)
    }

    /// Get the topic this subscription is for.
    pub fn topic(&self) -> &str {
        &self.topic
    }
}

/// Pub/sub trait for message broadcasting.
///
/// Implementations:
/// - [`InProcessPubSub`] — In-process broadcast channels (single-node)
/// - NATS adapter (multi-node, in `reactor-cache`)
/// - Redis Streams adapter (multi-node, in `reactor-cache`)
#[async_trait]
pub trait PubSub: Send + Sync {
    /// Publish a message to a topic.
    ///
    /// # Arguments
    /// * `topic` — Topic name (e.g., "data:org123:todos")
    /// * `message` — Message payload (typically JSON-encoded)
    async fn publish(&self, topic: &str, message: Bytes) -> Result<(), PubSubError>;

    /// Subscribe to a topic.
    ///
    /// Returns a `Subscription` that can be used to receive messages.
    async fn subscribe(&self, topic: &str) -> Result<Subscription, PubSubError>;

    /// Unsubscribe from a topic.
    ///
    /// The subscription should no longer receive messages after this call.
    async fn unsubscribe(&self, topic: &str) -> Result<(), PubSubError>;

    /// Check if connected to the pub/sub backend.
    async fn is_connected(&self) -> bool;
}

/// In-process pub/sub implementation using tokio broadcast channels.
///
/// Suitable for single-node deployments (G1/G2). Messages are not persisted
/// and do not cross process boundaries.
#[derive(Debug)]
pub struct InProcessPubSub {
    channels: RwLock<HashMap<String, broadcast::Sender<Bytes>>>,
    channel_capacity: usize,
}

impl InProcessPubSub {
    /// Create a new in-process pub/sub with default capacity (1024 messages).
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    /// Create a new in-process pub/sub with specified channel capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            channel_capacity: capacity,
        }
    }

    /// Get or create a channel for a topic.
    async fn get_or_create_channel(&self, topic: &str) -> broadcast::Sender<Bytes> {
        // Fast path: check if channel exists
        {
            let channels: tokio::sync::RwLockReadGuard<'_, HashMap<String, broadcast::Sender<Bytes>>> =
                self.channels.read().await;
            if let Some(sender) = channels.get(topic) {
                return sender.clone();
            }
        }

        // Slow path: create new channel
        let mut channels: tokio::sync::RwLockWriteGuard<'_, HashMap<String, broadcast::Sender<Bytes>>> =
            self.channels.write().await;
        // Double-check after acquiring write lock
        if let Some(sender) = channels.get(topic) {
            return sender.clone();
        }

        let (sender, _): (broadcast::Sender<Bytes>, broadcast::Receiver<Bytes>) =
            broadcast::channel(self.channel_capacity);
        channels.insert(topic.to_string(), sender.clone());
        sender
    }
}

impl Default for InProcessPubSub {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PubSub for InProcessPubSub {
    async fn publish(&self, topic: &str, message: Bytes) -> Result<(), PubSubError> {
        let sender = self.get_or_create_channel(topic).await;
        // send() only fails if there are no receivers, which is fine
        let _ = sender.send(message);
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<Subscription, PubSubError> {
        let sender = self.get_or_create_channel(topic).await;
        let receiver = sender.subscribe();
        Ok(Subscription::new(topic.to_string(), receiver))
    }

    async fn unsubscribe(&self, _topic: &str) -> Result<(), PubSubError> {
        // For in-process, unsubscribe is a no-op — just drop the Subscription
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        true // Always connected for in-process
    }
}

/// Create a shared in-process pub/sub instance.
#[must_use]
pub fn in_process_pubsub() -> Arc<dyn PubSub> {
    Arc::new(InProcessPubSub::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let pubsub = InProcessPubSub::new();

        let mut sub = pubsub.subscribe("test-topic").await.unwrap();

        // Publish a message
        pubsub
            .publish("test-topic", Bytes::from("hello"))
            .await
            .unwrap();

        // Receive it
        let msg = sub.recv().await.unwrap();
        assert_eq!(msg, Bytes::from("hello"));
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let pubsub = InProcessPubSub::new();

        let mut sub1 = pubsub.subscribe("topic").await.unwrap();
        let mut sub2 = pubsub.subscribe("topic").await.unwrap();

        pubsub
            .publish("topic", Bytes::from("broadcast"))
            .await
            .unwrap();

        let msg1 = sub1.recv().await.unwrap();
        let msg2 = sub2.recv().await.unwrap();

        assert_eq!(msg1, Bytes::from("broadcast"));
        assert_eq!(msg2, Bytes::from("broadcast"));
    }

    #[tokio::test]
    async fn test_publish_no_subscribers() {
        let pubsub = InProcessPubSub::new();

        // Should not error even with no subscribers
        let result = pubsub.publish("nobody", Bytes::from("ignored")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_is_connected() {
        let pubsub = InProcessPubSub::new();
        assert!(pubsub.is_connected().await);
    }

    #[tokio::test]
    async fn test_topic_isolation() {
        let pubsub = InProcessPubSub::new();

        let mut sub_a = pubsub.subscribe("topic-a").await.unwrap();
        let _sub_b = pubsub.subscribe("topic-b").await.unwrap();

        pubsub
            .publish("topic-a", Bytes::from("for a"))
            .await
            .unwrap();

        let msg = sub_a.recv().await.unwrap();
        assert_eq!(msg, Bytes::from("for a"));

        // sub_b should not receive the message (would block/timeout)
    }
}
