//! Connector runtime abstraction.
//!
//! The `ConnectorRuntime` trait is the single load-bearing abstraction.
//! Every adapter (native, manifest, airbyte-container) implements it.

#[cfg(feature = "runtime-native")]
pub mod native;

#[cfg(feature = "runtime-manifest")]
pub mod manifest;

#[cfg(feature = "runtime-native")]
pub use native::{NativeConnector, NativeRuntime};

#[cfg(feature = "runtime-manifest")]
pub use manifest::ManifestRuntime;

use crate::descriptor::{ConnectorDescriptor, ConnectorTypeId};
use crate::error::ConnectError;
use crate::protocol::{ConfiguredCatalog, ConnectorMessage, ConnectionStatus, DiscoveredCatalog, StateBundle, SyncLimits, WriteOutcome};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

/// Runtime kind identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    /// First-party Rust connectors.
    Native,
    /// Airbyte Low-Code CDK YAML interpreter.
    Manifest,
    /// Airbyte container runner (via reactor-jobs).
    AirbyteContainer,
}

impl std::fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeKind::Native => write!(f, "native"),
            RuntimeKind::Manifest => write!(f, "manifest"),
            RuntimeKind::AirbyteContainer => write!(f, "airbyte"),
        }
    }
}

/// Options for action invocation.
#[derive(Debug, Clone, Default)]
pub struct ActionOpts {
    /// Whether to run in dry-run mode.
    pub dry_run: bool,
    /// Idempotency key for deduplication.
    pub idempotency_key: Option<String>,
}

/// Stream of connector messages.
pub type MessageStream = BoxStream<'static, Result<ConnectorMessage, ConnectError>>;

/// The connector runtime trait.
///
/// Every adapter implements this trait. Nothing else is allowed to touch
/// a connector directly.
#[async_trait]
pub trait ConnectorRuntime: Send + Sync + 'static {
    /// Get the runtime kind.
    fn kind(&self) -> RuntimeKind;

    /// List available connector types.
    async fn list_types(&self) -> Result<Vec<ConnectorTypeId>, ConnectError>;

    /// Get the descriptor for a connector type.
    ///
    /// For Native this is in-memory; for Manifest this parses a YAML file;
    /// for AirbyteContainer this calls `spec`.
    async fn descriptor(&self, type_id: &ConnectorTypeId) -> Result<ConnectorDescriptor, ConnectError>;

    /// Verify credentials work end-to-end. Cheap call (auth probe).
    ///
    /// Returns ConnectionStatus::Succeeded | Failed { code, cause, suggested_fix }.
    async fn check(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
    ) -> Result<ConnectionStatus, ConnectError>;

    /// Schema discovery: returns the catalog of available streams.
    ///
    /// Some connectors discover at runtime (Salesforce custom objects);
    /// others are static.
    async fn discover(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
    ) -> Result<DiscoveredCatalog, ConnectError>;

    /// Stream a sync run: produces a MessageStream of Airbyte-compatible records + state.
    ///
    /// The caller (stream::exec) writes records to the destination and persists state.
    async fn read(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        catalog: &ConfiguredCatalog,
        state: Option<&StateBundle>,
        limits: &SyncLimits,
    ) -> Result<MessageStream, ConnectError>;

    /// Invoke a typed action.
    ///
    /// Returns either real output or a synthesized dry-run preview.
    async fn invoke_action(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        action: &str,
        input: &serde_json::Value,
        opts: &ActionOpts,
    ) -> Result<serde_json::Value, ConnectError>;

    /// Outbound stream write: deliver records *to* the third party.
    ///
    /// Used for reverse sync (Postgres → Salesforce) when the StreamDescriptor
    /// declares supports_outbound.
    async fn write(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        stream: &str,
        records: MessageStream,
        limits: &SyncLimits,
    ) -> Result<WriteOutcome, ConnectError>;
}

/// A boxed connector runtime.
pub type BoxedRuntime = Box<dyn ConnectorRuntime>;
