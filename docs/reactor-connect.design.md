# `reactor-connect` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Eighth capability of the Reactor.cloud BaaS. Owns the Connect capability — agent-shaped integration with third-party systems via Actions, Streams, and Webhooks.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, connector descriptor, runtime trait, schema, policy integration — not implementation. Code lands in follow-up PRs against this doc.

`reactor-connect` is a **composition capability**: it orchestrates `reactor-vault` (credentials), `reactor-jobs` (durable execution + scheduling), `reactor-data` (default destination + state cursors), `reactor-storage` (large payload landing), `reactor-functions` (custom transformations), and `reactor-policy` (conflict resolution + authorization). It owns no new primitives — it owns the connector contract, the catalog, and the sandbox semantics that make agents trust it.

---

## 1. Goals

1. Expose a **single agent-shaped HTTP surface** for talking to any third-party system: discover, configure, test, sandbox, run. The same shape works for ingest (Stripe → Postgres), reverse-sync (Postgres → Salesforce), and discrete RPC (`slack.postMessage`).
2. Define a **connector contract** with three primitives — Streams, Actions, Webhooks — sharing one auth/credential model. All three ship at v0; this is non-negotiable because each one alone is half a story.
3. Ship a **native Rust runtime** as the default adapter, with first-party connectors for the long-tail apps actually need (Stripe, Salesforce, HubSpot, Slack, Linear, GitHub, Postgres-CDC, S3). G1/G2/G3 all run this with zero Python / zero Docker.
4. **Adopt the Airbyte wire protocol** (`AirbyteRecordMessage`, `AirbyteStateMessage`, `ConfiguredAirbyteCatalog`) as the Streams message format. Reactor's connector contract is a *superset* — adds Actions and Webhooks. Existing Airbyte connectors are interoperable at the wire level.
5. Ship a **manifest adapter** that interprets Airbyte's Low-Code Declarative YAML manifests in pure Rust. Unlocks ~100+ Airbyte connectors at v0 with no Python runtime. Users supply their own OAuth client IDs / secrets via `reactor-vault`.
6. Ship an **`airbyte-container` adapter** for the long tail. Runs an Airbyte connector image as a `reactor-jobs` step on Docker/Firecracker. G3-only; G1/G2 boot-in-seconds invariant is preserved.
7. **First-class sandboxing** as a verb on the surface: every Stream sync and every Action can be invoked in sandbox mode against an ephemeral destination / vendor test mode / dry-run output. Agents get a structured `{ records_read, schema_inferred, diff_vs_current, errors: [{ code, cause, suggested_fix }] }` back, not a log tail.
8. **Bidirectional sync as composition, not a new primitive**: a "two-way sync" is two stream bindings plus a conflict-resolution policy stored on the binding. Policy DSL is `reactor-policy`. Starts simple (`last_write_wins`, `source_of_truth`), can grow.
9. **Be the only crate allowed to touch `_reactor_connect.*`** Postgres metadata schema.

## 2. Non-goals (v0)

