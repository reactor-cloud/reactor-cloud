# `reactor-functions` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Fourth crate of the Reactor.cloud BaaS. Owns the Functions capability per `docs/ReactorCloud_spec.md` §2/§3/§6.1/§10-F.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, bundle format, runtime trait, schema, policy integration — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Expose a **bounded, sandboxed HTTP-handler primitive**: a function is `Request → Response` (Web Standard APIs), nothing more. Streaming responses are first-class.
2. Be **runtime-portable** behind a single `FunctionRuntime` trait. Three adapters ship at v0 — `wasm` (in-process), `bun` (subprocess + warm pool), `lambda` (remote) — exercising in-process / subprocess / remote topologies so the trait is genuinely tested.
3. Treat **bundles as durable artifacts**: every deployment is a versioned bundle stored in `reactor-storage`'s `_reactor_functions` system bucket. This is the first internal consumer of `reactor-storage` and validates that contract.
4. **Reuse the shared `reactor-policy` engine** for invoke-time authorization, with a small set of `function.*` / `request.*` builtins.
5. Be the **third real consumer of `reactor_core::auth::AuthClient`** — exercises both `InProcessAuthClient` and `RemoteAuthClient` topologies alongside reactor-data and reactor-storage.
6. Be the only crate allowed to touch the `_reactor_functions.*` Postgres metadata schema.

## 2. Non-goals (v0)

- **Durable execution / multi-step workflows** — that is `reactor-jobs` (next crate). A function that needs to survive a crash mid-execution is the wrong abstraction.
- **Built-in retries / dead-letter handling** — caller's responsibility, or use Jobs. Building retries into a request/response primitive makes side-effect counting unanswerable.
- **`waitUntil` / "background after response"** — Vercel's Fluid Compute feature; conflates function lifecycle with job lifecycle. Trigger a Job from a function instead.
- **Cron / scheduled invocation** — function metadata should not carry a scheduler. Use `reactor-jobs`.
- **WebSocket handlers** — different protocol semantics; deferred to a future `reactor-realtime` capability.
- **Stateful between invocations** (in-memory cache, file writes that persist) — adapter-defined and not portable. Use `reactor-storage` / `reactor-data`.
- **Function-to-function service mesh** — just call HTTP.
- **Multi-region / edge runtime** — v0.2+.
- **VPC peering / private networking** — G3b concern, deferred.
- **Provisioned concurrency for `wasm` / `bun`** — only Lambda needs the concept. Wasm is always warm; Bun has a fixed-size warm pool.
- **Container-image bundles** (Lambda's >50MB tier) — bundle size capped at 50 MB at v0 to skip container image plumbing.
- **Custom domains** — Sites' problem.
- **Additional language runtimes** beyond Rust→WASM and TypeScript-via-Bun (Python, Go, Node-via-Lambda) — v0.2+; the Lambda Web Adapter pattern makes this mechanical.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, IDs, AuthClient trait
├── reactor-policy/                # (existing) shared policy engine
├── reactor-auth/                  # (existing)
├── reactor-data/                  # (existing)
├── reactor-storage/               # (existing) — bundles live here
│
├── reactor-functions/             # the functions library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_functions.*
│   │   ├── 001_metadata.sql
│   │   ├── 002_deployments.sql
│   │   ├── 003_policies.sql
│   │   ├── 004_invocations.sql
│   │   └── 005_audit.sql
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # FunctionsConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # FunctionsState, FunctionCtx
│       ├── error.rs               # FunctionsError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs
│       │   ├── invoke.rs          # the hot path: /fn/v1/{name}/{*sub}
│       │   ├── admin.rs           # CRUD on functions + deployments
│       │   ├── deployments.rs     # version listing, rollback
│       │   └── logs.rs            # SSE log tail
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth.rs            # bearer + X-Reactor-Org → FunctionCtx
│       │
│       ├── service/
│       │   ├── mod.rs
│       │   ├── functions.rs       # function CRUD logic
│       │   ├── deployments.rs     # deploy / rollback orchestration
│       │   ├── invoke.rs          # invocation pipeline
│       │   ├── secrets.rs         # per-function env / secret resolution
│       │   └── policy.rs          # invoke policy enforcement
│       │
│       ├── bundle/
│       │   ├── mod.rs             # Bundle, Manifest types
│       │   ├── manifest.rs        # manifest.json schema + validation
│       │   ├── package.rs         # build / verify / unpack
│       │   ├── upload.rs          # push to reactor-storage system bucket
│       │   └── fetch.rs           # pull from reactor-storage on cold start
│       │
│       ├── runtime/
│       │   ├── mod.rs             # FunctionRuntime trait, IncomingRequest, OutgoingResponse
│       │   ├── wasm.rs            # WasmRuntime (wasmtime)
│       │   ├── bun.rs             # BunRuntime (subprocess + warm pool)
│       │   ├── lambda.rs          # LambdaRuntime (aws-sdk-lambda + Function URLs)
│       │   └── pool.rs            # warm-pool primitive used by Bun
│       │
│       ├── store/
│       │   ├── mod.rs             # FunctionsStore trait
│       │   └── postgres.rs        # PgFunctionsStore
│       │
│       ├── invoke/
│       │   ├── pipeline.rs        # auth → policy → runtime → response
│       │   ├── streaming.rs       # SSE / chunked transfer plumbing
│       │   ├── timeout.rs         # per-invocation deadline + cancel
│       │   └── metrics.rs         # invocation counters / latency histograms
│       │
│       └── audit.rs               # admin-event audit writer
│
└── reactor-functions-server/      # standalone bin
    ├── Cargo.toml
    └── src/
        ├── main.rs                # axum bind + tracing + migrate + serve
        └── cli/
            ├── mod.rs
            └── doctor.rs          # connectivity diagnostics (DB, auth, storage, runtimes)
```

Conventions:
- `reactor-functions` depends on `reactor-core` (for `ReactorId`, `AuthClient`, `AuthCtx`) and `reactor-policy` (for the shared policy engine). It depends on `reactor-storage` (as a library) only for the bundle store typed client; **never** on `reactor-auth`.
- All three runtime adapters (`wasm`, `bun`, `lambda`) ship at v0, gated behind Cargo features (`runtime-wasm`, `runtime-bun`, `runtime-lambda`) so a Tauri build can drop `lambda` and trim the AWS SDK.
- The Postgres adapter lives inside `reactor-functions/src/store/postgres.rs`; if SQLite arrives, the same split as reactor-data applies (extract to `reactor-functions-postgres` / `reactor-functions-sqlite`).

---

## 4. Core types

### 4.1 ID & types reuse

All IDs are `ReactorId` (UUIDv7) from `reactor-core`. Functions-specific types:

| Type | Rust | Notes |
|---|---|---|
| `FunctionId` | `ReactorId` | Primary key for functions |
| `DeploymentId` | `ReactorId` | Primary key for deployments (bundle + manifest pair) |
| `InvocationId` | `ReactorId` | One per invocation; correlates logs / metrics |
| `RuntimeKind` | `enum { Wasm, Bun, Lambda }` | Selected at deploy time, immutable per deployment |
| `BundleSha256` | `[u8; 32]` | Content hash; verified on fetch |

### 4.2 `FunctionCtx` (request-local)

Constructed by middleware once per request from `AuthCtx`:

```rust
// reactor-functions/src/state.rs
#[derive(Debug, Clone)]
pub struct FunctionCtx {
    pub auth:       AuthCtx,           // required for all routes (no anon invoke at v0)
    pub request_id: String,
    pub org_id:     OrgId,             // resolved active org (deny if None)
}

impl FunctionCtx {
    pub fn user_id(&self) -> &UserId { /* ... */ }
    pub fn active_org(&self) -> &OrgId { &self.org_id }
    pub fn has_permission(&self, perm: &str) -> bool {
        self.auth.has_permission(perm)
    }
}
```

There is **no anonymous invoke** at v0. Public functions are achievable by issuing a short-lived token via reactor-auth and embedding it in the caller; this matches reactor-data's posture and avoids a `service_role`-shaped escape hatch.

### 4.3 `FunctionRuntime` trait

The single load-bearing abstraction. Every adapter implements it; nothing else is allowed to touch a runtime directly.

```rust
// reactor-functions/src/runtime/mod.rs
#[async_trait]
pub trait FunctionRuntime: Send + Sync + 'static {
    fn kind(&self) -> RuntimeKind;

    /// Materialise a deployment so it is ready to invoke.
    /// Idempotent — calling deploy() twice with the same handle is a no-op.
    async fn deploy(
        &self,
        bundle: &BundleRef,             // SHA + storage key; runtime fetches lazily
        manifest: &Manifest,
    ) -> Result<DeploymentHandle, FunctionsError>;

    /// Invoke a deployed function. The body of the response is a stream;
    /// non-streaming functions return a one-chunk stream.
    async fn invoke(
        &self,
        handle: &DeploymentHandle,
        req:    IncomingRequest,
        limits: &Limits,
    ) -> Result<OutgoingResponse, FunctionsError>;

    /// Pre-warm a deployment (no-op for adapters without a warm concept).
    async fn warm(&self, handle: &DeploymentHandle) -> Result<(), FunctionsError>;

    /// Tear down a deployment and release adapter resources.
    /// Used on rollback, delete, and reconciliation drift.
    async fn destroy(&self, handle: &DeploymentHandle) -> Result<(), FunctionsError>;

    /// List handles the runtime currently believes are active.
    /// Used by the reconciler to detect drift between metadata and runtime state.
    async fn list_active(&self) -> Result<Vec<DeploymentHandle>, FunctionsError>;
}

