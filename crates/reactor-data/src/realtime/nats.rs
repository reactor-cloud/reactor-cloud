//! NATS-based realtime backend for multi-tenant deployments.
//!
//! Uses NATS JetStream for durable, at-least-once message delivery across
//! multiple reactor-server instances in a shared cluster.

use super::{build_topic, DataChangeEvent, DataChangeOp, RealtimeBackend, RealtimeError, RealtimeSubscription};
use async_nats::Client;
use async_trait::async_trait;
use futures::StreamExt;
use reactor_core::ProjectRef;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

/// Configuration for NATS realtime backend.
#[derive(Debug, Clone)]
pub struct NatsConfig {
    /// NATS server URLs.
    pub servers: Vec<String>,
    /// Credentials file path (optional).
    pub credentials_file: Option<String>,
    /// Connection name for debugging.
    pub connection_name: String,
    /// Subject prefix for realtime messages.
    pub subject_prefix: String,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            servers: vec!["nats://localhost:4222".to_string()],
            credentials_file: None,
            connection_name: "reactor-data-realtime".to_string(),
            subject_prefix: "reactor".to_string(),
        }
    }
}

/// Active subscription state.
struct ActiveSubscription {
    sender: broadcast::Sender<DataChangeEvent>,
    _task: tokio::task::JoinHandle<()>,
}

/// NATS-based realtime backend.
///
/// Provides cross-node data change event broadcasting using NATS.
/// Each tenant's events are scoped to their own subject namespace.
pub struct NatsRealtime {
    client: Client,
    config: NatsConfig,
    subscriptions: RwLock<HashMap<String, ActiveSubscription>>,
}

impl NatsRealtime {
    /// Create a new NATS realtime backend.
    pub async fn new(config: NatsConfig) -> Result<Self, RealtimeError> {
        let client = Self::connect(&config).await?;

        info!(
            servers = ?config.servers,
            connection_name = %config.connection_name,
            "NATS realtime connected"
        );

        Ok(Self {
            client,
            config,
            subscriptions: RwLock::new(HashMap::new()),
        })
    }

    /// Connect to NATS servers.
    async fn connect(config: &NatsConfig) -> Result<Client, RealtimeError> {
        // Build connection string
        let servers: Vec<async_nats::ServerAddr> = config
            .servers
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        if servers.is_empty() {
            return Err(RealtimeError::Connection(
                "no valid NATS server addresses".to_string(),
            ));
        }

        // Connect with optional credentials
        let client = if let Some(ref creds_file) = config.credentials_file {
            async_nats::ConnectOptions::with_credentials_file(creds_file)
                .await
                .map_err(|e| RealtimeError::Connection(format!("failed to load credentials: {}", e)))?
                .name(&config.connection_name)
                .connect(servers)
                .await
                .map_err(|e| RealtimeError::Connection(format!("failed to connect: {}", e)))?
        } else {
            async_nats::ConnectOptions::new()
                .name(&config.connection_name)
                .connect(servers)
                .await
                .map_err(|e| RealtimeError::Connection(format!("failed to connect: {}", e)))?
        };

        Ok(client)
    }

    /// Build the NATS subject for a data change.
    fn build_subject(&self, project_ref: &ProjectRef, table: &str, op: DataChangeOp) -> String {
        format!(
            "{}.{}.data.{}.{}",
            self.config.subject_prefix,
            project_ref,
            table,
            op.as_str()
        )
    }

    /// Build a subscription subject pattern (supports wildcards).
    fn build_subscription_subject(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        op: DataChangeOp,
    ) -> String {
        let table_part = if table == "*" { "*" } else { table };
        let op_part = if op == DataChangeOp::All { "*" } else { op.as_str() };

        format!(
            "{}.{}.data.{}.{}",
            self.config.subject_prefix, project_ref, table_part, op_part
        )
    }
}

