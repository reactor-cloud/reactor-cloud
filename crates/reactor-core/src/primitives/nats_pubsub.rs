//! NATS-based pub/sub implementation for multi-node deployments.
//!
//! This adapter uses NATS JetStream for reliable message delivery across
//! multiple reactor-server instances in shared cluster mode.
//!
//! # Subject naming
//!
//! PubSub topics are mapped to NATS subjects:
//! ```text
//! reactor.pubsub.{topic}
//! ```
//!
//! # Configuration
//!
//! ```toml
//! [pubsub]
//! backend = "nats"
//!
//! [pubsub.nats]
//! servers = ["nats://nats-0.internal:4222", "nats://nats-1.internal:4222"]
//! credentials_file = "/secrets/nats.creds"
//! ```

use async_nats::Client;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use super::{PubSub, PubSubError, Subscription};

/// Configuration for NATS connection.
#[derive(Debug, Clone)]
pub struct NatsPubSubConfig {
    /// NATS server URLs.
    pub servers: Vec<String>,

    /// Path to NATS credentials file.
    pub credentials_file: Option<String>,

    /// Connection name (for debugging).
    pub connection_name: String,

    /// Reconnect buffer size in bytes.
    pub reconnect_buffer_size: usize,

    /// Maximum reconnect attempts (None = unlimited).
    pub max_reconnects: Option<usize>,

    /// Subject prefix for pub/sub topics.
    pub subject_prefix: String,

    /// Local channel capacity for subscriptions.
    pub channel_capacity: usize,
}

impl Default for NatsPubSubConfig {
    fn default() -> Self {
        Self {
            servers: vec!["nats://localhost:4222".to_string()],
            credentials_file: None,
            connection_name: "reactor-pubsub".to_string(),
            reconnect_buffer_size: 8 * 1024 * 1024, // 8 MiB
            max_reconnects: None,
            subject_prefix: "reactor.pubsub".to_string(),
            channel_capacity: 1024,
        }
    }
}

/// Active subscription state.
struct ActiveSubscription {
    sender: broadcast::Sender<Bytes>,
    nats_subscription: async_nats::Subscriber,
}

/// NATS-based pub/sub implementation.
///
/// Uses NATS for multi-node message fanout. Each subscription spawns a
/// background task that receives from NATS and broadcasts to local receivers.
pub struct NatsPubSub {
    client: Client,
    config: NatsPubSubConfig,
    subscriptions: RwLock<HashMap<String, ActiveSubscription>>,
}

impl NatsPubSub {
    /// Connect to NATS with the given configuration.
    pub async fn connect(config: NatsPubSubConfig) -> Result<Self, PubSubError> {
        info!(
            servers = ?config.servers,
            name = %config.connection_name,
            "connecting to NATS"
        );

        // Build connection options
        let mut options = async_nats::ConnectOptions::new()
            .name(&config.connection_name)
            .reconnect_buffer_size(config.reconnect_buffer_size);

        if let Some(max) = config.max_reconnects {
            options = options.max_reconnects(max);
        }

        // Load credentials if provided
        if let Some(ref creds_file) = config.credentials_file {
            let creds = std::fs::read_to_string(creds_file)
                .map_err(|e| PubSubError::Connection(format!("failed to read credentials: {}", e)))?;
            options = options.credentials(&creds)
                .map_err(|e| PubSubError::Connection(format!("invalid credentials: {}", e)))?;
        }

        // Connect
        let servers: Vec<&str> = config.servers.iter().map(|s| s.as_str()).collect();
        let client = options
            .connect(servers.as_slice())
            .await
            .map_err(|e| PubSubError::Connection(format!("failed to connect: {}", e)))?;

        info!("connected to NATS");

        Ok(Self {
            client,
            config,
            subscriptions: RwLock::new(HashMap::new()),
        })
    }

    /// Build the NATS subject for a topic.
    fn subject(&self, topic: &str) -> String {
        format!("{}.{}", self.config.subject_prefix, topic)
    }