#[derive(Debug)]
pub struct IncomingRequest {
    pub method:     http::Method,
    pub sub_path:   String,             // path after /fn/v1/{name}
    pub query:      String,             // raw querystring (no parsing inside the runtime)
    pub headers:    http::HeaderMap,    // forwarded subset (denylist: Authorization, Cookie unless allow_credentials)
    pub body:       BoxStream<'static, Result<bytes::Bytes, std::io::Error>>,
    pub ctx:        InvokeCtx,          // request_id, function_name, deployment_version, user_id, org_id
}

#[derive(Debug)]
pub struct OutgoingResponse {
    pub status:     http::StatusCode,
    pub headers:    http::HeaderMap,
    pub body:       BoxStream<'static, Result<bytes::Bytes, std::io::Error>>,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub timeout:        Duration,
    pub memory_mb:      u32,
    pub max_body_in:    u64,
    pub max_body_out:   u64,
}

#[derive(Debug, Clone)]
pub struct DeploymentHandle {
    pub deployment_id: DeploymentId,
    pub runtime_ref:   String,           // adapter-specific (wasm: module hash; bun: pid; lambda: ARN)
}
```

**Streaming is not optional.** Every adapter returns a stream. A non-streaming function is wrapped as a one-chunk stream by the adapter. This is the v0 contract; bolting it on later means rewriting `invoke` and every adapter.

### 4.4 `FunctionsStore` trait

```rust
// reactor-functions/src/store/mod.rs
#[async_trait]
pub trait FunctionsStore: Send + Sync + 'static {
    type Tx<'a>: FunctionsTx where Self: 'a;

    async fn begin(&self) -> Result<Self::Tx<'_>, FunctionsError>;

    // Function CRUD
    async fn create_function(&self, f: &NewFunction) -> Result<Function, FunctionsError>;
    async fn get_function(&self, org: &OrgId, name: &str) -> Result<Option<Function>, FunctionsError>;
    async fn list_functions(&self, org: &OrgId) -> Result<Vec<Function>, FunctionsError>;
    async fn delete_function(&self, id: &FunctionId) -> Result<(), FunctionsError>;

    // Deployments
    async fn create_deployment(&self, d: &NewDeployment) -> Result<Deployment, FunctionsError>;
    async fn get_deployment(&self, id: &DeploymentId) -> Result<Option<Deployment>, FunctionsError>;
    async fn current_deployment(&self, fn_id: &FunctionId) -> Result<Option<Deployment>, FunctionsError>;
    async fn promote_deployment(&self, id: &DeploymentId) -> Result<(), FunctionsError>;
    async fn list_deployments(&self, fn_id: &FunctionId, limit: u32) -> Result<Vec<Deployment>, FunctionsError>;

    // Per-function env / secrets
    async fn upsert_env(&self, fn_id: &FunctionId, key: &str, value: &SecretValue) -> Result<(), FunctionsError>;
    async fn get_env(&self, fn_id: &FunctionId) -> Result<Vec<EnvEntry>, FunctionsError>;
    async fn delete_env(&self, fn_id: &FunctionId, key: &str) -> Result<(), FunctionsError>;

    // Policies
    async fn get_invoke_policies(&self, fn_id: &FunctionId) -> Result<Vec<InvokePolicy>, FunctionsError>;
    async fn upsert_policy(&self, p: &NewInvokePolicy) -> Result<InvokePolicy, FunctionsError>;

    // Invocations (lightweight; not full audit)
    async fn record_invocation(&self, inv: &InvocationRecord) -> Result<(), FunctionsError>;
    async fn list_invocations(&self, fn_id: &FunctionId, limit: u32) -> Result<Vec<InvocationRecord>, FunctionsError>;

    // Audit (admin events only)
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), FunctionsError>;
}
```

### 4.5 No `FunctionsClient` trait

Same posture as reactor-data: there is **no `FunctionsClient` trait** in `reactor-core`. Other capabilities (Sites, Jobs) consume reactor-functions via its HTTP surface, the same way external callers do. If a unified Reactor binary needs in-process invocation for performance, `reactor_functions::router(state)` is embeddable behind `tower::Service`.

---

## 5. HTTP surface (v0)

### 5.1 Health

```
GET    /fn/v1/health
       → 200 { "status": "ok", "version": "0.1.0", "runtimes": ["wasm", "bun", "lambda"] }