- **Visual flow builder.** CLI + descriptor files are the v0 DX. Studio surface lands later.
- **Bidirectional sync with field-level conflict resolution.** v0 supports binding-level policies (`last_write_wins`, `source_of_truth_a`, `source_of_truth_b`); per-field rules deferred.
- **Custom Rust connectors via plugin loading.** v0 native connectors live in-tree under `reactor-connect/connectors/`. Out-of-tree plugin SDK is v0.2.
- **CDC for arbitrary sources.** Only Postgres-CDC ships at v0 (via logical replication slot). MySQL/MongoDB CDC v0.2+.
- **Per-row encryption / field masking at the connector layer.** Use `reactor-vault` for credentials only. Data-at-rest is `reactor-data`'s concern.
- **OAuth client provisioning UI.** Users bring their own client IDs and secrets per provider, stored in `reactor-vault`. Reactor never holds a global OAuth app credential at v0.
- **Cross-org connections.** A connection lives in exactly one org; share data via `reactor-data` if needed.
- **Connector marketplace / payments.** All v0 connectors are first-party or community-contributed under the workspace license.
- **GraphQL connector type.** Manifest adapter is REST-shaped; GraphQL sources go through `airbyte-container` adapter or a custom native connector.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) ReactorId, AuthClient, AuthCtx, errors
├── reactor-policy/                # (existing) shared policy engine
├── reactor-auth/                  # (existing)
├── reactor-vault/                 # (existing) — connector credentials live here
├── reactor-data/                  # (existing) — default Stream destination, cursor storage
├── reactor-storage/               # (existing) — large payload landing, raw record archive
├── reactor-functions/             # (existing) — optional transform step
├── reactor-jobs/                  # (existing) — Stream syncs run here as durable jobs
│
├── reactor-connect/               # the connect library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_connect.*
│   │   ├── 001_metadata.sql       # connectors, instances
│   │   ├── 002_connections.sql    # streams bindings, action targets, webhook receivers
│   │   ├── 003_state.sql          # per-stream cursor / state messages
│   │   ├── 004_runs.sql           # sync run history
│   │   ├── 005_policies.sql       # conflict resolution policies
│   │   ├── 006_sandbox.sql        # ephemeral schema tracking
│   │   └── 007_audit.sql
│   ├── connectors/                # first-party native connector modules
│   │   ├── mod.rs
│   │   ├── stripe.rs              # Stripe (Streams: charges, customers, subs; Actions: refund, createCustomer; Webhooks: events)
│   │   ├── salesforce.rs          # Salesforce (Streams: Lead, Contact, Account, Opportunity; Actions: createLead; Webhooks: Platform Events)
│   │   ├── hubspot.rs
│   │   ├── slack.rs               # Actions: postMessage, openModal; Webhooks: events_api
│   │   ├── linear.rs              # Streams: issues; Actions: createIssue; Webhooks: webhooks
│   │   ├── github.rs              # Streams: PRs, issues; Actions: createIssue, mergePR; Webhooks: events
│   │   ├── postgres_cdc.rs        # logical replication slot, no destination side at v0
│   │   └── s3.rs                  # bucket-as-stream + file fetch action
│   ├── DESIGN.md                  # (this file, mirrored)
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # ConnectConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # ConnectState, ConnectCtx
│       ├── service.rs             # ConnectService (orchestrates discover → sandbox → run)
│       ├── error.rs               # ConnectError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs
│       │   ├── catalog.rs         # GET /connect/v1/catalog (available connector types)
│       │   ├── instances.rs       # configured connector instances + credentials
│       │   ├── connections.rs     # bindings (streams) + targets (actions) + receivers (webhooks)
│       │   ├── sandbox.rs         # POST .../sandbox endpoints
│       │   ├── runs.rs            # sync run listing, cancel, retry
│       │   ├── invoke.rs          # POST /connect/v1/instances/{i}/actions/{a}/invoke
│       │   ├── ingress.rs         # POST /connect/v1/ingress/{receiver_id} (webhook hot path)
│       │   └── logs.rs            # SSE log tail
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   ├── auth.rs            # bearer + X-Reactor-Org → ConnectCtx
│       │   └── ingress_verify.rs  # webhook signature verification per descriptor
│       │
│       ├── descriptor/            # connector descriptor format + validation
│       │   ├── mod.rs             # ConnectorDescriptor, StreamDescriptor, ActionDescriptor, WebhookDescriptor
│       │   ├── validate.rs        # schema + cross-field rules
│       │   ├── manifest_yaml.rs   # Airbyte low-code YAML → ConnectorDescriptor
│       │   └── auth_shape.rs      # OAuth2 / PAT / Basic / Custom credential shapes
│       │
│       ├── runtime/
│       │   ├── mod.rs             # ConnectorRuntime trait, MessageStream, ActionRequest/Response
│       │   ├── native.rs          # NativeRuntime: dispatches to crates/reactor-connect/connectors/*
│       │   ├── manifest.rs        # ManifestRuntime: interprets Airbyte low-code YAML in Rust
│       │   └── airbyte.rs         # AirbyteContainerRuntime: spawns container via reactor-jobs
│       │
│       ├── protocol/              # wire protocol (Airbyte-compatible + Reactor extensions)
│       │   ├── mod.rs
│       │   ├── airbyte.rs         # AirbyteMessage enum (Record, State, Log, Trace, Spec, Catalog, ConnectionStatus)
│       │   └── reactor.rs         # ReactorMessage::Action* / ::Webhook* extensions
│       │
│       ├── stream/                # Streams primitive (replication)
│       │   ├── mod.rs
│       │   ├── plan.rs            # SyncPlan (stream selection, mode, cursor, destination)
│       │   ├── exec.rs            # Sync execution loop: runtime → buffer → destination
│       │   ├── cursor.rs          # state checkpoint persistence (per binding)
│       │   ├── schema.rs          # schema discovery + drift detection
│       │   └── destination.rs     # DestinationSink trait + impls (reactor-data, reactor-storage)
│       │
│       ├── action/                # Actions primitive (typed RPC)
│       │   ├── mod.rs
│       │   ├── invoke.rs          # invocation pipeline: schema validate → policy → runtime
│       │   ├── dry_run.rs         # dry-run response synthesis
│       │   └── result.rs          # ActionResult shape
│       │
│       ├── webhook/               # Webhooks primitive (inbound events)
│       │   ├── mod.rs
│       │   ├── verify.rs          # signature verification per descriptor (hmac-sha256, ed25519, custom)
│       │   ├── dispatch.rs        # receiver → action / job / stream / function
│       │   └── replay.rs          # replay protection (timestamp + nonce)
│       │
│       ├── sandbox/
│       │   ├── mod.rs
│       │   ├── schema.rs          # ephemeral schema provisioning in reactor-data
│       │   ├── diff.rs            # diff_vs_current computation
│       │   ├── lifecycle.rs       # TTL + cleanup
│       │   └── vendor_test.rs     # vendor test-mode routing (Stripe test keys, SF sandbox orgs)
│       │
│       ├── policy/                # conflict resolution + invoke authz
│       │   ├── mod.rs
│       │   ├── conflict.rs        # ConflictPolicy DSL (built on reactor-policy)
│       │   └── invoke.rs          # action.* + stream.* builtins
│       │
│       ├── credentials/           # vault integration
│       │   ├── mod.rs
│       │   ├── oauth2.rs          # PKCE flow, token refresh
│       │   ├── pat.rs             # personal access tokens
│       │   └── refresh.rs         # background refresh worker
│       │
│       ├── store/
│       │   ├── mod.rs             # ConnectStore trait
│       │   └── postgres.rs        # PgConnectStore (sqlx)
│       │
│       └── audit.rs               # admin-event audit writer
│
├── reactor-connect-server/        # standalone bin
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # axum bind + tracing + migrate + serve
│       └── cli/
│           ├── mod.rs
│           └── doctor.rs          # connectivity diagnostics (DB, auth, vault, data, jobs)
│
└── packages/
    └── connect-sdk/               # TypeScript SDK
        ├── package.json
        └── src/
            ├── index.ts
            ├── client.ts          # ReactorConnect client
            ├── streams.ts         # streams API
            ├── actions.ts         # typed action wrappers (codegen from descriptor)
            └── webhooks.ts        # webhook receiver helpers
