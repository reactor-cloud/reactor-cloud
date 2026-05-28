//! Bundle manifest types.

use serde::{Deserialize, Serialize};

/// The top-level bundle manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    /// Project ID this bundle is for.
    pub project_id: String,

    /// Minimum reactor version required to apply this bundle.
    pub reactor_version: String,

    /// Timestamp when the bundle was created (ISO 8601).
    pub created_at: String,

    /// Per-capability manifest entries.
    pub capabilities: CapabilitiesManifest,
}

/// Per-capability manifest entries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitiesManifest {
    /// Data capability entries (migrations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<DataManifest>,

    /// Storage capability entries (policies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<Vec<StoragePolicyEntry>>,

    /// Functions capability entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<FunctionEntry>>,

    /// Jobs capability entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<JobEntry>>,

    /// Sites capability entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sites: Option<Vec<SiteEntry>>,

    /// Connect capability entries (connectors, connections).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect: Option<ConnectManifest>,
}

/// Data capability manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataManifest {
    /// Migration entries to apply.
    pub migrations: Vec<MigrationEntry>,
}

/// A single migration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationEntry {
    /// Migration filename (e.g., "001_init.sql").
    pub name: String,

    /// Path within the bundle archive.
    pub path: String,

    /// SHA-256 hash of the file contents.
    pub sha256: String,
}

/// Storage policy entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePolicyEntry {
    /// Bucket name the policy applies to.
    pub bucket: String,

    /// Policy name.
    pub name: String,

    /// Path within the bundle archive.
    pub path: String,

    /// SHA-256 hash of the file contents.
    pub sha256: String,
}

/// Function deployment entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionEntry {
    /// Function name.
    pub name: String,

    /// Path to the function bundle within the archive.
    pub path: String,

    /// SHA-256 hash of the function bundle.
    pub sha256: String,

    /// Runtime type (wasm, bun, lambda).
    pub runtime: String,
}

/// Job entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEntry {
    /// Job name.
    pub name: String,

    /// Associated function name.
    pub function_name: String,

    /// Path to the job manifest within the archive.
    pub path: String,

    /// SHA-256 hash of the job manifest.
    pub sha256: String,
}

/// Site deployment entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteEntry {
    /// Site name.
    pub name: String,

    /// Path to the site bundle within the archive.
    pub path: String,

    /// SHA-256 hash of the site bundle.
    pub sha256: String,

    /// Framework (nextjs, static, sveltekit, etc.).
    pub framework: String,
}

/// Connect capability manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectManifest {
    /// Connector instances to deploy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<ConnectInstanceEntry>,

    /// Connections to deploy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<ConnectConnectionEntry>,

    /// Receivers (webhooks) to deploy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receivers: Vec<ConnectReceiverEntry>,
}

/// Connector instance entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectInstanceEntry {
    /// Instance name.
    pub name: String,

    /// Connector type ID (e.g., "stripe", "hubspot").
    pub connector_type: String,

    /// Path to config file within the archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,

    /// SHA-256 hash of the config file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_sha256: Option<String>,
}

/// Connection entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectConnectionEntry {
    /// Connection name.
    pub name: String,

    /// Source instance name.
    pub source_instance: String,

    /// Destination type ("data", "storage").
    pub dest_type: String,

    /// Path to connection config within the archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,

    /// SHA-256 hash of the config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_sha256: Option<String>,
}

/// Receiver entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectReceiverEntry {
    /// Receiver name.
    pub name: String,

    /// Instance name.
    pub instance_name: String,

    /// Webhook name from the connector descriptor.
    pub webhook_name: String,

    /// Dispatch target type (job, stream, action, function).
    pub dispatch_type: String,

    /// Dispatch target name.
    pub dispatch_target: String,
}