```

### 5.2 Invoke (the hot path)

```
*      /fn/v1/{name}                 -- root invocation (any method)
*      /fn/v1/{name}/{*sub}          -- with sub-path

       Body:    raw bytes (any content type)
       Headers: Content-Type, Content-Length, custom headers (forwarded to function)
       Auth:    Bearer JWT required; X-Reactor-Org optional

       → 200/2xx (function response, body streamed)
       → 4xx     (function returned an error response)
       → 408     function_timeout
       → 413     payload_too_large
       → 429     too_many_requests (concurrency cap hit)
       → 500     function_crashed
       → 502     runtime_error (adapter failure, not a function failure)
       → 503     deployment_not_ready (cold start in progress, retry-after set)

       Requires: functions:{name}:invoke + invoke-policy evaluation
```

The platform's response is the function's response, modulo:
- `X-Request-Id` header injected
- `Server: reactor-functions/0.1` header injected
- `Authorization` header is **not** forwarded by default (denylist); the function receives a synthesized `X-Reactor-Auth` header with the resolved user/org instead. Opt-in via manifest `forward_authorization = true`.

### 5.3 Admin: functions

```
POST   /fn/v1/_admin/functions
       Body: { "name": "checkout", "runtime": "bun", "description": "..." }
       → 201 { function }
       Requires: functions:create

GET    /fn/v1/_admin/functions
       → 200 [ function, ... ]
       Lists functions in active org

GET    /fn/v1/_admin/functions/{name}
       → 200 { function, current_deployment }

DELETE /fn/v1/_admin/functions/{name}
       → 204
       Requires: functions:{name}:admin
       Tears down all deployments via FunctionRuntime::destroy
```

Function-name constraints: `^[a-z][a-z0-9-]{0,62}$` (lowercase, hyphens, 1–63 chars). Runtime is immutable per function.

### 5.4 Admin: deployments

```
POST   /fn/v1/_admin/functions/{name}/deployments
       Body (multipart/form-data):
         - manifest: application/json
         - bundle:   application/zip (≤ 50 MiB)
       → 201 { deployment }
       Requires: functions:{name}:deploy
       Side-effects:
         1. Validate manifest schema
         2. Verify bundle SHA256
         3. Upload bundle to reactor-storage (_reactor_functions bucket)
         4. Insert deployment row (status=pending)
         5. Call runtime.deploy() — blocking up to 30s
         6. On success: status=ready; do NOT promote yet
         7. On failure: status=failed; bundle stays in storage for diagnosis

POST   /fn/v1/_admin/functions/{name}/promote
       Body: { "deployment_id": "..." }
       → 200 { function }
       Requires: functions:{name}:deploy
       Atomic swap of current_deployment_id; new invocations route to new deployment

POST   /fn/v1/_admin/functions/{name}/rollback
       Body: { "to_deployment_id": "..." } (optional; defaults to previous)
       → 200 { function }
       Requires: functions:{name}:admin

GET    /fn/v1/_admin/functions/{name}/deployments
       Query: ?limit=20
       → 200 [ deployment, ... ]
       Most recent first
```

Deploy and promote are **separate** steps. Deploy materialises the bundle and runs the runtime cold-start cycle; promote swaps live traffic. This is the Vercel/Fly model and keeps "build broken on prod" failures impossible.

### 5.5 Admin: env / secrets

```
PUT    /fn/v1/_admin/functions/{name}/env/{key}
       Body: { "value": "...", "secret": true }
       → 204
       If secret=true, value is encrypted at rest with REACTOR_FUNCTIONS_DATA_KEY (column-encrypted)

GET    /fn/v1/_admin/functions/{name}/env
       → 200 [ { key, secret, last_updated_at }, ... ]
       Secret values are never returned via API; only metadata.

DELETE /fn/v1/_admin/functions/{name}/env/{key}
       → 204
```

Env changes apply to **future invocations**. Existing warm instances are torn down on next idle window.

### 5.6 Admin: logs

```
GET    /fn/v1/_admin/functions/{name}/logs
       Query: ?since=2026-05-14T00:00:00Z&limit=200&follow=1
       → 200 (text/event-stream when follow=1, otherwise application/json)

       SSE event shape:
         event: log
         data: { "ts": "...", "level": "info", "deployment_id": "...", "request_id": "...", "message": "..." }

       Requires: functions:{name}:logs
```

Log source per adapter:
- `wasm`: stdout/stderr captured by the host; piped through tracing
- `bun`: subprocess stdout/stderr; piped through tracing
- `lambda`: CloudWatch Logs subscription filter → forwarded to local buffer

### 5.7 Headers

| Header | Direction | Meaning |
|---|---|---|
| `Authorization: Bearer <jwt>` | inbound | Required on every route; admin and invoke alike |
| `X-Reactor-Org: <ref>` | inbound | Active org override; UUID or slug |
| `X-Request-Id` | both | Generated if absent; echoed in response and forwarded to function |
| `X-Reactor-Auth` | inbound→fn | Synthesized header passed to function with `{user_id, org_id, role}` JSON |
| `X-Reactor-Function` | response | `{name}@{version}` for observability |
| `X-Reactor-Cold-Start` | response | `1` if this invocation paid the cold-start penalty |
| `X-Reactor-Duration-Ms` | response | Total invoke duration including cold start |

### 5.8 Error envelope

Same shape as reactor-auth/data/storage:

```json
{
  "error": {
    "code": "function_timeout",
    "message": "Function 'checkout' exceeded 30000ms timeout.",
    "status": 408,
    "request_id": "req_01HZ...",
    "details": {
      "function": "checkout",
      "deployment_id": "dep_01HZ...",
      "duration_ms": 30001
    }
  }
}
```

Error codes (snake_case): `function_not_found`, `deployment_not_ready`, `function_timeout`, `function_crashed`, `payload_too_large`, `response_too_large`, `too_many_requests`, `runtime_error`, `bundle_invalid`, `manifest_invalid`, `bundle_too_large`, `policy_denied`, `unsupported_runtime`, `env_key_invalid`, `cold_start_failed`.

---

## 6. Database schema (`_reactor_functions`)

```sql
create schema if not exists _reactor_functions;

-- 6.1 Functions
create table _reactor_functions.functions (
  id                       uuid primary key,                   -- ReactorId
  org_id                   uuid not null,                      -- FK to reactor_auth.orgs conceptually
  name                     citext not null,
  description              text,
  runtime                  text not null,                      -- 'wasm' | 'bun' | 'lambda'
  current_deployment_id    uuid,                               -- FK; null until first promote
  created_at               timestamptz not null default now(),
  updated_at               timestamptz not null default now(),
  unique (org_id, name)
);
create index on _reactor_functions.functions (org_id);

-- 6.2 Deployments (one row per (function, version))
create table _reactor_functions.deployments (
  id                       uuid primary key,
  function_id              uuid not null references _reactor_functions.functions(id) on delete cascade,
  version                  bigint not null,                    -- monotonic per function
  bundle_bucket            text not null,                      -- always '_reactor_functions'
  bundle_object_key        text not null,                      -- "{function_name}/{version}.zip"
  bundle_sha256            bytea not null,
  bundle_size              bigint not null,
  manifest_json            jsonb not null,                     -- full validated manifest
  status                   text not null,                      -- 'pending' | 'ready' | 'failed' | 'destroyed'
  status_detail            text,                               -- error message on failed
  runtime_ref              text,                               -- adapter-specific (Lambda ARN, etc.)
  deployed_at              timestamptz not null default now(),
  deployed_by_user_id      uuid,
  unique (function_id, version)
);
create index on _reactor_functions.deployments (function_id, deployed_at desc);
create index on _reactor_functions.deployments (status) where status in ('pending', 'failed');

