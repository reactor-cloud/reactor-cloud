//! Native connector runtime.
//!
//! First-party Rust connectors implementing the `NativeConnector` trait.

use super::{ActionOpts, ConnectorRuntime, MessageStream, RuntimeKind};
use crate::descriptor::{ConnectorDescriptor, ConnectorTypeId};
use crate::error::ConnectError;
use crate::protocol::{ConfiguredCatalog, ConnectionStatus, DiscoveredCatalog, StateBundle, SyncLimits, WriteOutcome};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for native connector implementations.
#[async_trait]
pub trait NativeConnector: Send + Sync + 'static {
    /// Get the connector descriptor.
    fn descriptor(&self) -> ConnectorDescriptor;

    /// Check credentials.
    async fn check(&self, config: &serde_json::Value) -> Result<ConnectionStatus, ConnectError>;

    /// Discover available streams.
    async fn discover(&self, config: &serde_json::Value) -> Result<DiscoveredCatalog, ConnectError> {
        // Default: return streams from descriptor
        let desc = self.descriptor();
        Ok(DiscoveredCatalog {
            streams: desc.streams,
        })
    }

    /// Read records from a stream.
    async fn read(
        &self,
        _config: &serde_json::Value,
        _catalog: &ConfiguredCatalog,
        _state: Option<&StateBundle>,
        _limits: &SyncLimits,
    ) -> Result<MessageStream, ConnectError> {
        Err(ConnectError::Internal("read not implemented".to_string()))
    }

    /// Invoke an action.
    async fn invoke_action(
        &self,
        config: &serde_json::Value,
        action: &str,
        input: &serde_json::Value,
        opts: &ActionOpts,
    ) -> Result<serde_json::Value, ConnectError>;

    /// Write records to a stream (outbound).
    async fn write(
        &self,
        _config: &serde_json::Value,
        _stream: &str,
        _records: MessageStream,
        _limits: &SyncLimits,
    ) -> Result<WriteOutcome, ConnectError> {
        Err(ConnectError::Internal("write not implemented".to_string()))
    }
}

/// Native runtime that dispatches to registered connectors.
pub struct NativeRuntime {
    connectors: HashMap<ConnectorTypeId, Arc<dyn NativeConnector>>,
}

impl NativeRuntime {
    /// Create a new native runtime with the given connectors.
    pub fn new(connectors: HashMap<ConnectorTypeId, Arc<dyn NativeConnector>>) -> Self {
        Self { connectors }
    }

    /// Create a native runtime with all built-in connectors.
    #[cfg(feature = "runtime-native")]
    pub fn with_builtins() -> Self {
        use crate::connectors::{
            GitHubConnector, LinearConnector, SalesforceConnector, SlackConnector, StripeConnector,
        };

        let mut connectors: HashMap<ConnectorTypeId, Arc<dyn NativeConnector>> = HashMap::new();

        // Register built-in connectors (M1.6)
        connectors.insert(
            "stripe".to_string(),
            Arc::new(StripeConnector::new()),
        );
        connectors.insert(
            "slack".to_string(),
            Arc::new(SlackConnector::new()),
        );
        connectors.insert(
            "linear".to_string(),
            Arc::new(LinearConnector::new()),
        );
        connectors.insert(
            "github".to_string(),
            Arc::new(GitHubConnector::new()),
        );

        // Register M3.1 connector
        connectors.insert(
            "salesforce".to_string(),
            Arc::new(SalesforceConnector::new()),
        );

        Self { connectors }
    }

    fn get_connector(&self, type_id: &ConnectorTypeId) -> Result<&Arc<dyn NativeConnector>, ConnectError> {
        self.connectors
            .get(type_id)
            .ok_or_else(|| ConnectError::ConnectorTypeNotFound(type_id.clone()))
    }
}

#[async_trait]
impl ConnectorRuntime for NativeRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Native
    }

    async fn list_types(&self) -> Result<Vec<ConnectorTypeId>, ConnectError> {
        Ok(self.connectors.keys().cloned().collect())
    }

    async fn descriptor(&self, type_id: &ConnectorTypeId) -> Result<ConnectorDescriptor, ConnectError> {
        let connector = self.get_connector(type_id)?;
        Ok(connector.descriptor())
    }

    async fn check(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
    ) -> Result<ConnectionStatus, ConnectError> {
        let connector = self.get_connector(type_id)?;
        connector.check(config).await
    }

    async fn discover(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
    ) -> Result<DiscoveredCatalog, ConnectError> {
        let connector = self.get_connector(type_id)?;
        connector.discover(config).await
    }

    async fn read(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        catalog: &ConfiguredCatalog,
        state: Option<&StateBundle>,
        limits: &SyncLimits,
    ) -> Result<MessageStream, ConnectError> {
        let connector = self.get_connector(type_id)?;
        connector.read(config, catalog, state, limits).await
    }

    async fn invoke_action(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        action: &str,
        input: &serde_json::Value,
        opts: &ActionOpts,
    ) -> Result<serde_json::Value, ConnectError> {
        let connector = self.get_connector(type_id)?;
        connector.invoke_action(config, action, input, opts).await
    }

    async fn write(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        stream: &str,
        records: MessageStream,
        limits: &SyncLimits,
    ) -> Result<WriteOutcome, ConnectError> {
        let connector = self.get_connector(type_id)?;
        connector.write(config, stream, records, limits).await
    }
}