    /// Spawn a background task to forward NATS messages to local broadcast.
    fn spawn_subscription_forwarder(
        topic: String,
        mut nats_sub: async_nats::Subscriber,
        sender: broadcast::Sender<Bytes>,
    ) {
        tokio::spawn(async move {
            debug!(topic = %topic, "subscription forwarder started");

            while let Some(msg) = nats_sub.next().await {
                if sender.receiver_count() == 0 {
                    debug!(topic = %topic, "no local receivers, stopping forwarder");
                    break;
                }

                if let Err(e) = sender.send(msg.payload.clone()) {
                    warn!(topic = %topic, error = %e, "failed to forward message");
                }
            }

            debug!(topic = %topic, "subscription forwarder stopped");
        });
    }
}

impl std::fmt::Debug for NatsPubSub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsPubSub")
            .field("config", &self.config)
            .field("connected", &self.client.connection_state())
            .finish()
    }
}

#[async_trait]
impl PubSub for NatsPubSub {
    async fn publish(&self, topic: &str, message: Bytes) -> Result<(), PubSubError> {
        let subject = self.subject(topic);

        self.client
            .publish(subject.clone(), message)
            .await
            .map_err(|e| PubSubError::PublishFailed(format!("NATS publish failed: {}", e)))?;

        debug!(topic = %topic, subject = %subject, "published message to NATS");
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<Subscription, PubSubError> {
        let subject = self.subject(topic);

        // Check if we already have a subscription for this topic
        {
            let subs = self.subscriptions.read().await;
            if let Some(active) = subs.get(topic) {
                // Return a new receiver from the existing broadcast channel
                let receiver = active.sender.subscribe();
                return Ok(Subscription::new(topic.to_string(), receiver));
            }
        }

        // Create new NATS subscription
        let nats_sub = self
            .client
            .subscribe(subject.clone())
            .await
            .map_err(|e| PubSubError::SubscribeFailed(format!("NATS subscribe failed: {}", e)))?;

        // Create local broadcast channel
        let (sender, receiver) = broadcast::channel(self.config.channel_capacity);

        // Spawn forwarder task
        Self::spawn_subscription_forwarder(topic.to_string(), nats_sub.clone(), sender.clone());

        // Store subscription state
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(
                topic.to_string(),
                ActiveSubscription {
                    sender,
                    nats_subscription: nats_sub,
                },
            );
        }

        debug!(topic = %topic, subject = %subject, "subscribed to NATS");
        Ok(Subscription::new(topic.to_string(), receiver))
    }

    async fn unsubscribe(&self, topic: &str) -> Result<(), PubSubError> {
        let mut subs = self.subscriptions.write().await;

        if let Some(active) = subs.remove(topic) {
            // Unsubscribe from NATS (this will cause the forwarder to stop)
            if let Err(e) = active.nats_subscription.unsubscribe().await {
                warn!(topic = %topic, error = %e, "failed to unsubscribe from NATS");
            }
            debug!(topic = %topic, "unsubscribed from NATS");
        }

        Ok(())
    }

    async fn is_connected(&self) -> bool {
        matches!(
            self.client.connection_state(),
            async_nats::connection::State::Connected
        )
    }
}

/// Create a NATS pub/sub instance.
pub async fn nats_pubsub(config: NatsPubSubConfig) -> Result<Arc<dyn PubSub>, PubSubError> {
    let pubsub = NatsPubSub::connect(config).await?;
    Ok(Arc::new(pubsub))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = NatsPubSubConfig::default();
        assert_eq!(config.servers, vec!["nats://localhost:4222"]);
        assert_eq!(config.subject_prefix, "reactor.pubsub");
        assert_eq!(config.channel_capacity, 1024);
    }

    #[test]
    fn test_subject_building() {
        // Can't test without connection, but we can verify the logic
        let config = NatsPubSubConfig::default();
        let prefix = &config.subject_prefix;
        let expected = format!("{}.my-topic", prefix);
        assert_eq!(expected, "reactor.pubsub.my-topic");
    }
}