alter table _reactor_functions.functions
  add constraint fk_current_deployment
  foreign key (current_deployment_id) references _reactor_functions.deployments(id) on delete set null;

-- 6.3 Per-function env / secrets
create table _reactor_functions.env (
  function_id              uuid not null references _reactor_functions.functions(id) on delete cascade,
  key                      text not null,
  value_plaintext          text,                               -- non-secret values
  value_encrypted          bytea,                              -- pgcrypto-encrypted with REACTOR_FUNCTIONS_DATA_KEY
  is_secret                boolean not null default false,
  last_updated_at          timestamptz not null default now(),
  primary key (function_id, key),
  check ((is_secret and value_encrypted is not null and value_plaintext is null)
      or (not is_secret and value_plaintext is not null and value_encrypted is null))
);

-- 6.4 Invoke policies
create table _reactor_functions.policies (
  id                       uuid primary key,
  function_id              uuid not null references _reactor_functions.functions(id) on delete cascade,
  name                     text not null,
  using_expr_json          jsonb,                              -- PolicyExpr; evaluated for invoke
  raw_text                 text not null,
  sha256                   bytea not null,
  created_at               timestamptz not null default now(),
  unique (function_id, name)
);
create index on _reactor_functions.policies (function_id);

-- 6.5 Invocations (lightweight log; not full audit)
create table _reactor_functions.invocations (
  id                       uuid primary key,                   -- InvocationId
  deployment_id            uuid not null references _reactor_functions.deployments(id) on delete cascade,
  function_id              uuid not null,                      -- denormalised for query speed
  org_id                   uuid not null,
  actor_user_id            uuid,
  actor_apikey_id          uuid,
  request_id               text not null,
  method                   text not null,
  sub_path                 text not null,
  status_code              integer not null,
  duration_ms              integer not null,
  cold_start               boolean not null default false,
  bytes_in                 bigint not null default 0,
  bytes_out                bigint not null default 0,
  error_code               text,                               -- platform error code if any
  started_at               timestamptz not null default now()
);
create index on _reactor_functions.invocations (function_id, started_at desc);
create index on _reactor_functions.invocations (org_id, started_at desc);
create index on _reactor_functions.invocations (deployment_id, started_at desc);
create index on _reactor_functions.invocations (status_code, started_at desc) where status_code >= 500;

-- 6.6 Audit (admin events only — invocations live in §6.5)
create table _reactor_functions.audit_events (
  id                       uuid primary key,
  ts                       timestamptz not null default now(),
  actor_user_id            uuid,
  actor_apikey_id          uuid,
  org_id                   uuid,
  function_id              uuid,
  deployment_id            uuid,
  event_type               text not null,                      -- 'function.create', 'deployment.create', 'deployment.promote', etc.
  details                  jsonb not null default '{}'::jsonb,
  request_id               text not null
);
create index on _reactor_functions.audit_events (org_id, ts desc);
create index on _reactor_functions.audit_events (function_id, ts desc);
```

### 6.7 Role grants

`_reactor_functions` is **not** readable by user application roles. reactor-functions-server connects with a dedicated role that has:
- `USAGE` on `_reactor_functions` schema
- Full DML on all tables in `_reactor_functions`
- No access to user data schemas

---

## 7. Bundle format

A bundle is a single zip file with a strict layout. Maximum size: 50 MiB at v0 (Lambda's direct-upload ceiling).

```
my-function.zip
├── manifest.json                     # required, schema below
└── code/                             # required, runtime-specific contents
    ├── (wasm)    main.wasm           # one .wasm module, exports `_start` or HTTP handler
    ├── (bun)     index.{ts,js}       # Bun entrypoint, default export is fetch handler
    └── (lambda)  bootstrap           # Lambda Web Adapter expects HTTP server on $PORT
                  index.{ts,js}       # plus the actual handler
```

### 7.1 Manifest schema

```json
{
  "name": "checkout",
  "version": 7,
  "runtime": "bun",
  "entrypoint": "code/index.ts",

  "limits": {
    "timeout_ms":      30000,
    "memory_mb":       256,
    "max_body_in_mb":  5,
    "max_body_out_mb": 6
  },

  "concurrency": {
    "min_instances":   0,
    "max_concurrency": 50
  },

  "env_keys": ["STRIPE_PUBLIC_KEY"],
  "secret_keys": ["STRIPE_SECRET_KEY", "WEBHOOK_SIGNING_SECRET"],

  "forward_authorization": false,

  "bundle_sha256": "hex-encoded-sha256-of-the-zip-body"
}
```

Validation rules:
- `version` is server-assigned on deploy (the `manifest.json` value is recomputed; client value is ignored).
- `runtime` must match the function's immutable runtime.
- `timeout_ms` is capped per adapter: `wasm` 300_000, `bun` 300_000, `lambda` 900_000.
- `memory_mb` ranges per adapter: `wasm` 32–1024, `bun` 64–2048, `lambda` 128–10_240 (Lambda's range).
- `env_keys` / `secret_keys` must exist in `_reactor_functions.env` at deploy time, else `400 manifest_invalid`.
- `bundle_sha256` is recomputed by the server and rejected on mismatch with `400 bundle_invalid`.

### 7.2 Bundle storage

Bundles live in `reactor-storage`'s `_reactor_functions` system bucket:

```
_reactor_functions/
└── {function_name}/
    ├── 1.zip
    ├── 2.zip
    ├── 7.zip
    └── manifests/
        ├── 1.json
        ├── 2.json
        └── 7.json