```

Conventions:
- `reactor-connect` depends on `reactor-core`, `reactor-policy`, `reactor-vault` (as a library), and the HTTP surfaces of `reactor-data`, `reactor-jobs`, `reactor-storage`, `reactor-functions`. It **never** depends on `reactor-auth` directly — auth is consumed through `AuthClient`.
- First-party connectors live in-tree at `crates/reactor-connect/connectors/` for v0. They use the same `ConnectorDescriptor` shape as YAML manifests — code paths are identical from the runtime's perspective.
- All three runtime adapters (`native`, `manifest`, `airbyte`) ship at v0, gated behind Cargo features (`runtime-native`, `runtime-manifest`, `runtime-airbyte`). A Tauri build (G1) drops `airbyte`.
- The Postgres adapter is *inside* `reactor-connect/src/store/postgres.rs` for v0. SQLite adapter (v0.2) follows the same split pattern as reactor-data.

---

## 4. Core types

### 4.1 ID & types

All IDs are `ReactorId` (UUIDv7). Connect-specific types:

| Type | Rust | Notes |
|---|---|---|
| `ConnectorTypeId` | `String` | Catalog key, e.g. `salesforce`, `stripe`, `airbyte:facebook-marketing` |
| `InstanceId` | `ReactorId` | A configured + credentialed connector instance |
| `ConnectionId` | `ReactorId` | A stream binding (source instance + dest instance/table) |
| `ReceiverId` | `ReactorId` | A webhook receiver, generates a stable ingress URL |
| `SyncRunId` | `ReactorId` | One sync run (delegates to reactor-jobs RunId internally) |
| `ActionInvocationId` | `ReactorId` | One action call (mirrors function InvocationId) |
| `RuntimeKind` | `enum { Native, Manifest, AirbyteContainer }` | Selected per ConnectorType, immutable |
| `SyncMode` | `enum { FullRefresh, IncrementalAppend, IncrementalDedup }` | Per-stream sync semantics |
| `ConnectionDirection` | `enum { Inbound, Outbound }` | Source-of-truth direction for the binding |

### 4.2 `ConnectCtx` (request-local)

```rust
// reactor-connect/src/state.rs
#[derive(Debug, Clone)]
pub struct ConnectCtx {
    pub auth:       AuthCtx,
    pub request_id: String,
    pub org_id:     OrgId,
}

