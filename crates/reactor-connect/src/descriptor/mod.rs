//! Connector descriptor types.
//!
//! This module defines the shape of connector descriptors — the contract
//! that all connectors (native, manifest, or container) must satisfy.

mod auth_shape;
mod validate;

pub use auth_shape::*;

use serde::{Deserialize, Serialize};

/// Connector type identifier.
///
/// Examples: "stripe", "salesforce", "airbyte:facebook-marketing"
pub type ConnectorTypeId = String;

/// Complete descriptor for a connector type.
///
/// Every connector — whether native Rust, YAML manifest, or Airbyte container —
/// produces one of these. This is the single shape agents reason about.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDescriptor {
    /// Unique type identifier (e.g., "stripe", "salesforce").
    pub type_id: ConnectorTypeId,

    /// Human-readable display name.
    pub display_name: String,

    /// Connector version (semver).
    pub version: String,

    /// Runtime that handles this connector.
    pub runtime: crate::runtime::RuntimeKind,

    /// Authentication configuration.
    pub auth: AuthDescriptor,

    /// Available streams for data replication.
    pub streams: Vec<StreamDescriptor>,

    /// Available actions (typed RPC calls).
    pub actions: Vec<ActionDescriptor>,

    /// Available webhook receivers.
    pub webhooks: Vec<WebhookDescriptor>,

    /// Connector capabilities.
    pub capabilities: ConnectorCapabilities,

    /// Rate limits for API calls.
    #[serde(default)]
    pub rate_limits: Option<RateLimitDescriptor>,

    /// Documentation URL.
    #[serde(default)]
    pub doc_url: Option<String>,
}

/// Descriptor for a data stream (replication).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDescriptor {
    /// Stream name (e.g., "charges", "Lead", "issues").
    pub name: String,

    /// JSON Schema (draft-07) for records.
    pub json_schema: serde_json::Value,

    /// Supported sync modes.
    pub supported_modes: Vec<SyncMode>,

    /// Cursor field path for incremental sync (dot-separated).
    #[serde(default)]
    pub cursor_field: Option<Vec<String>>,

    /// Primary key paths (composite keys supported).
    #[serde(default)]
    pub primary_key: Option<Vec<Vec<String>>>,

    /// Whether this stream can be a destination (for outbound/reverse sync).
    #[serde(default)]
    pub supports_outbound: bool,

    /// Whether the source can define streams dynamically.
    #[serde(default)]
    pub source_defined: bool,
}

/// Sync mode for streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// Full refresh: reload all data each sync.
    FullRefresh,
    /// Incremental append: only new records.
    IncrementalAppend,
    /// Incremental with deduplication by primary key.
    IncrementalDedup,
}

/// Descriptor for an action (typed RPC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescriptor {
    /// Action name (e.g., "createLead", "postMessage").
    pub name: String,

    /// JSON Schema for input.
    pub input_schema: serde_json::Value,

    /// JSON Schema for output.
    pub output_schema: serde_json::Value,

    /// Side effect classification.
    pub side_effects: SideEffectKind,

    /// Dry-run support level.
    pub dry_run: DryRunSupport,

    /// Idempotency configuration.
    #[serde(default)]
    pub idempotency: Option<IdempotencyHint>,
}

/// Classification of side effects for an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectKind {
    /// Action only reads data.
    Reads,
    /// Action mutates data.
    Mutates,
    /// Action sends messages/notifications.
    Sends,
}

/// Dry-run support level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryRunSupport {
    /// Vendor exposes a real dry-run mode (e.g., Stripe test keys).
    Native,
    /// Reactor synthesizes the outbound request without sending.
    Synthesized,
    /// Action cannot be dry-run.
    Unsupported,
}

/// Idempotency configuration for an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyHint {
    /// JSON path to the idempotency key in the input.
    pub key_path: String,
    /// TTL in seconds for the idempotency key.
    pub ttl_seconds: u64,
}