```

The bucket is provisioned by `reactor-functions-server` on boot if missing. Access is via a dedicated reactor-storage API key with `storage:_reactor_functions:*` permission. **No user-facing route exposes this bucket**; reactor-storage's policy engine denies any tenant request that names a bucket starting with `_`.

### 7.3 Bundle lifecycle

| State | Created by | Cleared by |
|---|---|---|
| Uploaded | `POST /_admin/functions/{name}/deployments` | Never (audit trail) |
| Deployed | runtime.deploy() success | runtime.destroy() on rollback or function delete |
| Active | promote() | next promote() |
| Stale | Older than the last 10 deployments per function | Background sweeper (v0.2; manual `DELETE` at v0) |

---

## 8. Runtime adapters

### 8.1 `WasmRuntime` (in-process)

- **Engine**: `wasmtime` with the Component Model + WASI Preview 2
- **Function contract**: WASI HTTP (`wasi:http/incoming-handler`) — the same shape Wasmtime + Spin use
- **Deploy**: pre-compile module, cache by `bundle_sha256` in memory and on disk
- **Invoke**: instantiate a fresh Store per request (cheap), wire stdin/stdout to streaming body
- **Concurrency**: bounded by `max_concurrency`; tokio semaphore per deployment
- **Cold start target**: <100 ms (compile cached); <500 ms first time
- **Warm cost**: <10 ms (module already in memory)
- **Destroy**: drop cached module and its on-disk artifact

### 8.2 `BunRuntime` (subprocess + warm pool)

- **Process model**: long-lived `bun run` subprocess per deployment, listening on a Unix socket assigned by the host
- **Function contract**: ES module exporting `default { fetch(req: Request): Response | Promise<Response> }` (the Bun.serve / Cloudflare Workers / Deno shape)
- **Deploy**: write bundle to `{FUNCTIONS_WORKDIR}/{function}/{version}/`, spawn one subprocess to verify import, leave it running as the first warm instance
- **Invoke**: pick a free instance from the warm pool, forward HTTP over Unix socket, stream the response back
- **Warm pool**: configurable `min_instances` (default 0, max 8), LRU eviction after `BUN_IDLE_TTL` (default 300s), respawn on crash
- **Cold start target**: <300 ms (Bun startup + module import)
- **Warm cost**: <20 ms (Unix socket roundtrip)
- **Timeout enforcement**: SIGTERM at deadline, SIGKILL after 5s grace; the instance is then evicted
- **Destroy**: SIGTERM all instances, remove workdir

### 8.3 `LambdaRuntime` (remote, AWS)

- **Mechanism**: AWS Lambda + Function URL (no API Gateway) + Lambda Web Adapter (LWA) layer
- **Bundle ingest**: same Bun bundle as §8.2; we add the LWA layer (~2 MB) and a `bootstrap` script that runs `bun run code/index.ts` on `$PORT`. The function code does not need to know it is on Lambda.
- **Deploy**:
  1. Upload zip to the function's S3 bundle path (S3 referenced by Lambda)
  2. `aws-sdk-lambda` `CreateFunction` or `UpdateFunctionCode`
  3. `CreateFunctionUrlConfig` with `InvokeMode = RESPONSE_STREAM`
  4. Wait until Lambda reports `Active`
  5. Store the Function URL + ARN in `deployments.runtime_ref`
- **Invoke**: HTTP POST to the Function URL with streaming body; LWA translates to the in-container HTTP server; response is streamed back
- **Cold start**: 200–800 ms (AWS-controlled, can't be cheated)
- **Warm cost**: <50 ms (depends on Lambda's instance reuse)
- **Provisioned concurrency**: opt-in per function via `concurrency.min_instances > 0` → `PutProvisionedConcurrencyConfig`
- **Logs**: CloudWatch Logs subscription filter writes to a Kinesis stream that reactor-functions-server consumes; logs land in `/_admin/functions/{name}/logs`
- **Destroy**: `DeleteFunctionUrlConfig` then `DeleteFunction`; bundle stays in S3 for audit

### 8.4 Cold-start budgets (binding for §13 quality bars)

| Adapter | Cold (p50) | Cold (p95) | Warm (p50) | Notes |
|---|---|---|---|---|
| `wasm` | 50 ms | 100 ms | 5 ms | Module precompile cached |
| `bun` | 200 ms | 300 ms | 15 ms | Subprocess + import |
| `lambda` (no PC) | 300 ms | 800 ms | 30 ms | AWS-controlled; honest numbers |
| `lambda` (PC=1) | 30 ms | 50 ms | 30 ms | Costs money; opt-in |

The §13 spec target (<200 ms p95 cold start on G3a) is achievable on `lambda` only with provisioned concurrency or on `bun` warm-pooled. Document this honestly; do not promise across-the-board <200 ms.

### 8.5 Feature gates

```toml
[features]
default = ["runtime-wasm", "runtime-bun"]
runtime-wasm   = ["dep:wasmtime", "dep:wasmtime-wasi", "dep:wasmtime-wasi-http"]
runtime-bun    = []                                                    # uses subprocess + tokio
runtime-lambda = ["dep:aws-sdk-lambda", "dep:aws-sdk-s3", "dep:aws-config"]
```

`reactor-functions-server` enables all three by default. Tauri / G1 builds drop `runtime-lambda`. G3-only deployments drop `runtime-wasm` if module size matters.

### 8.6 Runtime selection at deploy

The function's `runtime` is fixed at function creation. There is no runtime switching across deployments — switching runtimes means deleting and re-creating the function. This keeps the deployment pipeline a single code path per function.

---

## 9. Invocation pipeline

### 9.1 Request flow

```
POST /fn/v1/checkout/api/process    Authorization: Bearer <jwt>
      ▼
auth_middleware
  - extract Bearer + X-Reactor-Org
  - AuthClient::resolve_ctx → AuthCtx
  - construct FunctionCtx
  ▼
route → service::invoke
  - resolve function by (org_id, "checkout") → Function
  - require permission "functions:checkout:invoke"
  - load current_deployment; deny with 503 if status != ready
  ▼
policy::compile_for_invoke
  - load policies for function_id
  - build RequestFacts (method, sub_path, headers subset)
  - evaluate against FunctionCtx
  - AlwaysDeny → 403 immediately; AlwaysAllow → proceed
  ▼
runtime registry → FunctionRuntime impl (matched by deployment.runtime)
  ▼
runtime.invoke(handle, IncomingRequest, Limits)
  - body streamed in (size capped at limits.max_body_in)
  - timeout enforced via tokio::time::timeout + cancel token
  - response body streamed out (size capped at limits.max_body_out)
  ▼
response shaping
  - inject X-Request-Id, X-Reactor-Function, X-Reactor-Cold-Start, X-Reactor-Duration-Ms
  - record InvocationRecord (in a non-blocking task; errors logged but never fail the request)
  - emit metrics
  ▼