impl ConnectCtx {
    pub fn user_id(&self) -> Option<&UserId> { self.auth.user_id() }
    pub fn active_org(&self) -> &OrgId { &self.org_id }
    pub fn has_permission(&self, perm: &str) -> bool { self.auth.has_permission(perm) }
}
```

### 4.3 `ConnectorDescriptor` — the load-bearing contract

Every connector — whether native Rust, YAML manifest, or Airbyte container — produces one of these. This is the single shape agents reason about.

```rust
// reactor-connect/src/descriptor/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDescriptor {
    pub type_id:      ConnectorTypeId,        // "stripe", "salesforce", "airbyte:facebook-marketing"
    pub display_name: String,
    pub version:      semver::Version,
    pub runtime:      RuntimeKind,
    pub auth:         AuthDescriptor,         // OAuth2 / PAT / Basic / Custom
    pub streams:      Vec<StreamDescriptor>,
    pub actions:      Vec<ActionDescriptor>,
    pub webhooks:     Vec<WebhookDescriptor>,
    pub capabilities: ConnectorCapabilities,  // sandbox_mode, vendor_test_mode, cdc, incremental, ...
    pub doc_url:      Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDescriptor {
    pub name:               String,                 // "charges", "Lead", "issues"
    pub json_schema:        serde_json::Value,      // JSON Schema draft-07
    pub supported_modes:    Vec<SyncMode>,
    pub cursor_field:       Option<Vec<String>>,    // dot-path; required for IncrementalAppend
    pub primary_key:        Option<Vec<Vec<String>>>, // composite supported
    pub supports_outbound:  bool,                   // can this stream be a *destination*?
    pub source_defined:     bool,                   // can the source define streams dynamically?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescriptor {
    pub name:           String,                 // "createLead", "postMessage", "refundCharge"
    pub input_schema:   serde_json::Value,      // JSON Schema
    pub output_schema:  serde_json::Value,
    pub side_effects:   SideEffectKind,         // Reads | Mutates | Sends
    pub dry_run:        DryRunSupport,          // Native | Synthesized | Unsupported
    pub idempotency:    Option<IdempotencyHint>,// key path, ttl
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDescriptor {
    pub name:               String,             // "events", "platform_events"
    pub verification:       VerificationKind,   // HmacSha256 { header, secret_field } | Ed25519 | Custom
    pub event_types:        Vec<String>,        // declared event taxonomy; "*" if open-ended
    pub replay_window:      Duration,           // for replay protection
    pub setup_instructions: String,             // markdown: how to wire it in the vendor UI
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDescriptor {
    pub kind:   AuthKind,                       // OAuth2 | PAT | Basic | Custom
    pub fields: Vec<AuthField>,                 // declares which secrets the user must supply
    pub test:   Option<TestCallDescriptor>,     // a free probe call: HEAD /me, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthKind {
    OAuth2 {
        authorize_url:  String,
        token_url:      String,
        scopes:         Vec<String>,
        pkce:           bool,
        // client_id + client_secret are supplied by the user, stored in vault
    },
    PersonalAccessToken { header: String, format: String },
    Basic,
    Custom { docs_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DryRunSupport {
    Native,         // the vendor exposes a real dry-run mode (Stripe test mode keys, etc.)
    Synthesized,    // Reactor synthesizes the outbound request without sending
    Unsupported,    // action cannot be dry-run; sandbox is the only safety net
}
```

The descriptor is what `discover` returns, what agents read, and what the CLI uses to generate typed clients.

### 4.4 `ConnectorRuntime` trait

```rust
// reactor-connect/src/runtime/mod.rs
#[async_trait]
pub trait ConnectorRuntime: Send + Sync + 'static {
    fn kind(&self) -> RuntimeKind;

    /// Return the descriptor for a connector type. For Native this is in-memory;
    /// for Manifest this parses a YAML file; for AirbyteContainer this calls `spec`.
    async fn descriptor(&self, ty: &ConnectorTypeId) -> Result<ConnectorDescriptor, ConnectError>;

    /// Verify credentials work end-to-end. Cheap call (auth probe).
    /// Returns ConnectionStatus::Succeeded | Failed { code, cause, suggested_fix }.
    async fn check(
        &self,
        ty:    &ConnectorTypeId,
        creds: &Credentials,
        cfg:   &serde_json::Value,
    ) -> Result<ConnectionStatus, ConnectError>;

    /// Schema discovery: returns the catalog of available streams.
    /// Some connectors discover at runtime (Salesforce custom objects); others are static.
    async fn discover(
        &self,
        ty:    &ConnectorTypeId,
        creds: &Credentials,
        cfg:   &serde_json::Value,
    ) -> Result<DiscoveredCatalog, ConnectError>;

    /// Stream a sync run: produces a MessageStream of Airbyte-compatible records + state.
    /// The caller (stream::exec) writes records to the destination and persists state.
    async fn read(
        &self,
        ty:        &ConnectorTypeId,
        creds:     &Credentials,
        cfg:       &serde_json::Value,
        catalog:   &ConfiguredCatalog,    // selected streams + modes
        state:     Option<&StateBundle>,  // prior state for incremental
        limits:    &SyncLimits,           // max rows, max duration, sandbox cap
    ) -> Result<MessageStream, ConnectError>;

    /// Invoke a typed action. Returns either real output or a synthesized dry-run preview.
    async fn invoke_action(
        &self,
        ty:        &ConnectorTypeId,
        creds:     &Credentials,
        cfg:       &serde_json::Value,
        action:    &str,
        input:     &serde_json::Value,
        opts:      &ActionOpts,           // dry_run, idempotency_key
    ) -> Result<ActionResult, ConnectError>;

    /// Outbound stream write: deliver records *to* the third party. Used for reverse
    /// sync (Postgres → Salesforce) when the StreamDescriptor declares supports_outbound.
    async fn write(
        &self,
        ty:        &ConnectorTypeId,
        creds:     &Credentials,
        cfg:       &serde_json::Value,
        stream:    &str,
        records:   MessageStream,
        limits:    &SyncLimits,
    ) -> Result<WriteOutcome, ConnectError>;
}

pub type MessageStream = BoxStream<'static, Result<ConnectorMessage, ConnectError>>;

/// Wire protocol — Airbyte-compatible with Reactor extensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "UPPERCASE")]
pub enum ConnectorMessage {
    Record(AirbyteRecordMessage),
    State(AirbyteStateMessage),
    Log(AirbyteLogMessage),
    Trace(AirbyteTraceMessage),
    Spec(AirbyteSpecMessage),
    Catalog(AirbyteCatalogMessage),
    ConnectionStatus(AirbyteConnectionStatus),
    // Reactor extensions:
    ActionResult(ActionResult),
    WebhookEvent(WebhookEvent),
}
```

**Streaming is mandatory.** `read` and `write` are streams end-to-end so memory stays bounded even on a multi-million-row backfill.

### 4.5 `DestinationSink` trait

Streams land somewhere. Each destination implements:

```rust
// reactor-connect/src/stream/destination.rs
#[async_trait]
pub trait DestinationSink: Send + Sync + 'static {
    async fn prepare(&self, schema: &DiscoveredSchema, mode: SyncMode) -> Result<(), ConnectError>;
    async fn write_batch(&self, stream: &str, records: Vec<serde_json::Value>) -> Result<usize, ConnectError>;
    async fn finalize(&self, stream: &str) -> Result<(), ConnectError>;
}
```

v0 sinks:
- `ReactorDataSink` — writes to a table in `reactor-data` (default).
- `ReactorStorageSink` — writes records as JSONL blobs to `reactor-storage` (raw archive, no schema enforcement).
- `EphemeralSink` — used for sandbox runs; provisions a temp schema in `reactor-data`, returns a diff at the end.
- `ConnectorSink` — for reverse sync; wraps `runtime.write(...)` on another connector instance.

### 4.6 `ConnectStore` trait

```rust
// reactor-connect/src/store/mod.rs
#[async_trait]
pub trait ConnectStore: Send + Sync + 'static {
    type Tx<'a>: ConnectTx where Self: 'a;
    async fn begin(&self) -> Result<Self::Tx<'_>, ConnectError>;

    // Instances
    async fn create_instance(&self, i: &NewInstance) -> Result<Instance, ConnectError>;
    async fn get_instance(&self, org: &OrgId, name: &str) -> Result<Option<Instance>, ConnectError>;
    async fn list_instances(&self, org: &OrgId) -> Result<Vec<Instance>, ConnectError>;
    async fn delete_instance(&self, id: &InstanceId) -> Result<(), ConnectError>;

    // Connections (stream bindings)
    async fn create_connection(&self, c: &NewConnection) -> Result<Connection, ConnectError>;
    async fn get_connection(&self, id: &ConnectionId) -> Result<Option<Connection>, ConnectError>;
    async fn list_connections(&self, org: &OrgId) -> Result<Vec<Connection>, ConnectError>;
    async fn set_connection_enabled(&self, id: &ConnectionId, enabled: bool) -> Result<(), ConnectError>;
    async fn delete_connection(&self, id: &ConnectionId) -> Result<(), ConnectError>;

    // Webhook receivers
    async fn create_receiver(&self, r: &NewReceiver) -> Result<Receiver, ConnectError>;
    async fn get_receiver_by_token(&self, token: &str) -> Result<Option<Receiver>, ConnectError>;
    async fn list_receivers(&self, instance_id: &InstanceId) -> Result<Vec<Receiver>, ConnectError>;

    // State (per-binding cursor / Airbyte state messages)
    async fn get_state(&self, conn_id: &ConnectionId, stream: &str) -> Result<Option<StateBundle>, ConnectError>;
    async fn put_state(&self, conn_id: &ConnectionId, stream: &str, state: &StateBundle) -> Result<(), ConnectError>;

    // Runs
    async fn record_run(&self, run: &SyncRunRecord) -> Result<(), ConnectError>;
    async fn list_runs(&self, conn_id: &ConnectionId, limit: u32) -> Result<Vec<SyncRunRecord>, ConnectError>;

    // Action invocations (lightweight; full audit handled separately)
    async fn record_invocation(&self, inv: &ActionInvocationRecord) -> Result<(), ConnectError>;

    // Policies
    async fn get_conflict_policy(&self, pair_id: &ReactorId) -> Result<Option<ConflictPolicy>, ConnectError>;
    async fn upsert_conflict_policy(&self, p: &ConflictPolicy) -> Result<(), ConnectError>;

    // Audit
    async fn write_audit_event(&self, e: &AuditEvent) -> Result<(), ConnectError>;
}
```

### 4.7 No `ConnectClient` trait

Mirrors reactor-data / reactor-functions: there is **no `ConnectClient` trait** in `reactor-core`. Other capabilities consume reactor-connect via its HTTP surface. The unified Reactor binary can embed `reactor_connect::router(state)` behind `tower::Service` for in-process calls.

---

## 5. HTTP surface (v0)

All routes are scoped to the active org (resolved by middleware). All admin routes require an authenticated user. Webhook ingress is the only anonymous route, gated by a stable per-receiver token + signature verification.

### 5.1 Health

```
GET    /connect/v1/health
       → 200 { "status": "ok", "version": "0.1.0", "runtimes": ["native", "manifest", "airbyte"] }
```

### 5.2 Catalog (available connector types)

```
GET    /connect/v1/catalog
       Query: ?runtime=native&search=salesforce
       → 200 [ { type_id, display_name, runtime, version, capabilities, doc_url }, ... ]

GET    /connect/v1/catalog/{type_id}
       → 200 { ...ConnectorDescriptor }
       Used by agents to introspect an entire connector before any setup.
```

The catalog merges entries from all enabled runtimes. `native:salesforce` and `airbyte:salesforce` may both exist; they have distinct `type_id`s.

### 5.3 Instances (configured connector + credentials)

```
POST   /connect/v1/instances
       Body: { "type_id": "salesforce", "name": "sf-prod", "config": { "instance_url": "..." } }
       → 201 { instance, oauth_url? }      // oauth_url present when AuthKind::OAuth2
       Requires: connect:instances:create
       Side-effects:
         - Reserves a vault path: secret/data/connect/{org_id}/instances/{instance_id}
         - For OAuth2: returns the authorize URL with PKCE challenge; user redirects, callback completes.

POST   /connect/v1/instances/{name}/credentials
       Body: { "client_id": "...", "client_secret": "...", "refresh_token": "..." }
             OR { "personal_access_token": "..." }
             OR { "code": "...", "redirect_uri": "..." }   // OAuth2 callback exchange
       → 200 { instance, credential_state: "ready" }
       Requires: connect:instances:{name}:admin
       Side-effects: secrets land in reactor-vault, never in metadata DB.

POST   /connect/v1/instances/{name}/check
       → 200 { "status": "ok" | "failed", "error": { code, cause, suggested_fix }? }
       Calls ConnectorRuntime::check — cheap auth probe.

GET    /connect/v1/instances
       → 200 [ instance, ... ]
GET    /connect/v1/instances/{name}
       → 200 { instance, credential_state, descriptor }
DELETE /connect/v1/instances/{name}
       → 204
       Cascades: disables connections referencing it, deletes vault path, marks receivers inactive.
```

### 5.4 Discovery

```
POST   /connect/v1/instances/{name}/discover
       Body: { "include_dynamic": true }
       → 200 { "catalog": [ StreamDescriptor, ... ], "discovered_at": "...", "duration_ms": ... }
       Calls ConnectorRuntime::discover. Result is also cached in metadata for 1h.
```

### 5.5 Connections (stream bindings)

```
POST   /connect/v1/connections
       Body: {
         "name": "salesforce-leads-to-postgres",
         "source":      { "instance": "sf-prod",  "streams": [ { "name": "Lead", "mode": "incremental_dedup", "primary_key": [["Id"]] } ] },
         "destination": { "kind": "data", "instance": null, "table": "salesforce_leads" },
         "schedule":    { "cron": "*/15 * * * *" } | { "on_event": "..." } | { "manual": true },
         "options":     { "schema_drift": "alert", "max_rows_per_run": 100000 }
       }
       → 201 { connection }
       Side-effects:
         - Creates a reactor-jobs job under the hood; this endpoint *is* the agent-shaped façade.
         - On first run, prepares destination schema via DestinationSink::prepare.

       Reverse direction:
         "source":      { "instance": null, "kind": "data", "table": "salesforce_leads" }
         "destination": { "instance": "sf-prod", "stream": "Lead" }

GET    /connect/v1/connections
GET    /connect/v1/connections/{name}
PATCH  /connect/v1/connections/{name}    Body: { enabled?, schedule?, options? }
DELETE /connect/v1/connections/{name}
```

### 5.6 Sandbox (the agent's safety net)

```
POST   /connect/v1/connections/{name}/sandbox
       Body: { "limit_rows": 100, "limit_seconds": 60 }
       → 200 {
           "ephemeral_schema": "_sandbox_<run_id>",
           "records_read":     { "Lead": 100 },
           "schema_inferred":  { ... },
           "diff_vs_current":  { "Lead": { "added_columns": [...], "type_changes": [...], "row_delta": +100 } },
           "errors":           [ { "code", "cause", "suggested_fix" }, ... ],
           "promote_token":    "..."
         }
       Side-effects:
         - Provisions ephemeral schema in reactor-data via EphemeralSink.
         - Auto-cleans after 1h unless promoted.

POST   /connect/v1/connections/{name}/promote
       Body: { "promote_token": "..." }
       → 200 { connection, schema_migration_applied }
       Side-effects:
         - Applies inferred schema changes to the real destination.
         - Enables the connection's schedule.

POST   /connect/v1/instances/{name}/actions/{action}/sandbox
       Body: { "input": { ... } }
       → 200 {
           "would_have_sent": { "method": "POST", "url": "...", "body": "..." },
           "estimated_cost":  { ... }?,
           "validation":      { "ok": true } | { "ok": false, "errors": [...] }
         }
       Behaviour by DryRunSupport:
         - Native      → connector switches to vendor test mode and actually executes there.
         - Synthesized → connector returns the outbound request it would have made, without sending.
         - Unsupported → 422 with { code: "dry_run_unsupported", suggested_fix: "use vendor test instance" }
```

### 5.7 Action invocation

```
POST   /connect/v1/instances/{name}/actions/{action}/invoke
       Body: { "input": { ... } }
       Headers: Idempotency-Key (optional; reused if descriptor declares idempotency)
       → 200 { "output": { ... }, "invocation_id": "..." }
       → 422 { code: "invalid_input", errors: [ JSON Schema errors ] }
       → 502 { code: "third_party_error", cause, retry_after_ms? }
       Requires: connect:actions:{name}:{action}:invoke + policy eval
```

### 5.8 Webhook ingress (anonymous hot path)

```
POST   /connect/v1/ingress/{receiver_token}
       Body: raw bytes from third party
       Headers: vendor-specific signature header
       → 200 { "event_id": "..." }
       → 401 if signature verification fails
       → 410 if receiver disabled
       Side-effects:
         - Signature verified per WebhookDescriptor.verification.
         - Replay protection (timestamp + nonce store in reactor-cache).
         - Dispatched per receiver config to: action invoke / job event / stream upsert / function call.
```

Receivers are created via the connection routes:

```
POST   /connect/v1/instances/{name}/receivers
       Body: { "webhook": "events", "dispatch": { "kind": "job", "name": "process-stripe-event" } }
                                  | { "kind": "stream", "connection": "stripe-events-to-pg" }
                                  | { "kind": "action", "instance": "...", "action": "..." }
                                  | { "kind": "function", "name": "..." }
       → 201 { receiver, ingress_url }       // ingress_url contains the receiver_token
```

### 5.9 Runs

```
GET    /connect/v1/connections/{name}/runs?status=failed&limit=50
GET    /connect/v1/connections/{name}/runs/{run_id}
POST   /connect/v1/connections/{name}/runs/{run_id}/cancel
POST   /connect/v1/connections/{name}/runs/{run_id}/retry
GET    /connect/v1/connections/{name}/runs/{run_id}/logs    -- SSE
```

Internally each run is a `reactor-jobs` run; the connect surface is the agent-shaped façade.

### 5.10 Conflict policies (bidirectional sync)

```
POST   /connect/v1/policies/conflict
       Body: {
         "pair": { "connection_a": "salesforce-to-pg", "connection_b": "pg-to-salesforce" },
         "rules": [
           { "stream": "Lead", "policy": "last_write_wins", "tiebreak": "source_of_truth_a" },
           { "stream": "Account", "policy": "source_of_truth_a" }
         ]
       }
       → 201 { policy }

GET    /connect/v1/policies/conflict
PATCH  /connect/v1/policies/conflict/{id}
DELETE /connect/v1/policies/conflict/{id}
```

Conflict rules are evaluated by `reactor-policy` at write time on the receiving side. v0 ships three policies; the DSL grows from there.

---

## 6. Connector descriptor — YAML / TOML form

Native connectors register their descriptor in code. YAML manifests (Airbyte low-code) are interpreted by `ManifestRuntime`. Both shapes flatten to the same `ConnectorDescriptor`.

```yaml
# connectors/stripe.connector.yaml (illustrative)
type_id: stripe
display_name: Stripe
version: 0.1.0
runtime: native
auth:
  kind: personal_access_token
  fields:
    - name: api_key
      label: Secret key
      sensitive: true
  test:
    method: GET
    path: /v1/account
streams:
  - name: charges
    json_schema: { $ref: "schemas/charge.json" }
    supported_modes: [incremental_append]
    cursor_field: [created]
    primary_key: [[id]]
    supports_outbound: false
  - name: customers
    json_schema: { $ref: "schemas/customer.json" }
    supported_modes: [incremental_dedup, full_refresh]
    primary_key: [[id]]
    supports_outbound: true
actions:
  - name: createCustomer
    input_schema: { $ref: "schemas/createCustomer.input.json" }
    output_schema: { $ref: "schemas/customer.json" }
    side_effects: mutates
    dry_run: native        # uses Stripe test mode
    idempotency:
      key_path: $.idempotency_key
      ttl_seconds: 86400
  - name: refundCharge
    input_schema: { ... }
    output_schema: { ... }
    side_effects: mutates
    dry_run: native
webhooks:
  - name: events
    verification:
      kind: hmac_sha256
      header: Stripe-Signature
      secret_field: webhook_signing_secret
    event_types: ["*"]
    replay_window: 5m
    setup_instructions: |
      In the Stripe dashboard, add a webhook endpoint pointing at the
      ingress URL returned when the receiver is created.
capabilities:
  sandbox_mode: native
  vendor_test_mode: true
  incremental: true
```

The Airbyte low-code YAML format is interpreted at parse time and compiled into the same `ConnectorDescriptor` — `streams` come from the manifest's `streams[]`, `actions` is empty (Airbyte connectors don't declare actions), `webhooks` is empty unless the user adds a `reactor:` extension block.

---

## 7. Wire protocol

Reactor's connector wire protocol is the **Airbyte Protocol with two Reactor extensions** (`ACTION_RESULT`, `WEBHOOK_EVENT`).

For Streams:
- `RECORD` — a single row, with stream name + emitted timestamp + data.
- `STATE` — checkpoint marker. Reactor persists the most recent `STATE` per connection+stream.
- `LOG`, `TRACE` — observability.
- `SPEC`, `CATALOG`, `CONNECTION_STATUS` — out-of-band lifecycle.

For Actions (Reactor extension):
- `ACTION_RESULT { invocation_id, output | error, mode: real | dry_run }`

For Webhooks (Reactor extension):
- `WEBHOOK_EVENT { receiver_id, event_id, payload, occurred_at }`

An Airbyte container connector emitting raw Airbyte messages just works; an action-capable native connector additionally emits `ACTION_RESULT`. The runtime tags every message with the originating `RuntimeKind` for diagnostics.

---

## 8. Sandbox semantics — the agent contract

Sandbox is the part that earns this capability. Three modes:

**Stream sandbox** (`POST /connections/{name}/sandbox`)
1. Provision ephemeral schema `_sandbox_<run_id>` in `reactor-data`.
2. Run the connector with a hard `limit_rows` and `limit_seconds`. No state is persisted to the real binding.
3. Compute a structured diff against the real destination's current schema + sample rows.
4. Return the diff plus a `promote_token` that, if passed back within 1h, atomically applies the schema migration and enables the binding.
5. After 1h, cleanup worker drops the ephemeral schema.

**Action sandbox** (`POST /actions/{name}/sandbox`)
- If `DryRunSupport::Native` — connector switches to vendor test mode and runs for real there. Stripe test keys, Salesforce sandbox orgs, GitHub test repos, etc. The connector's `AuthDescriptor` declares which credentials are "test" vs "live".
- If `DryRunSupport::Synthesized` — the runtime executes everything up to the outbound HTTP call, returns the request that *would* have been sent.
- If `DryRunSupport::Unsupported` — refuse with a `suggested_fix` pointing the user at vendor test instances.

**Webhook sandbox** (`POST /receivers/{id}/sandbox`)
- Accepts a sample payload, signature-verifies it as if it came from the vendor, dispatches it to the configured target *in sandbox mode* (action dry-run, stream → ephemeral schema, etc.).

The structured `errors` shape is the same everywhere:

```json
{ "code": "auth_token_expired", "cause": "refresh failed: 401 invalid_grant",
  "suggested_fix": "re-run `reactor connect instances credentials sf-prod` to re-authenticate",
  "docs_url": "https://docs.reactor.cloud/connect/troubleshooting#auth_token_expired" }
```

This is the contract agents read. Every connector author must produce these shapes; the runtime wraps raw vendor errors with a default `code: "third_party_error"` if the connector forgets.

---

## 9. Policy integration

Two places `reactor-policy` plugs in:

### 9.1 Invoke authorization

Per-action and per-stream policies, evaluated at invoke / sync time:

```text
policy stripe_refund_max on action stripe.refundCharge
  using action.input.amount <= 10000 and auth.has_permission("connect:stripe:refund:large")
```

Builtins:
- `action.input.*`, `action.name`, `action.instance`
- `stream.name`, `stream.connection`
- `auth.user_id()`, `auth.has_permission(p)`, `auth.org_id()`

### 9.2 Conflict resolution DSL

For bidirectional sync. v0 ships three named policies; the DSL allows custom rules:

```text
policy lead_priority on stream Lead
  conflict using
    when source_a.LastModifiedDate > source_b.updated_at then prefer source_a
    when source_a.OwnerId == "system" then prefer source_b
    else prefer source_a
```

Compiled to a `reactor-policy` expression evaluated per row at write time. v0 supports only stream-level rules (`last_write_wins`, `source_of_truth_a`, `source_of_truth_b`); the `when ... then prefer ...` grammar is the v0.2 extension.

---

## 10. The CRM migration story (locked demo flow)

This is the demo the capability is built around.

1. **Provision source.**
   `POST /instances { "type_id": "salesforce", "name": "sf-prod" }`
   Returns `oauth_url`. Agent opens it for the user. User approves. Callback completes.
2. **Verify.**
   `POST /instances/sf-prod/check` → `{ status: ok }`.
3. **Discover.**
   `POST /instances/sf-prod/discover` → 73 streams. Agent picks `Lead`, `Contact`, `Account`, `Opportunity`.
4. **Create inbound connection.**
   `POST /connections` with source=sf-prod, dest=reactor-data, schedule `*/15 * * * *`.
5. **Sandbox.**
   `POST /connections/sf-leads/sandbox { limit_rows: 100 }` → diff shown. Agent presents to user.
6. **Promote.**
   `POST /connections/sf-leads/promote { promote_token }`. Schedule kicks in via reactor-jobs.
7. **Add outbound connection (reverse).**
   `POST /connections` with source=reactor-data, dest=sf-prod.
   Reactor sees both directions exist on overlapping streams → prompts for conflict policy.
8. **Conflict policy.**
   `POST /policies/conflict { rules: [{ stream: "Lead", policy: "last_write_wins" }] }`.
9. **Both directions run.** Agent monitors via `GET /connections/.../runs`.
10. **Cutover.** After bake period:
    `PATCH /connections/sf-leads { enabled: false }`
    `PATCH /connections/pg-to-sf { enabled: false }`
    Reactor is now system of record.

No other BaaS can tell this story. No standalone Airbyte deployment can tell this story (no outbound + no sandbox + no agent surface).

---

## 11. Schema (Postgres metadata)

`_reactor_connect.*`:

| Table | Purpose |
|---|---|
| `instances` | configured connector + credential pointer (vault path) |
| `connections` | stream bindings (source instance, dest instance/table, schedule, options) |
| `receivers` | webhook ingress tokens + dispatch config |
| `discovered_catalogs` | cached `discover` results (ttl 1h) |
| `connection_state` | latest `AirbyteStateMessage` per (connection, stream) |
| `sync_runs` | run history (run_id, jobs run_id, started_at, finished_at, status, rows, error) |
| `action_invocations` | lightweight log of action calls |
| `conflict_policies` | parsed `ConflictPolicy` per binding pair |
| `sandbox_schemas` | ephemeral schema names + ttl |
| `audit_events` | admin event log |

No user data lives here. Streams write to `reactor-data` / `reactor-storage`.

---

## 12. SDK surface (TypeScript)

`@reactor/connect-sdk` mirrors the HTTP surface but with typed action wrappers, codegen'd per instance:

```typescript
import { ReactorConnect } from "@reactor/connect-sdk";

const connect = new ReactorConnect({ baseUrl, token });

// Direct action call
const lead = await connect.instance("sf-prod").action("createLead").invoke({
  FirstName: "Ada", LastName: "Lovelace", Company: "Reactor"
}, { dryRun: false, idempotencyKey: "lead-ada-2026" });

// Stream sync trigger
const run = await connect.connection("sf-leads-to-pg").run({ wait: true });

// Sandbox before promoting
const sandbox = await connect.connection("sf-leads-to-pg").sandbox({ limitRows: 100 });
if (sandbox.errors.length === 0) {
  await connect.connection("sf-leads-to-pg").promote(sandbox.promoteToken);
}
```

Codegen from the descriptor (`reactor connect codegen --instance sf-prod`) produces a strongly-typed client where `connect.instance("sf-prod").actions.createLead({...})` has compile-time validated inputs.

---

## 13. CLI surface

```
reactor connect catalog
reactor connect catalog show <type_id>

reactor connect instances list
reactor connect instances create <name> --type <type_id>
reactor connect instances credentials <name>           # interactive flow (OAuth opens browser)
reactor connect instances check <name>
reactor connect instances discover <name>

reactor connect connections list
reactor connect connections create <name> --from spec.toml
reactor connect connections sandbox <name>
reactor connect connections promote <name> --token <token>
reactor connect connections enable <name>
reactor connect connections disable <name>
reactor connect connections runs <name>

reactor connect actions invoke <instance>/<action> --input @input.json --dry-run
reactor connect receivers list <instance>
reactor connect receivers create <instance> --webhook <name> --dispatch <target>

reactor connect codegen --instance <name> --out src/connect/<name>.ts
```

---

## 14. Phasing

| Phase | Scope | Delivers |
|---|---|---|
| **v0.1 (M1)** | Native runtime + 4 native connectors (Stripe, Slack, Linear, GitHub) | Actions + Webhooks end-to-end. Stream is read-only and ships only for GitHub (issues). Sandbox for Actions only. |
| **v0.2 (M2)** | Manifest runtime + 30+ Airbyte YAML connectors | Streams (inbound) for all manifest connectors. Stream sandbox with ephemeral schema + diff. |
| **v0.3 (M3)** | Salesforce native + bidirectional + conflict policy DSL | Outbound streams, reverse sync, the CRM migration demo lands end-to-end. |
| **v0.4 (M4)** | AirbyteContainer runtime on G3 | Long-tail catalog access via Docker/Firecracker on jobs. G1/G2 unaffected. |
| **v0.5+** | Postgres-CDC native, S3 native, custom Rust plugin SDK, GraphQL connector type | |

Realtime stays parked. This capability is more pressing because every CRM-class app needs it on day one; realtime is a v1 enhancement on existing data flows.

---

## 15. Open questions

1. **Vault path layout.** Per-instance path is `secret/data/connect/{org_id}/instances/{instance_id}` — does that compose with the existing `reactor-vault` org-scoping? Check before migration 001.
2. **Receiver token rotation.** v0 tokens are stable; if leaked the receiver must be deleted and recreated. Rotation API (`POST /receivers/{id}/rotate`) — v0 or v0.2?
3. **Schema drift defaults.** `options.schema_drift` enum: `alert | auto_add | reject | block_until_approved`. Default should be `block_until_approved` for safety, but that breaks "set it and forget it" for casual users. Decide before v0.2.
4. **Outbound rate limiting.** Action invocation needs vendor-aware rate limiting (Salesforce daily API limits, Slack tier 3, etc.). Connector descriptor should declare rate limits; the invoke pipeline enforces. Spec the descriptor field shape — v0 or v0.2?
5. **OAuth client provisioning.** v0 = users supply their own client id/secret per provider. Is there a future "Reactor-hosted OAuth proxy" for users who don't want to register a dev app? Probably not v0 but worth a placeholder in `AuthDescriptor::OAuth2 { reactor_proxy: bool }`.