/// Descriptor for a webhook receiver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDescriptor {
    /// Webhook name (e.g., "events", "platform_events").
    pub name: String,

    /// Signature verification method.
    pub verification: VerificationKind,

    /// Declared event types ("*" if open-ended).
    pub event_types: Vec<String>,

    /// Replay protection window in seconds.
    #[serde(default = "default_replay_window")]
    pub replay_window_seconds: u64,

    /// Setup instructions (markdown).
    pub setup_instructions: String,
}

fn default_replay_window() -> u64 {
    300 // 5 minutes
}

/// Webhook signature verification method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerificationKind {
    /// HMAC-SHA256 signature.
    HmacSha256 {
        /// Header containing the signature.
        header: String,
        /// Field name in credentials containing the secret.
        secret_field: String,
    },
    /// Ed25519 signature (e.g., GitHub).
    Ed25519 {
        /// Header containing the signature.
        header: String,
        /// Header containing the public key ID.
        key_id_header: Option<String>,
    },
    /// Custom verification (connector handles it).
    Custom {
        /// Documentation URL for the verification method.
        docs_url: String,
    },
}

/// Authentication descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDescriptor {
    /// Authentication method.
    pub kind: AuthKind,

    /// Required credential fields.
    pub fields: Vec<AuthField>,

    /// Optional test call to verify credentials.
    #[serde(default)]
    pub test: Option<TestCallDescriptor>,
}

/// Authentication method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthKind {
    /// OAuth2 authentication.
    OAuth2 {
        /// Authorization URL.
        authorize_url: String,
        /// Token URL.
        token_url: String,
        /// Required scopes.
        scopes: Vec<String>,
        /// Whether to use PKCE.
        #[serde(default)]
        pkce: bool,
        /// Placeholder for future Reactor-hosted proxy.
        #[serde(default)]
        reactor_proxy: bool,
    },
    /// Personal access token.
    PersonalAccessToken {
        /// Header to send the token in.
        header: String,
        /// Token format (e.g., "Bearer {token}").
        format: String,
    },
    /// HTTP Basic authentication.
    Basic,
    /// Custom authentication.
    Custom {
        /// Documentation URL.
        docs_url: String,
    },
}

/// A credential field that the user must supply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthField {
    /// Field name (used as key in credentials JSON).
    pub name: String,
    /// Human-readable label.
    pub label: String,
    /// Whether this field is sensitive (should be masked in UI).
    #[serde(default)]
    pub sensitive: bool,
    /// Whether this field is required.
    #[serde(default = "default_true")]
    pub required: bool,
    /// Description/help text.
    #[serde(default)]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Test call descriptor for credential verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCallDescriptor {
    /// HTTP method.
    pub method: String,
    /// Path to call.
    pub path: String,
    /// Expected status codes for success.
    #[serde(default = "default_success_codes")]
    pub success_codes: Vec<u16>,
}

fn default_success_codes() -> Vec<u16> {
    vec![200, 201, 204]
}

/// Connector capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectorCapabilities {
    /// Supports sandbox mode.
    #[serde(default)]
    pub sandbox_mode: bool,
    /// Has vendor test mode (e.g., Stripe test keys).
    #[serde(default)]
    pub vendor_test_mode: bool,
    /// Supports CDC (change data capture).
    #[serde(default)]
    pub cdc: bool,
    /// Supports incremental sync.
    #[serde(default)]
    pub incremental: bool,
    /// Supports schema discovery.
    #[serde(default)]
    pub schema_discovery: bool,
}

/// Rate limit descriptor for a connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitDescriptor {
    /// Requests per second limit.
    #[serde(default)]
    pub requests_per_second: Option<u32>,
    /// Requests per minute limit.
    #[serde(default)]
    pub requests_per_minute: Option<u32>,
    /// Requests per hour limit.
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
    /// Requests per day limit.
    #[serde(default)]
    pub requests_per_day: Option<u32>,
    /// Concurrent request limit.
    #[serde(default)]
    pub concurrent_requests: Option<u32>,
}