client receives streamed response
```

### 9.2 Streaming contract

- The `body` field in both `IncomingRequest` and `OutgoingResponse` is `BoxStream<Result<Bytes, io::Error>>`.
- Adapters that don't stream natively (e.g., a `wasm` function returning a `Vec<u8>`) wrap it as a single-chunk stream.
- Backpressure: each adapter implements its own (Wasmtime: pull-based; Bun: socket-level; Lambda: HTTP/1.1 chunked from the Function URL).
- Cancellation: the host cancel token propagates to the adapter; on cancel, every adapter must release resources within 500 ms or it's logged as a leak.

### 9.3 Timeout enforcement

| Adapter | Mechanism |
|---|---|
| `wasm` | Wasmtime `epoch_interruption` + `epoch_deadline_async_yield_and_update`; cleanly interrupts at instruction boundary |
| `bun` | tokio timeout on the Unix-socket exchange; SIGTERM the process; SIGKILL after 5s grace |
| `lambda` | Set Lambda function `Timeout` to `manifest.limits.timeout_ms / 1000`; AWS enforces; we also enforce client-side as a cross-check |

On timeout the platform returns `408 function_timeout` with the deployment id and observed duration in `details`.

### 9.4 Concurrency control

Each deployment carries a tokio `Semaphore` sized to `manifest.concurrency.max_concurrency`. Acquire-with-timeout of 100 ms; on failure return `429 too_many_requests` with `Retry-After: 1`. Lambda's reserved concurrency is set from the same value when applicable.

### 9.5 Body size enforcement

- Inbound body is wrapped in a `Limited` stream that errors with `413 payload_too_large` when `max_body_in` is exceeded.
- Outbound body is wrapped similarly; if exceeded mid-stream, the connection is reset and a `response_too_large` error is recorded (the client will see a truncated response).

### 9.6 Cold-start handling

If `runtime.invoke` reports the deployment is not yet hot and `min_instances == 0`, a cold-start counter starts. Cold-starts are not retried by the platform. The response carries `X-Reactor-Cold-Start: 1` and the `cold_start` flag is set on the `InvocationRecord`.

---

## 10. Auth integration

### 10.1 Middleware

```rust
async fn auth_middleware<B>(
    State(state): State<FunctionsState>,
    headers: HeaderMap,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, FunctionsError> {
    let token = headers.get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(FunctionsError::Unauthorized)?;

    let requested_org: Option<OrgRef> = headers.get("x-reactor-org")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.parse().unwrap());

    let ctx = state.auth.resolve_ctx(token, requested_org.as_ref()).await?;
    let org_id = ctx.active_org.clone().ok_or(FunctionsError::OrgRequired)?;

    req.extensions_mut().insert(FunctionCtx { auth: ctx, request_id, org_id });
    Ok(next.run(req).await)
}
```

There is no anonymous middleware path. Every request — invoke or admin — bears a JWT.

### 10.2 Permission scheme

| Permission | Scope |
|---|---|
| `functions:create` | Create new functions in the org |
| `functions:{name}:invoke` | Invoke a specific function |
| `functions:{name}:deploy` | Push new deployments + promote |
| `functions:{name}:admin` | Rollback, delete, env management |
| `functions:{name}:logs` | Read logs |
| `functions:*:invoke` | Invoke any function |
| `functions:*:*` | Full functions access |

### 10.3 Storage API key for bundle bucket

`reactor-functions-server` holds a reactor-storage API key with `storage:_reactor_functions:*` scope, supplied via `REACTOR_FUNCTIONS_STORAGE_API_KEY`. This is the only path to the system bucket; user requests cannot reach `_`-prefixed buckets at all (enforced by reactor-storage policy).

### 10.4 Topology wiring

```rust
let auth: Arc<dyn AuthClient> = match config.deployment {
    Deployment::Monolith => {
        let auth_service = reactor_auth::AuthService::new(auth_pool, auth_config).await?;
        Arc::new(reactor_auth::client::InProcessAuthClient::new(Arc::new(auth_service)))
    }
    Deployment::Microservices => Arc::new(
        reactor_auth::client::RemoteAuthClient::builder()
            .base_url(config.auth_url.clone())
            .internal_secret(config.internal_secret.clone())
            .build()?
    ),
};

let storage = StorageClient::new(config.storage_url.clone(), config.storage_api_key.clone());
let runtimes = build_runtime_registry(&config)?;     // wires wasm / bun / lambda based on features
let store = PgFunctionsStore::new(pool);
let policy = PolicyEngine::with_builtins(functions_builtins());

let state = FunctionsState::new(store, storage, runtimes, auth, policy, config);
let app = reactor_functions::router(state);
```

---

## 11. Policy engine integration

### 11.1 Builtins

In addition to `auth.*`, invoke policies can reference:

| Builtin | Type | Description |
|---|---|---|
| `function.name` | `text` | The function's name |
| `function.runtime` | `text` | `'wasm' \| 'bun' \| 'lambda'` |
| `deployment.version` | `bigint` | Active deployment version |
| `request.method` | `text` | HTTP method |
| `request.sub_path` | `text` | Path after `/fn/v1/{name}` |
| `request.header(name)` | `text` | Forwarded request header value (deny-listed headers return null) |

### 11.2 Example policies

```sql
-- Only allow invocation during business hours UTC
policy biz_hours on function "checkout"
  using (extract(hour from now()) between 9 and 17);

-- Require an explicit X-Confirm header for destructive operations
policy delete_confirm on function "delete-account"
  using (request.method != 'DELETE' or request.header('x-confirm') = 'yes');

-- Tenant isolation through sub-path convention
policy tenant_scoped on function "tenant-api"
  using (request.sub_path like '/' || auth.org_id()::text || '/%');
```

Policies are advisory in addition to permission gates; both must pass.

---

## 12. Configuration

`reactor-functions-server` reads from env (12-factor).

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_FUNCTIONS_DATABASE_URL` | yes | — | Postgres connection string |
| `REACTOR_FUNCTIONS_BIND` | no | `0.0.0.0:8004` | HTTP bind address |
| `REACTOR_FUNCTIONS_DATA_KEY` | yes | — | Base64 32-byte key for env-secret column encryption |
| `REACTOR_FUNCTIONS_WORKDIR` | no | `/var/lib/reactor-functions` | Local workspace for bundles + bun subprocesses |
| `REACTOR_FUNCTIONS_STORAGE_URL` | yes | — | URL of reactor-storage-server |
| `REACTOR_FUNCTIONS_STORAGE_API_KEY` | yes | — | Storage API key with `storage:_reactor_functions:*` |
| `REACTOR_FUNCTIONS_BUNDLE_MAX_BYTES` | no | `52428800` | 50 MiB |
| `REACTOR_FUNCTIONS_INVOKE_DEFAULT_TIMEOUT_MS` | no | `30000` | Used when manifest omits |
| `REACTOR_FUNCTIONS_INVOKE_MAX_TIMEOUT_MS` | no | `300000` | Server-level cap (Lambda has its own 900s) |
| `REACTOR_FUNCTIONS_BUN_BIN` | no | `bun` | Path to `bun` binary |
| `REACTOR_FUNCTIONS_BUN_IDLE_TTL_SECS` | no | `300` | LRU eviction window for warm Bun instances |
| `REACTOR_FUNCTIONS_BUN_MAX_INSTANCES_PER_FN` | no | `8` | Hard cap on warm pool size |
| `REACTOR_FUNCTIONS_LAMBDA_REGION` | no | `us-east-1` | AWS region |
| `REACTOR_FUNCTIONS_LAMBDA_ROLE_ARN` | yes (lambda) | — | Execution role for created Lambda functions |
| `REACTOR_FUNCTIONS_LAMBDA_BUNDLE_S3_BUCKET` | yes (lambda) | — | S3 bucket Lambda reads bundles from |
| `REACTOR_FUNCTIONS_LAMBDA_LWA_LAYER_ARN` | yes (lambda) | — | Lambda Web Adapter layer ARN |
| `REACTOR_FUNCTIONS_LAMBDA_LOG_GROUP_PREFIX` | no | `/reactor/functions/` | CloudWatch log group prefix |
| `REACTOR_FUNCTIONS_DEPLOYMENT` | no | `monolith` | `monolith` or `microservices` |
| `REACTOR_FUNCTIONS_AUTH_URL` | yes (microservices) | — | URL of reactor-auth-server |
| `REACTOR_FUNCTIONS_INTERNAL_SECRET` | yes (microservices) | — | Shared secret for internal endpoints |
| `REACTOR_FUNCTIONS_AUTH_DATABASE_URL` | yes (monolith) | — | Postgres URL for auth schema |
| `REACTOR_FUNCTIONS_AUTH_DATA_KEY` | yes (monolith) | — | Forwarded to reactor-auth column-encryption key |
| `REACTOR_FUNCTIONS_METRICS` | no | `0` | Set to `1` to enable Prometheus `/metrics` |
| `REACTOR_LOG` | no | `info` | Tracing filter |