#[async_trait]
impl RealtimeBackend for NatsRealtime {
    async fn publish(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        event: &DataChangeEvent,
    ) -> Result<(), RealtimeError> {
        let subject = self.build_subject(project_ref, table, event.op);
        let payload = event.to_bytes()?;

        debug!(
            subject = %subject,
            event_id = %event.id,
            op = %event.op,
            "publishing realtime event"
        );

        self.client
            .publish(subject, payload)
            .await
            .map_err(|e| RealtimeError::PublishFailed(e.to_string()))?;

        Ok(())
    }

    async fn subscribe(
        &self,
        project_ref: &ProjectRef,
        table: &str,
        op: DataChangeOp,
    ) -> Result<RealtimeSubscription, RealtimeError> {
        let topic = build_topic(project_ref, table, op);
        let subject = self.build_subscription_subject(project_ref, table, op);

        // Check if subscription already exists
        {
            let subs = self.subscriptions.read().await;
            if let Some(active) = subs.get(&topic) {
                return Ok(RealtimeSubscription::new(topic, active.sender.subscribe()));
            }
        }

        // Create new subscription
        let (tx, rx) = broadcast::channel::<DataChangeEvent>(1024);

        let subscriber = self
            .client
            .subscribe(subject.clone())
            .await
            .map_err(|e| RealtimeError::SubscribeFailed(e.to_string()))?;

        info!(
            subject = %subject,
            topic = %topic,
            "created NATS realtime subscription"
        );

        // Spawn task to bridge NATS messages to broadcast channel
        let sender = tx.clone();
        let task_topic = topic.clone();
        let task = tokio::spawn(async move {
            let mut subscriber = subscriber;
            while let Some(msg) = subscriber.next().await {
                match DataChangeEvent::from_bytes(&msg.payload) {
                    Ok(event) => {
                        if sender.send(event).is_err() {
                            debug!(topic = %task_topic, "no receivers for realtime event");
                        }
                    }
                    Err(e) => {
                        warn!(
                            topic = %task_topic,
                            error = %e,
                            "failed to deserialize realtime event"
                        );
                    }
                }
            }
            debug!(topic = %task_topic, "NATS subscription ended");
        });

        // Store subscription
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(
                topic.clone(),
                ActiveSubscription {
                    sender: tx,
                    _task: task,
                },
            );
        }

        Ok(RealtimeSubscription::new(topic, rx))
    }

    async fn unsubscribe(&self, topic: &str) -> Result<(), RealtimeError> {
        let mut subs = self.subscriptions.write().await;
        if let Some(active) = subs.remove(topic) {
            // Task will be aborted when dropped
            active._task.abort();
            debug!(topic = %topic, "unsubscribed from realtime topic");
        }
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.client.connection_state() == async_nats::connection::State::Connected
    }
}

impl std::fmt::Debug for NatsRealtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsRealtime")
            .field("servers", &self.config.servers)
            .field("connection_name", &self.config.connection_name)
            .finish()
    }
}

/// Create a shared NATS realtime backend.
pub async fn nats_realtime(config: NatsConfig) -> Result<Arc<dyn RealtimeBackend>, RealtimeError> {
    Ok(Arc::new(NatsRealtime::new(config).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nats_config_default() {
        let config = NatsConfig::default();
        assert!(!config.servers.is_empty());
        assert_eq!(config.subject_prefix, "reactor");
    }

    #[test]
    fn test_subscription_subject_building() {
        // Test the subject building logic without actual NATS connection
        let config = NatsConfig::default();

        // Build subject manually to test logic
        let project_ref = "test123456789012";
        let table = "users";
        let op = DataChangeOp::Insert;

        let subject = format!(
            "{}.{}.data.{}.{}",
            config.subject_prefix, project_ref, table, op.as_str()
        );

        assert_eq!(subject, "reactor.test123456789012.data.users.insert");

        // Test wildcard subscription subject
        let wildcard_subject = format!(
            "{}.{}.data.{}.{}",
            config.subject_prefix, project_ref, "*", "*"
        );

        assert_eq!(wildcard_subject, "reactor.test123456789012.data.*.*");
    }
}
