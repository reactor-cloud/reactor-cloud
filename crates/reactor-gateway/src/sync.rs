//! Sync loop for applying routing changes to Caddy.
//!
//! This module listens for changes to the routing table and applies
//! them to Caddy atomically.

use crate::caddy_admin::CaddyAdminClient;
use crate::error::GatewayResult;
use crate::routing::RoutingTable;
use crate::snapshot::SnapshotManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

/// Message types for the sync loop.
#[derive(Debug)]
pub enum SyncMessage {
    /// Reload the full routing table.
    Reload,
    /// A specific route changed.
    RouteChanged { host: String },
    /// Shutdown the sync loop.
    Shutdown,
}

/// Configuration for the sync loop.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Debounce interval for batching changes.
    pub debounce_interval: Duration,
    /// Maximum time to wait before forcing a sync.
    pub max_debounce_wait: Duration,
    /// Health check interval.
    pub health_check_interval: Duration,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            debounce_interval: Duration::from_millis(100),
            max_debounce_wait: Duration::from_secs(1),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// A trait for loading the routing table.
#[async_trait::async_trait]
pub trait RoutingTableLoader: Send + Sync {
    /// Load the full routing table.
    async fn load(&self) -> GatewayResult<RoutingTable>;
}

/// The sync loop that applies routing changes to Caddy.
pub struct SyncLoop<L: RoutingTableLoader> {
    loader: Arc<L>,
    snapshot_manager: Arc<SnapshotManager>,
    config: SyncConfig,
    receiver: mpsc::Receiver<SyncMessage>,
}

impl<L: RoutingTableLoader + 'static> SyncLoop<L> {
    /// Create a new sync loop.
    pub fn new(
        loader: Arc<L>,
        caddy_client: Arc<CaddyAdminClient>,
        config: SyncConfig,
    ) -> (Self, mpsc::Sender<SyncMessage>) {
        let (sender, receiver) = mpsc::channel(100);
        let snapshot_manager = Arc::new(SnapshotManager::new(caddy_client));

        let sync_loop = Self {
            loader,
            snapshot_manager,
            config,
            receiver,
        };

        (sync_loop, sender)
    }

    /// Run the sync loop.
    pub async fn run(mut self) -> GatewayResult<()> {
        info!("Starting sync loop");

        // Initial load
        self.do_sync().await?;

        let mut pending_changes = false;
        let mut last_change = Instant::now();
        let mut last_sync = Instant::now();

        loop {
            let timeout = if pending_changes {
                self.config.debounce_interval
            } else {
                self.config.health_check_interval
            };

            tokio::select! {
                msg = self.receiver.recv() => {
                    match msg {
                        Some(SyncMessage::Shutdown) | None => {
                            info!("Sync loop shutting down");
                            break;
                        }
                        Some(SyncMessage::Reload) => {
                            debug!("Received reload message");
                            pending_changes = true;
                            last_change = Instant::now();
                        }
                        Some(SyncMessage::RouteChanged { host }) => {
                            debug!("Route changed: {}", host);
                            pending_changes = true;
                            last_change = Instant::now();
                        }
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    // Check if we should sync
                    let should_sync = pending_changes && (
                        last_change.elapsed() >= self.config.debounce_interval ||
                        last_sync.elapsed() >= self.config.max_debounce_wait
                    );

                    if should_sync {
                        match self.do_sync().await {
                            Ok(()) => {
                                pending_changes = false;
                                last_sync = Instant::now();
                            }
                            Err(e) => {
                                error!("Sync failed: {}", e);
                                // Will retry on next iteration
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Perform a sync.
    async fn do_sync(&self) -> GatewayResult<()> {
        let start = Instant::now();
        debug!("Starting sync");

        // Load the full routing table
        let table = self.loader.load().await?;
        let route_count = table.len();

        debug!("Loaded {} routes in {:?}", route_count, start.elapsed());

        // Apply to Caddy
        self.snapshot_manager.apply(&table).await?;

        info!(
            "Sync completed: {} routes in {:?}",
            route_count,
            start.elapsed()
        );

        Ok(())
    }
}

/// A simple handle for sending sync messages.
#[derive(Clone)]
pub struct SyncHandle {
    sender: mpsc::Sender<SyncMessage>,
}

impl SyncHandle {
    /// Create a new sync handle.
    pub fn new(sender: mpsc::Sender<SyncMessage>) -> Self {
        Self { sender }
    }

    /// Request a full reload.
    pub async fn reload(&self) -> GatewayResult<()> {
        self.sender
            .send(SyncMessage::Reload)
            .await
            .map_err(|e| crate::error::GatewayError::internal(e.to_string()))?;
        Ok(())
    }

    /// Notify of a route change.
    pub async fn route_changed(&self, host: impl Into<String>) -> GatewayResult<()> {
        self.sender
            .send(SyncMessage::RouteChanged { host: host.into() })
            .await
            .map_err(|e| crate::error::GatewayError::internal(e.to_string()))?;
        Ok(())
    }

    /// Request shutdown.
    pub async fn shutdown(&self) -> GatewayResult<()> {
        self.sender
            .send(SyncMessage::Shutdown)
            .await
            .map_err(|e| crate::error::GatewayError::internal(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLoader;

    #[async_trait::async_trait]
    impl RoutingTableLoader for MockLoader {
        async fn load(&self) -> GatewayResult<RoutingTable> {
            Ok(RoutingTable::new())
        }
    }

    #[test]
    fn test_sync_config_defaults() {
        let config = SyncConfig::default();
        assert_eq!(config.debounce_interval, Duration::from_millis(100));
        assert_eq!(config.max_debounce_wait, Duration::from_secs(1));
    }
}