Boot fails fast on missing required vars (selected by feature flags + `_DEPLOYMENT` mode). `doctor` subcommand prints diagnostics including: DB reachability, auth client reachability, storage client reachability + bucket existence, each enabled runtime's self-check (wasmtime version, `bun --version`, AWS STS `GetCallerIdentity`).

---

## 13. Tracing, metrics, audit

- **Tracing**: `tracing` + JSON subscriber; every request has a `request_id` span; fields include `function`, `deployment_version`, `runtime`, `cold_start`, `duration_ms`, `bytes_in`, `bytes_out`, `policy_decision`, `status_code`.
- **Metrics**: Prometheus `/metrics` (gated by `REACTOR_FUNCTIONS_METRICS=1`):
  - `functions_invocations_total{function, runtime, status}`
  - `functions_invocation_duration_seconds{function, runtime, cold_start}`
  - `functions_cold_starts_total{function, runtime}`
  - `functions_bytes_in_total{function}`
  - `functions_bytes_out_total{function}`
  - `functions_concurrency_rejected_total{function}`
  - `functions_timeouts_total{function}`
  - `functions_warm_instances{function, runtime}` (gauge)
  - `functions_deployments_total{function, status}` (gauge)
  - `functions_policy_denied_total{function}`

### 13.1 Unified audit surface

Per the locked decision: a single audit table covers admin events, while invocation-level events are recorded in `_reactor_functions.invocations` to keep audit volume manageable.

**`audit_events` (admin only)**:
- `function.create`, `function.delete`
- `deployment.create`, `deployment.promote`, `deployment.rollback`, `deployment.fail`, `deployment.destroy`
- `env.upsert`, `env.delete`
- `policy.create`, `policy.delete`
- `policy.bypass` (when `*` permission used to skip an invoke policy)

**`invocations` (every invoke)**:
- One row per invocation; status code, duration, bytes, cold-start flag, error code
- Indexed for time-range queries by function/org/deployment
- v0.2 will add a retention policy (default 30 days) and rollup tables for analytics

The CLI's `reactor logs` command can stream against either table — the design treats both as part of the unified observability surface, with the split being purely a volume/retention concern.

---

## 14. Test surface

- **Unit**: manifest validation; bundle SHA verification; warm-pool eviction logic; timeout enforcement; policy expression evaluation; error envelope shaping.
- **Integration**: `testcontainers` Postgres + `tempdir` for `wasm`/`bun` workdirs + `localstack` for `lambda` (gated behind `LOCALSTACK_AVAILABLE` env).
- **Runtime conformance**: `tests/runtime_conformance.rs` runs identical scenarios against all enabled runtimes:
  - simple echo handler
  - streaming SSE response (10 chunks over 2s)
  - timeout-triggered handler (sleep 60s, expect 408)
  - large-body handler (4 MB upload, 5 MB download)
  - crash handler (panic / throw, expect 500)
  - cold-start measurement (assert <budget per §8.4)
- **Cross-capability**: `tests/auth_storage_integration.rs` runs the matrix `{wasm, bun, lambda} × {InProcessAuthClient, RemoteAuthClient}` × storage backed by `{Fs, S3}`:
  - signup → create function → upload bundle (verifies storage path) → deploy → promote → invoke → assert response → check audit + invocation rows
- **Lambda LocalStack lane**: separate CI job, runs only when LocalStack is available; the rest of the suite runs everywhere.

---

## 15. Cargo workspace additions

Root `Cargo.toml` additions (`[workspace.dependencies]`):

```toml
wasmtime          = { version = "25", default-features = false }
wasmtime-wasi     = "25"
wasmtime-wasi-http = "25"
aws-sdk-lambda    = "1"
aws-sdk-s3        = "1"           # already present from reactor-storage
aws-config        = "1"           # already present from reactor-storage
zip               = "2"
sha2              = "0.10"
tokio-util        = { version = "0.7", features = ["io"] }
nix               = "0.29"        # for SIGTERM/SIGKILL on bun subprocess (unix only)
```

New workspace members:

```toml
[workspace]
members = [
  "crates/reactor-core",
  "crates/reactor-policy",
  "crates/reactor-auth",
  "crates/reactor-auth-server",
  "crates/reactor-data",
  "crates/reactor-data-server",
  "crates/reactor-storage",
  "crates/reactor-storage-server",
  "crates/reactor-functions",
  "crates/reactor-functions-server",
]
```

---

## 16. Build order (v0 slice)

| # | Task | Outcome |
|---|---|---|
| 0 | Land this design doc | Reviewed contract |
| 1 | Workspace skeleton: add `reactor-functions` + `reactor-functions-server` | `cargo check --workspace` clean across feature combos |
| 2 | `reactor-functions` skeleton: config, state, router, health, error envelope | Binary boots, `/fn/v1/health` returns 200 with runtime list |
| 3 | Metadata migrations + `FunctionsStore` trait + `PgFunctionsStore` scaffold | Schema applies, trait definitions complete, smoke test green |
| 4 | Auth middleware → `FunctionCtx`, admin function CRUD (`POST/GET/DELETE /_admin/functions`) | Functions can be created, listed, deleted with permissions enforced |
| 5 | Bundle pipeline: manifest validation, SHA verify, upload to reactor-storage `_reactor_functions` bucket, deployment row | `POST /_admin/functions/{name}/deployments` persists a bundle and a `pending` deployment |
| 6 | `FunctionRuntime` trait + runtime registry; `WasmRuntime` adapter with WASI HTTP handler | A Rust-WASM "echo" function deploys, promotes, invokes, returns expected body |
| 7 | Invoke pipeline: route, permission gate, runtime dispatch, body streaming both directions, timeout, concurrency cap, response shaping | Wasm echo + 5MB body + 30s timeout all work; X-Reactor-* headers present |
| 8 | Per-function env + secrets (column-encrypted); injected into runtime at invoke | Bun function reads `process.env.SECRET` and returns it (via WASM env emulation analog) |
| 9 | `BunRuntime` adapter: subprocess spawn, Unix socket protocol, warm pool, eviction, SIGTERM/SIGKILL | TS echo + streaming SSE + timeout + crash all behave per spec |
| 10 | Invoke policies: parser reuse from reactor-policy + functions builtins + enforcement | Policy denying outside business hours returns 403 with policy_denied |
| 11 | Promote / rollback / deployment listing; current_deployment swap is atomic | Promote new version → next invoke hits new code; rollback restores |
| 12 | `LambdaRuntime` adapter: aws-sdk-lambda CreateFunction + Function URL + LWA layer; CloudWatch log subscription | Same Bun bundle deploys to LocalStack Lambda and serves invokes |
| 13 | Logs SSE endpoint; unified across all three runtimes | `GET /_admin/functions/{name}/logs?follow=1` streams live logs |
| 14 | Invocations recording (non-blocking); audit events for admin actions; metrics + tracing | DB has invocation rows; audit table has admin rows; `/metrics` populated |
| 15 | `doctor` subcommand + README quickstart + cross-capability harness | Conformance + auth-storage matrix passes |

### v0 exit checklist

- [ ] `reactor-functions-server` boots against empty Postgres + reachable reactor-storage → migrations apply, `_reactor_functions` storage bucket auto-provisioned, doctor green for all enabled runtimes.
- [ ] Function CRUD: create with each runtime → list → delete (cascades runtime resources via `runtime.destroy`).
- [ ] Deploy + promote separation: pushing a deployment does not affect live traffic; promote is the only path that swaps `current_deployment_id`.
- [ ] Invoke matrix `{wasm, bun, lambda}` × `{echo, streaming SSE, timeout, large body, crash}` produces expected responses with correct platform headers.
- [ ] Cold-start budgets per §8.4 measured and asserted in conformance tests.
- [ ] Per-function env + secrets: secret values are returned only by metadata API; raw values reach the function via runtime injection.
- [ ] Policy denial: returns 403 with `policy_denied` and audit `policy.bypass` event when `*` permission used.
- [ ] Concurrency cap: 60 concurrent requests against `max_concurrency=50` produces 10 × `429 too_many_requests` with `Retry-After`.
- [ ] Logs endpoint streams from all three runtimes through the same SSE format.
- [ ] `_reactor_functions.audit_events` populated for every admin action; `_reactor_functions.invocations` populated for every invoke (success or failure).
- [ ] Cross-capability harness passes for `{wasm, bun} × {InProcess, Remote} × {Fs, S3}` (Lambda lane gated on LocalStack availability).

### Parallel-safe pairings

- Steps 6 (`WasmRuntime`) and 9 (`BunRuntime`) are independent after step 7 (the trait + invoke pipeline land first).
- Step 10 (policies) and step 11 (promote/rollback) are independent after step 7.
- Step 12 (`LambdaRuntime`) is independent after step 9 (it shares the bun bundle pipeline).

---

## 17. Decision log

Decisions locked during v0 planning (May 2026):

| Question | Decision | Rationale |
|---|---|---|
| **v0 runtime adapters** | `wasm` + `bun` + `lambda` (3 adapters) | Exercises in-process / subprocess / remote topologies; honest test of the trait. Lambda Web Adapter shares the bundle pipeline with Bun, so the third adapter is incremental, not multiplicative. |
| **Bundle storage** | `reactor-storage` system bucket `_reactor_functions` | First internal consumer of reactor-storage; validates that contract. System buckets are policy-protected from tenant access. |
| **Streaming responses** | First-class trait method (every adapter, day one) | All v0 adapters support it natively; AI workloads demand it; bolting on later means rewriting every adapter. |
| **Audit surface** | Unified: `audit_events` for admin actions + `invocations` table for per-invoke records | Keeps audit volume manageable while still supporting end-to-end observability through the same CLI/API surface. |
| **Cold-start budgets** | Per-adapter, honestly numbered (§8.4) | The §13 spec target (<200ms p95) is achievable on Lambda only with provisioned concurrency or Bun warm-pooled. Document reality, don't paper over it. |
| **Concurrency model** | Scale-to-zero default + opt-in `min_instances` + `max_concurrency` per function | Matches Lambda/Vercel/Cloudflare semantics; safest default for cost. |
| **Built-in retries** | None | Functions are request/response; retries belong to caller or to `reactor-jobs`. Prevents at-least-once side-effect ambiguity. |
| **Cron / scheduling** | Out of scope | `reactor-jobs` owns scheduling. Functions metadata stays clean. |
| **`waitUntil` / background-after-response** | Out of scope | Conflates function lifecycle with job lifecycle. Trigger a Job from a function. |
| **Anonymous invoke** | Not supported at v0 | Matches reactor-data posture; public functions issue short-lived tokens via reactor-auth. |
| **Authorization header forwarding** | Stripped by default; opt-in via manifest `forward_authorization` | Functions get the synthesized `X-Reactor-Auth` JSON instead, so they don't need to re-validate JWTs. |
| **Bundle size cap** | 50 MiB at v0 | Lambda's direct-upload limit; avoids container-image plumbing. |
| **Runtime per function** | Immutable post-creation; switching runtimes = recreate function | Single deploy code path per function; simpler reconciliation. |
| **Deploy / promote split** | Two separate operations | Vercel/Fly model; "build broken on prod" is impossible. |
| **Function contract** | Web Standard `Request → Response` (WASI HTTP for `wasm`, Bun.serve shape for `bun`/`lambda`) | Matches Vercel Build Output API → makes Sites trivial later. |
| **Lambda invocation model** | Function URL + RESPONSE_STREAM; no API Gateway | One fewer AWS product to wire; native streaming. |
| **Lambda bundle pipeline** | Bun bundle + Lambda Web Adapter layer + bootstrap script | Bun and Lambda adapters share 90% of the pipeline. |
| **WASI version** | Preview 2 (Component Model + WASI HTTP) | Stable, supported by current Wasmtime, matches Spin/Fastly direction. |
| **Anonymous bucket access** | Tenants cannot name buckets starting with `_`; system buckets invisible | Defends `_reactor_functions` and any future internal buckets. |
| **System bucket auto-provision** | reactor-functions-server creates `_reactor_functions` bucket on boot if missing | One less manual step in the deployment story. |

---

## 18. Open questions (deferred)

1. **Bundle GC**: how aggressive to prune old deployment bundles. v0 keeps everything; v0.2 adds a configurable retention policy (default: keep last 10 + the currently-promoted version).
2. **Multi-region Lambda**: today we wire one region per server. Multi-region Lambda deploy + region-aware Function URL routing is a v0.2+ topic.
3. **Provisioned concurrency cost surface**: when `min_instances > 0` on Lambda, surface the implied AWS cost in the deploy response so deployers see it before promoting.
4. **WASM imports beyond WASI HTTP**: do we expose a Reactor-specific host import set (e.g., a typed reactor-data client)? Today the function makes plain HTTP calls. The case for in-process imports is performance, but it locks Wasm functions to monolith deployments.
5. **Cold-start streaming**: Lambda Function URL streaming has a documented first-byte delay. Measure and decide whether to surface it as a separate metric.
6. **CDN integration**: for purely-static functions (rare but possible), should responses be cacheable via CDN headers? Likely a Sites concern, not Functions.
7. **Invocation log retention**: 30 days default proposed; verify against expected per-tenant volume before v0.2 rollup tables.

---

*End of design doc. Land code against checklist §16 in order, one PR per row, this doc updated as decisions change.*
