# `reactor-jobs` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Fifth crate of the Reactor.cloud BaaS. Owns the Jobs capability per `docs/ReactorCloud_spec.md` §2/§3/§6.1/§10-J.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, job manifest format, SDK shape, scheduler, schema, policy integration — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Provide **durable execution** on top of `reactor-functions`: a job is a function that survives crashes, retries on failure, and checkpoints progress across steps.
2. Ship a **thin `reactor-cache` abstraction** (queue + KV) backed by Postgres v0, swappable to Redis v0.2, usable by other capabilities.
3. Support **four trigger types** at v0: `cron` (scheduled), `webhook` (external HTTP), `event` (internal pub-sub), `manual` (API call).
4. **Checkpoint execution via steps**: `ctx.step(name, fn)` persists state before executing, enabling automatic replay on retry.
5. **Integrate with reactor-data**: jobs get a `ctx.data` client to query/mutate user tables using internal auth.
6. **Reuse reactor-functions runtimes** unchanged: bun/wasm/lambda. A job bundle is a function bundle with additional manifest fields.
7. Be the only crate allowed to touch the `_reactor_jobs.*` Postgres metadata schema.
8. **Pave the way for v0.2 workflows**: the data model supports conditionals, loops, and fan-out without schema changes.

## 2. Non-goals (v0)

- **Conditional branching** (`ctx.branch(cond, ifTrue, ifFalse)`) — v0.2 after the step model proves out.
- **Loops** (`ctx.loop(items, fn)` with parallelism) — v0.2.
- **Fan-out / fan-in** (`ctx.parallel([...])`) — v0.2.
- **Visual workflow builder** — depends on v0.2 workflow primitives.
- **Redis backend** for reactor-cache — v0 ships Postgres only; Redis is v0.2.
- **External queue integrations** (SQS, RabbitMQ) — v0 uses internal queue only.
- **Cross-job transactions** — jobs are isolated; use reactor-data transactions within a job.
- **Multi-language SDK** — TypeScript-first (Bun-native) at v0; Rust/wasm SDK v0.2.
- **Multi-region scheduler** — single-server scheduler at v0.
- **Rate limiting per trigger** — use reactor-auth rate limits or external gateway.
- **Job versioning** (like function deployments) — jobs inherit the function's deployment model.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, IDs, AuthClient trait
├── reactor-policy/                # (existing) shared policy engine
├── reactor-auth/                  # (existing)
├── reactor-data/                  # (existing) — jobs can query/mutate user tables
├── reactor-storage/               # (existing)
├── reactor-functions/             # (existing) — jobs invoke via this
│
├── reactor-cache/                 # NEW: queue + KV abstraction
│   ├── Cargo.toml
│   ├── migrations/                # migrations for _reactor_cache.* (queue + kv tables)
│   │   └── 001_queue_kv.sql
│   └── src/
│       ├── lib.rs
│       ├── backend.rs             # CacheBackend trait
│       ├── postgres.rs            # PostgresBackend (SKIP LOCKED queue, simple KV)
│       ├── queue.rs               # Queue operations (enqueue, dequeue, ack, nack)
│       └── kv.rs                   # KV operations (get, set, del, expire)
│
├── reactor-jobs/                  # the jobs library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_jobs.*
│   │   ├── 001_metadata.sql
│   │   ├── 002_triggers.sql
│   │   ├── 003_runs.sql
│   │   ├── 004_steps.sql
│   │   ├── 005_state.sql
│   │   ├── 006_events.sql
│   │   ├── 007_dlq.sql
│   │   └── 008_audit.sql
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # JobsConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # JobsState, JobCtx
│       ├── error.rs               # JobsError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs
│       │   ├── admin.rs           # job CRUD
│       │   ├── triggers.rs        # trigger CRUD
│       │   ├── runs.rs            # run listing, cancel, retry
│       │   ├── invoke.rs          # manual + webhook triggers
│       │   └── logs.rs            # SSE log tail
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth.rs            # bearer + X-Reactor-Org → JobCtx
│       │
│       ├── scheduler/
│       │   ├── mod.rs
│       │   ├── cron.rs            # cron trigger polling
│       │   ├── event.rs           # event matching
│       │   └── sleep.rs           # durable sleep wakeup
│       │
│       ├── worker/
│       │   ├── mod.rs
│       │   ├── pool.rs            # worker pool management
│       │   ├── executor.rs        # run execution (invoke reactor-functions)
│       │   └── checkpoint.rs      # step checkpoint management
│       │
│       ├── manifest/
│       │   ├── mod.rs             # JobManifest type (extends function manifest)
│       │   └── validate.rs        # manifest validation
│       │
│       ├── sdk/
│       │   ├── mod.rs
│       │   └── context.rs         # JobContext injected into function
│       │
│       ├── store/
│       │   ├── mod.rs             # JobsStore trait
│       │   └── postgres.rs        # PgJobsStore
│       │
│       └── audit.rs               # admin-event audit writer
│
├── reactor-jobs-server/           # standalone bin
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # axum bind + scheduler + workers + serve
│       └── cli/
│           ├── mod.rs
│           └── doctor.rs          # connectivity diagnostics
│
└── packages/
    └── jobs-sdk/                  # TypeScript SDK (npm package)
        ├── package.json
        ├── src/
        │   ├── index.ts           # main exports
        │   ├── context.ts         # JobContext implementation
        │   ├── step.ts            # step() with checkpointing
        │   ├── state.ts           # state get/set
        │   ├── emit.ts            # event emission
        │   └── sleep.ts           # durable sleep
        └── tsconfig.json
```

Conventions:
- `reactor-jobs` depends on `reactor-core`, `reactor-policy`, `reactor-cache`, and `reactor-functions` (as a client).
- `reactor-cache` is a standalone crate, usable by other capabilities (functions warm-pool, sites build cache).
- The TypeScript SDK (`@reactor/jobs-sdk`) is published to npm; Bun jobs import it.
- All three function runtimes (`wasm`, `bun`, `lambda`) can run jobs, but the full SDK is TypeScript-only at v0.

---

## 4. Core types

### 4.1 ID & types

All IDs are `ReactorId` (UUIDv7) from `reactor-core`. Jobs-specific types:

| Type | Rust | Notes |
|---|---|---|
| `JobId` | `ReactorId` | Primary key for jobs (mirrors FunctionId) |
| `TriggerId` | `ReactorId` | Primary key for triggers |
| `RunId` | `ReactorId` | Primary key for job runs |
| `StepId` | `ReactorId` | Primary key for steps within a run |
| `EventId` | `ReactorId` | Primary key for internal events |
| `TriggerKind` | `enum { Cron, Webhook, Event, Manual }` | How the run was initiated |
| `RunStatus` | `enum { Pending, Running, Sleeping, Succeeded, Failed, Cancelled }` | Run lifecycle |
| `StepStatus` | `enum { Pending, Running, Completed, Failed, Skipped }` | Step lifecycle |

### 4.2 `JobCtx` (request-local)

Constructed by middleware once per request from `AuthCtx`:

```rust
// reactor-jobs/src/state.rs
#[derive(Debug, Clone)]
pub struct JobCtx {
    pub auth:       AuthCtx,
    pub request_id: String,
    pub org_id:     OrgId,
}

impl JobCtx {
    pub fn user_id(&self) -> Option<&UserId> { self.auth.user_id() }
    pub fn active_org(&self) -> &OrgId { &self.org_id }
    pub fn has_permission(&self, perm: &str) -> bool {
        self.auth.has_permission(perm)
    }
}
```

### 4.3 `JobContext` (SDK, injected into function)

The TypeScript SDK exposes this interface to job code:

```typescript
// @reactor/jobs-sdk
interface JobContext {
  // Run metadata
  readonly runId: string;
  readonly jobName: string;
  readonly attempt: number;
  readonly trigger: TriggerInfo;
  
  // Step execution with checkpointing
  step<T>(name: string, fn: () => Promise<T>): Promise<T>;
  
  // Durable state (persisted to DB)
  state: {
    get<T>(key: string): Promise<T | undefined>;
    set<T>(key: string, value: T): Promise<void>;
    delete(key: string): Promise<void>;
  };
  
  // Event emission (triggers other jobs)
  emit(topic: string, payload: unknown): Promise<void>;
  
  // Durable sleep (releases worker, wakes up later)
  sleep(name: string, duration: string | number): Promise<void>;
  
  // reactor-data client (query/mutate user tables)
  data: DataClient;
  
  // Logging
  log: {
    info(message: string, data?: Record<string, unknown>): void;
    warn(message: string, data?: Record<string, unknown>): void;
    error(message: string, data?: Record<string, unknown>): void;
  };
}
```

### 4.4 `CacheBackend` trait (reactor-cache)

```rust
// reactor-cache/src/backend.rs
#[async_trait]
pub trait CacheBackend: Send + Sync + 'static {
    // Queue operations
    async fn enqueue(&self, queue: &str, item: &[u8], delay: Option<Duration>) -> Result<String, CacheError>;
    async fn dequeue(&self, queue: &str, count: u32, visibility_timeout: Duration) -> Result<Vec<QueueItem>, CacheError>;
    async fn ack(&self, queue: &str, receipt: &str) -> Result<(), CacheError>;
    async fn nack(&self, queue: &str, receipt: &str, delay: Option<Duration>) -> Result<(), CacheError>;
    
    // KV operations
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<(), CacheError>;
    async fn del(&self, key: &str) -> Result<bool, CacheError>;
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, CacheError>;
}

#[derive(Debug)]
pub struct QueueItem {
    pub id: String,
    pub receipt: String,
    pub data: Vec<u8>,
    pub enqueued_at: DateTime<Utc>,
    pub attempt: u32,
}
```

### 4.5 `JobsStore` trait

```rust
// reactor-jobs/src/store/mod.rs
#[async_trait]
pub trait JobsStore: Send + Sync + 'static {
    // Job CRUD (inherits function_id; job is metadata overlay)
    async fn create_job(&self, j: &NewJob) -> Result<Job, JobsError>;
    async fn get_job(&self, org: &OrgId, name: &str) -> Result<Option<Job>, JobsError>;
    async fn list_jobs(&self, org: &OrgId) -> Result<Vec<Job>, JobsError>;
    async fn delete_job(&self, id: &JobId) -> Result<(), JobsError>;
    
    // Triggers
    async fn create_trigger(&self, t: &NewTrigger) -> Result<Trigger, JobsError>;
    async fn get_triggers(&self, job_id: &JobId) -> Result<Vec<Trigger>, JobsError>;
    async fn delete_trigger(&self, id: &TriggerId) -> Result<(), JobsError>;
    async fn list_due_cron_triggers(&self, now: DateTime<Utc>) -> Result<Vec<Trigger>, JobsError>;
    
    // Runs
    async fn create_run(&self, r: &NewRun) -> Result<Run, JobsError>;
    async fn get_run(&self, id: &RunId) -> Result<Option<Run>, JobsError>;
    async fn update_run_status(&self, id: &RunId, status: RunStatus, detail: Option<&str>) -> Result<(), JobsError>;
    async fn list_runs(&self, job_id: &JobId, limit: u32) -> Result<Vec<Run>, JobsError>;
    async fn list_sleeping_runs_due(&self, now: DateTime<Utc>) -> Result<Vec<Run>, JobsError>;
    
    // Steps
    async fn create_step(&self, s: &NewStep) -> Result<Step, JobsError>;
    async fn get_step(&self, run_id: &RunId, name: &str) -> Result<Option<Step>, JobsError>;
    async fn update_step(&self, id: &StepId, status: StepStatus, output: Option<&serde_json::Value>) -> Result<(), JobsError>;
    async fn list_steps(&self, run_id: &RunId) -> Result<Vec<Step>, JobsError>;
    
    // State (per-run KV)
    async fn get_state(&self, run_id: &RunId, key: &str) -> Result<Option<serde_json::Value>, JobsError>;
    async fn set_state(&self, run_id: &RunId, key: &str, value: &serde_json::Value) -> Result<(), JobsError>;
    async fn delete_state(&self, run_id: &RunId, key: &str) -> Result<(), JobsError>;
    async fn list_state(&self, run_id: &RunId) -> Result<Vec<StateEntry>, JobsError>;
    
    // Events (internal pub-sub)
    async fn emit_event(&self, e: &NewEvent) -> Result<Event, JobsError>;
    async fn consume_event(&self, id: &EventId, run_id: &RunId) -> Result<(), JobsError>;
    async fn list_pending_events(&self, topic: &str, limit: u32) -> Result<Vec<Event>, JobsError>;
    
    // DLQ
    async fn move_to_dlq(&self, run_id: &RunId, reason: &str) -> Result<(), JobsError>;
    async fn list_dlq(&self, job_id: &JobId, limit: u32) -> Result<Vec<DlqEntry>, JobsError>;
    async fn retry_from_dlq(&self, dlq_id: &ReactorId) -> Result<RunId, JobsError>;
    
    // Audit
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), JobsError>;
}
```

---

## 5. HTTP surface (v0)

### 5.1 Health

```
GET    /jobs/v1/health
       → 200 { "status": "ok", "version": "0.1.0", "scheduler": "running", "workers": 4 }
```

### 5.2 Admin: jobs

```
POST   /jobs/v1/_admin/jobs
       Body: { "name": "process-order", "function_name": "process-order" }
       → 201 { job }
       Requires: jobs:create
       Note: The function must already exist in reactor-functions

GET    /jobs/v1/_admin/jobs
       → 200 [ job, ... ]
       Lists jobs in active org

GET    /jobs/v1/_admin/jobs/{name}
       → 200 { job, triggers, recent_runs }

DELETE /jobs/v1/_admin/jobs/{name}
       → 204
       Requires: jobs:{name}:admin
       Cascades: deletes triggers, pending runs cancelled
```

### 5.3 Admin: triggers

```
POST   /jobs/v1/_admin/jobs/{name}/triggers
       Body: { "kind": "cron", "config": { "schedule": "0 * * * *" } }
             { "kind": "event", "config": { "topic": "order.created" } }
             { "kind": "webhook", "config": {} }  // generates token
       → 201 { trigger }
       Requires: jobs:{name}:admin

GET    /jobs/v1/_admin/jobs/{name}/triggers
       → 200 [ trigger, ... ]

DELETE /jobs/v1/_admin/jobs/{name}/triggers/{trigger_id}
       → 204
       Requires: jobs:{name}:admin
```

### 5.4 Admin: runs

```
GET    /jobs/v1/_admin/jobs/{name}/runs
       Query: ?status=failed&limit=50
       → 200 [ run, ... ]

GET    /jobs/v1/_admin/jobs/{name}/runs/{run_id}
       → 200 { run, steps, state }

POST   /jobs/v1/_admin/jobs/{name}/runs/{run_id}/cancel
       → 200 { run }
       Requires: jobs:{name}:admin

POST   /jobs/v1/_admin/jobs/{name}/runs/{run_id}/retry
       → 201 { run }  // new run created
       Requires: jobs:{name}:admin
```

### 5.5 Admin: DLQ

```
GET    /jobs/v1/_admin/jobs/{name}/dlq
       → 200 [ dlq_entry, ... ]

POST   /jobs/v1/_admin/jobs/{name}/dlq/{dlq_id}/retry
       → 201 { run }  // new run from DLQ entry
       Requires: jobs:{name}:admin

DELETE /jobs/v1/_admin/jobs/{name}/dlq/{dlq_id}
       → 204  // discard DLQ entry
       Requires: jobs:{name}:admin
```

### 5.6 Invoke: manual trigger

```
POST   /jobs/v1/{name}/trigger
       Body: { "payload": { ... } }  // optional
       Auth: Bearer JWT required
       → 202 { run_id, status: "pending" }
       Requires: jobs:{name}:invoke
```

### 5.7 Invoke: webhook trigger

```
POST   /jobs/v1/webhooks/{token}
       Body: raw (forwarded as payload)
       Auth: None (token is auth)
       → 202 { run_id }
       
       The token is opaque, contains encrypted (job_id, trigger_id, org_id).
       Invalid/expired token → 404
```

### 5.8 Admin: logs

```
GET    /jobs/v1/_admin/jobs/{name}/logs
       Query: ?run_id=...&since=...&limit=200&follow=1
       → 200 (text/event-stream when follow=1, otherwise application/json)
       
       SSE event shape:
         event: log
         data: { "ts": "...", "level": "info", "run_id": "...", "step": "...", "message": "..." }
       
       Requires: jobs:{name}:logs
```

### 5.9 Headers

| Header | Direction | Meaning |
|---|---|---|
| `Authorization: Bearer <jwt>` | inbound | Required for admin routes and manual trigger |
| `X-Reactor-Org: <ref>` | inbound | Active org override |
| `X-Request-Id` | both | Generated if absent; echoed in response |
| `X-Reactor-Job-Context` | to function | JSON with run_id, step cache, state |
| `X-Reactor-Run-Id` | response | Run ID for tracking |

### 5.10 Error envelope

Same shape as other capabilities:

```json
{
  "error": {
    "code": "run_failed",
    "message": "Job 'process-order' run failed after 3 attempts.",
    "status": 500,
    "request_id": "req_01HZ...",
    "details": {
      "job": "process-order",
      "run_id": "run_01HZ...",
      "attempt": 3,
      "error": "Connection refused"
    }
  }
}
```

Error codes: `job_not_found`, `trigger_not_found`, `run_not_found`, `run_already_complete`, `run_cancelled`, `invalid_cron`, `invalid_trigger_config`, `step_failed`, `max_attempts_exceeded`, `concurrency_exceeded`, `webhook_token_invalid`, `payload_too_large`, `policy_denied`.

---

## 6. Database schema (`_reactor_jobs`)

```sql
create schema if not exists _reactor_jobs;

-- 6.1 Jobs (metadata overlay on functions)
create table _reactor_jobs.jobs (
  id                       uuid primary key,
  org_id                   uuid not null,
  name                     citext not null,
  function_name            citext not null,          -- references reactor-functions
  description              text,
  retry_max_attempts       integer not null default 3,
  retry_backoff            text not null default 'exponential',  -- 'linear' | 'exponential'
  retry_initial_delay_ms   integer not null default 1000,
  retry_max_delay_ms       integer not null default 60000,
  max_concurrency          integer not null default 10,
  timeout_ms               integer not null default 600000,      -- 10 minutes default
  created_at               timestamptz not null default now(),
  updated_at               timestamptz not null default now(),
  unique (org_id, name)
);
create index on _reactor_jobs.jobs (org_id);

-- 6.2 Triggers
create table _reactor_jobs.triggers (
  id                       uuid primary key,
  job_id                   uuid not null references _reactor_jobs.jobs(id) on delete cascade,
  kind                     text not null,            -- 'cron' | 'webhook' | 'event' | 'manual'
  config_json              jsonb not null default '{}',
  webhook_token            text unique,              -- for webhook triggers only
  enabled                  boolean not null default true,
  last_triggered_at        timestamptz,
  next_trigger_at          timestamptz,              -- for cron: precomputed next fire time
  created_at               timestamptz not null default now()
);
create index on _reactor_jobs.triggers (job_id);
create index on _reactor_jobs.triggers (kind, next_trigger_at) where kind = 'cron' and enabled;
create index on _reactor_jobs.triggers (kind, config_json) where kind = 'event' and enabled;  -- for topic lookup

-- 6.3 Runs (one per job execution)
create table _reactor_jobs.runs (
  id                       uuid primary key,
  job_id                   uuid not null references _reactor_jobs.jobs(id) on delete cascade,
  org_id                   uuid not null,
  trigger_id               uuid references _reactor_jobs.triggers(id) on delete set null,
  trigger_kind             text not null,
  status                   text not null default 'pending',  -- pending, running, sleeping, succeeded, failed, cancelled
  payload_json             jsonb not null default '{}',
  attempt                  integer not null default 1,
  max_attempts             integer not null,
  started_at               timestamptz,
  finished_at              timestamptz,
  wakeup_at                timestamptz,              -- for sleeping runs
  error_code               text,
  error_message            text,
  created_at               timestamptz not null default now()
);
create index on _reactor_jobs.runs (job_id, created_at desc);
create index on _reactor_jobs.runs (org_id, created_at desc);
create index on _reactor_jobs.runs (status) where status in ('pending', 'running', 'sleeping');
create index on _reactor_jobs.runs (wakeup_at) where status = 'sleeping';

-- 6.4 Steps (checkpoints within a run)
create table _reactor_jobs.steps (
  id                       uuid primary key,
  run_id                   uuid not null references _reactor_jobs.runs(id) on delete cascade,
  name                     text not null,
  status                   text not null default 'pending',  -- pending, running, completed, failed, skipped
  input_json               jsonb,
  output_json              jsonb,                    -- cached result for replay
  attempt                  integer not null default 1,
  started_at               timestamptz,
  finished_at              timestamptz,
  error_message            text,
  unique (run_id, name)
);
create index on _reactor_jobs.steps (run_id);

-- 6.5 State (per-run durable KV)
create table _reactor_jobs.state (
  run_id                   uuid not null references _reactor_jobs.runs(id) on delete cascade,
  key                      text not null,
  value_json               jsonb not null,
  updated_at               timestamptz not null default now(),
  primary key (run_id, key)
);

-- 6.6 Events (internal pub-sub)
create table _reactor_jobs.events (
  id                       uuid primary key,
  org_id                   uuid not null,
  topic                    text not null,
  payload_json             jsonb not null default '{}',
  emitted_by_run_id        uuid references _reactor_jobs.runs(id) on delete set null,
  consumed_by_run_id       uuid references _reactor_jobs.runs(id) on delete set null,
  created_at               timestamptz not null default now(),
  consumed_at              timestamptz
);
create index on _reactor_jobs.events (org_id, topic, created_at) where consumed_at is null;

-- 6.7 DLQ (dead letter queue)
create table _reactor_jobs.dlq (
  id                       uuid primary key,
  run_id                   uuid not null,            -- original run (may be deleted)
  job_id                   uuid not null references _reactor_jobs.jobs(id) on delete cascade,
  org_id                   uuid not null,
  payload_json             jsonb not null,
  error_code               text,
  error_message            text,
  attempt                  integer not null,
  created_at               timestamptz not null default now()
);
create index on _reactor_jobs.dlq (job_id, created_at desc);

-- 6.8 Audit events (admin actions)
create table _reactor_jobs.audit_events (
  id                       uuid primary key,
  ts                       timestamptz not null default now(),
  actor_user_id            uuid,
  actor_apikey_id          uuid,
  org_id                   uuid,
  job_id                   uuid,
  run_id                   uuid,
  event_type               text not null,
  details                  jsonb not null default '{}',
  request_id               text not null
);
create index on _reactor_jobs.audit_events (org_id, ts desc);
create index on _reactor_jobs.audit_events (job_id, ts desc);
```

### 6.9 reactor-cache schema (`_reactor_cache`)

```sql
create schema if not exists _reactor_cache;

-- Queue table (SKIP LOCKED pattern)
create table _reactor_cache.queue (
  id                       uuid primary key,
  queue_name               text not null,
  data                     bytea not null,
  visible_at               timestamptz not null default now(),
  attempt                  integer not null default 0,
  created_at               timestamptz not null default now(),
  receipt                  text unique              -- for ack/nack
);
create index on _reactor_cache.queue (queue_name, visible_at) where receipt is null;

-- KV table
create table _reactor_cache.kv (
  key                      text primary key,
  value                    bytea not null,
  expires_at               timestamptz,
  created_at               timestamptz not null default now(),
  updated_at               timestamptz not null default now()
);
create index on _reactor_cache.kv (expires_at) where expires_at is not null;
```

### 6.10 Role grants

`_reactor_jobs` and `_reactor_cache` are **not** readable by user application roles. `reactor-jobs-server` connects with a dedicated role.

---

## 7. Job manifest format

Jobs use the same bundle format as functions, with an additional `job` field in the manifest:

```json
{
  "name": "process-order",
  "version": 1,
  "runtime": "bun",
  "entrypoint": "code/index.ts",
  
  "limits": {
    "timeout_ms": 600000,
    "memory_mb": 512
  },
  
  "job": {
    "triggers": [
      { "kind": "cron", "schedule": "0 * * * *" },
      { "kind": "event", "topic": "order.created" },
      { "kind": "webhook" }
    ],
    "retry": {
      "maxAttempts": 3,
      "backoff": "exponential",
      "initialDelayMs": 1000,
      "maxDelayMs": 60000
    },
    "maxConcurrency": 5,
    "timeoutMs": 600000
  },
  
  "env_keys": ["API_URL"],
  "secret_keys": ["API_KEY"]
}
```

If `job` field is absent, the bundle is a regular function. If present, reactor-jobs takes over scheduling and execution.

### 7.1 Trigger config schemas

**Cron:**
```json
{ "kind": "cron", "schedule": "0 */6 * * *" }  // standard cron syntax
```

**Event:**
```json
{ "kind": "event", "topic": "order.created" }
```

**Webhook:**
```json
{ "kind": "webhook" }  // token generated server-side
```

**Manual:** (implicit, always available)
```json
{ "kind": "manual" }
```

---

## 8. Execution flow

### 8.1 Trigger → Run creation

```
Trigger fires (cron tick / webhook hit / event emitted / manual API call)
      ▼
scheduler/trigger handler
  - validate trigger still active
  - check concurrency limit (deny with 429 if exceeded)
  - create run row (status=pending, attempt=1)
  - enqueue run_id to reactor-cache queue "jobs:{org_id}"
      ▼
return run_id to caller (202 Accepted for webhook/manual)
```

### 8.2 Worker → Execution

```
worker.dequeue("jobs:{org_id}")
      ▼
load run + job metadata
  - if run.status != pending/sleeping → skip (already processed)
  - update run.status = running
      ▼
load completed steps (for replay)
load current state
      ▼
build X-Reactor-Job-Context header:
  { run_id, attempt, step_cache: { step_name: output, ... }, state: { ... } }
      ▼
POST reactor-functions /fn/v1/{function_name}
  - Authorization: internal service token
  - X-Reactor-Job-Context: <context json>
  - Body: payload_json from trigger
      ▼
function executes with SDK:
  - ctx.step() checks step_cache first; if hit, returns cached output
  - ctx.step() on miss: persist step (status=running), execute, persist output
  - ctx.state.set() persists to _reactor_jobs.state
  - ctx.emit() inserts to _reactor_jobs.events
  - ctx.sleep() updates run.status=sleeping, run.wakeup_at=now()+duration, returns
      ▼
on function success:
  - run.status = succeeded
  - run.finished_at = now()
      ▼
on function error:
  - if attempt < max_attempts:
    - run.status = pending, run.attempt++
    - compute backoff delay
    - enqueue with delay
  - else:
    - run.status = failed
    - move to DLQ
```

### 8.3 Sleep → Wake flow

```
scheduler polls: SELECT * FROM runs WHERE status = 'sleeping' AND wakeup_at <= now()
      ▼
for each sleeping run:
  - run.status = pending
  - enqueue run_id (resume execution)
```

### 8.4 Event → Trigger flow

```
ctx.emit(topic, payload)
  - INSERT INTO _reactor_jobs.events
      ▼
scheduler polls: SELECT events WHERE consumed_at IS NULL
      ▼
for each event:
  - find triggers WHERE kind = 'event' AND config_json->>'topic' = event.topic
  - for each matching trigger:
    - create run with payload = event.payload_json
    - mark event.consumed_by_run_id
```

### 8.5 Sequence diagram

```
┌─────────┐  ┌───────────┐  ┌─────────┐  ┌──────────┐  ┌──────────────────┐
│ Trigger │  │ Scheduler │  │  Queue  │  │  Worker  │  │ reactor-functions│
└────┬────┘  └─────┬─────┘  └────┬────┘  └────┬─────┘  └────────┬─────────┘
     │             │             │            │                  │
     │ fire        │             │            │                  │
     │────────────>│             │            │                  │
     │             │ create run  │            │                  │
     │             │ enqueue     │            │                  │
     │             │────────────>│            │                  │
     │             │             │            │                  │
     │             │             │  dequeue   │                  │
     │             │             │<───────────│                  │
     │             │             │   run_id   │                  │
     │             │             │───────────>│                  │
     │             │             │            │  invoke function │
     │             │             │            │─────────────────>│
     │             │             │            │                  │
     │             │             │            │  ctx.step()      │
     │             │             │            │<─────────────────│
     │             │             │            │  checkpoint      │
     │             │             │            │─────────────────>│
     │             │             │            │                  │
     │             │             │            │  response        │
     │             │             │            │<─────────────────│
     │             │             │            │                  │
     │             │             │  ack       │                  │
     │             │             │<───────────│                  │
```

---

## 9. SDK contract

### 9.1 TypeScript SDK (`@reactor/jobs-sdk`)

```typescript
// Example job using the SDK
import { JobContext } from '@reactor/jobs-sdk';

export default {
  async fetch(request: Request, env: Env, ctx: JobContext): Promise<Response> {
    const payload = await request.json();
    
    // Step 1: Fetch user (checkpointed)
    const user = await ctx.step('fetch-user', async () => {
      const resp = await fetch(`${env.API_URL}/users/${payload.userId}`);
      return resp.json();
    });
    
    // Step 2: Process order (checkpointed)
    const order = await ctx.step('process-order', async () => {
      // ... business logic
      return { orderId: '123', total: 99.99 };
    });
    
    // Persist state for later steps
    await ctx.state.set('processedOrder', order);
    
    // Step 3: Send notification
    await ctx.step('send-notification', async () => {
      await fetch(`${env.NOTIFY_URL}/send`, {
        method: 'POST',
        body: JSON.stringify({ userId: user.id, orderId: order.orderId })
      });
    });
    
    // Emit event for downstream jobs
    await ctx.emit('order.processed', { orderId: order.orderId });
    
    return new Response(JSON.stringify({ success: true, orderId: order.orderId }));
  }
};
```

### 9.2 Step checkpointing rules

1. If step name exists in step_cache → return cached output immediately
2. Otherwise:
   - Insert step row (status=running)
   - Execute the function
   - On success: update step (status=completed, output_json=result)
   - On failure: update step (status=failed), throw to trigger retry
3. Idempotent: re-executing a completed step returns cached value

### 9.3 Durable sleep

```typescript
// Wait 24 hours between steps
await ctx.sleep('wait-for-cooling-off', '24h');

// This releases the worker. When wakeup_at is reached:
// - run.status changes from 'sleeping' to 'pending'
// - run is re-enqueued
// - function is re-invoked
// - completed steps are skipped (replay from cache)
// - execution resumes after the sleep() call
```

### 9.4 WASM/Lambda jobs

For non-Bun runtimes, the context is passed via environment:
- `REACTOR_JOB_CONTEXT` = JSON string with run_id, attempt, step_cache, state
- SDK operations become HTTP calls to reactor-jobs internal endpoints
- Full TypeScript SDK features not available; basic step/state/emit work

---

## 10. Scheduler

### 10.1 Architecture

Single in-process scheduler in `reactor-jobs-server`:
- One tokio task for cron polling (every 1s)
- One tokio task for event matching (every 1s)
- One tokio task for sleep wakeups (every 1s)
- Worker pool (configurable, default 4) dequeuing from reactor-cache

### 10.2 Cron trigger

```rust
// Every second:
SELECT * FROM _reactor_jobs.triggers
WHERE kind = 'cron' AND enabled AND next_trigger_at <= now()
FOR UPDATE SKIP LOCKED;

// For each:
// 1. Create run
// 2. Enqueue to reactor-cache
// 3. Update trigger.last_triggered_at = now()
// 4. Compute and update trigger.next_trigger_at
```

### 10.3 Multi-worker safety

- `FOR UPDATE SKIP LOCKED` prevents duplicate trigger processing
- reactor-cache's `dequeue` uses `SKIP LOCKED` for competing workers
- `ack` on success, `nack` with delay on failure

---

## 11. Policy integration

### 11.1 Builtins

| Builtin | Type | Description |
|---|---|---|
| `job.name` | `text` | The job's name |
| `job.function_name` | `text` | Underlying function name |
| `trigger.kind` | `text` | `'cron' \| 'webhook' \| 'event' \| 'manual'` |
| `trigger.topic` | `text` | Event topic (null for non-event triggers) |
| `run.attempt` | `integer` | Current attempt number |

### 11.2 Example policies

```sql
-- Only allow manual triggers during business hours
policy biz_hours on job "process-order"
  using (trigger.kind != 'manual' or extract(hour from now()) between 9 and 17);

-- Limit webhook triggers to specific topics
policy webhook_topics on job "data-sync"
  using (trigger.kind != 'webhook' or trigger.topic in ('user.created', 'user.updated'));
```

---

## 12. Configuration

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_JOBS_DATABASE_URL` | yes | — | Postgres connection string |
| `REACTOR_JOBS_BIND` | no | `0.0.0.0:8005` | HTTP bind address |
| `REACTOR_JOBS_FUNCTIONS_URL` | yes | — | URL of reactor-functions-server |
| `REACTOR_JOBS_FUNCTIONS_API_KEY` | yes | — | Internal API key for reactor-functions |
| `REACTOR_JOBS_DATA_URL` | no | — | URL of reactor-data-server (for ctx.data) |
| `REACTOR_JOBS_DATA_API_KEY` | no | — | Internal API key for reactor-data |
| `REACTOR_JOBS_WORKER_COUNT` | no | `4` | Number of worker tasks |
| `REACTOR_JOBS_SCHEDULER_INTERVAL_MS` | no | `1000` | Scheduler poll interval |
| `REACTOR_JOBS_DEFAULT_TIMEOUT_MS` | no | `600000` | Default job timeout (10 min) |
| `REACTOR_JOBS_MAX_TIMEOUT_MS` | no | `3600000` | Max job timeout (1 hour) |
| `REACTOR_JOBS_WEBHOOK_SECRET` | yes | — | Secret for webhook token encryption |
| `REACTOR_JOBS_DEPLOYMENT` | no | `monolith` | `monolith` or `microservices` |
| `REACTOR_JOBS_AUTH_URL` | yes (microservices) | — | URL of reactor-auth-server |
| `REACTOR_JOBS_INTERNAL_SECRET` | yes (microservices) | — | Shared secret for internal endpoints |
| `REACTOR_JOBS_AUTH_DATABASE_URL` | yes (monolith) | — | Postgres URL for auth schema |
| `REACTOR_JOBS_AUTH_DATA_KEY` | yes (monolith) | — | Auth column-encryption key |
| `REACTOR_JOBS_METRICS` | no | `0` | Set to `1` to enable Prometheus `/metrics` |
| `REACTOR_LOG` | no | `info` | Tracing filter |

---

## 13. Tracing, metrics, audit

- **Tracing**: `tracing` + JSON subscriber; every run has a `run_id` span; fields include `job`, `trigger_kind`, `attempt`, `step`, `duration_ms`, `status`.
- **Metrics**: Prometheus `/metrics` (gated by `REACTOR_JOBS_METRICS=1`):
  - `jobs_runs_total{job, trigger_kind, status}`
  - `jobs_run_duration_seconds{job, trigger_kind}`
  - `jobs_steps_total{job, step, status}`
  - `jobs_step_duration_seconds{job, step}`
  - `jobs_retries_total{job}`
  - `jobs_dlq_total{job}`
  - `jobs_concurrency_rejected_total{job}`
  - `jobs_events_emitted_total{topic}`
  - `jobs_queue_depth{queue}` (gauge)
  - `jobs_sleeping_runs` (gauge)

### 13.1 Audit events

- `job.create`, `job.delete`
- `trigger.create`, `trigger.delete`, `trigger.disable`, `trigger.enable`
- `run.cancel`, `run.retry`
- `dlq.retry`, `dlq.delete`

---

## 14. Test surface

- **Unit**: manifest validation, cron parsing, backoff calculation, step cache logic, webhook token encoding.
- **Integration**: `testcontainers` Postgres + reactor-functions mock.
- **Scheduler conformance**: cron fires on time, events match topics, sleeps wake up.
- **Step replay**: crash mid-run → restart → completed steps skipped, execution resumes.
- **Cross-capability**: `tests/jobs_integration.rs` runs:
  - create job → add cron trigger → wait for run → assert completed
  - create job → emit event → assert run triggered
  - create job → webhook call → assert run triggered
  - step failure → retry → DLQ after max attempts

---

## 15. Cargo workspace additions

Root `Cargo.toml` additions:

```toml
[workspace]
members = [
  # ... existing ...
  "crates/reactor-cache",
  "crates/reactor-jobs",
  "crates/reactor-jobs-server",
]

[workspace.dependencies]
cron = "0.12"
reactor-cache = { path = "crates/reactor-cache" }
reactor-jobs = { path = "crates/reactor-jobs" }
```

---

## 16. Build order (v0 slice)

| # | Task | Outcome |
|---|---|---|
| 0 | Land this design doc | Reviewed contract |
| 1 | Workspace skeleton: `reactor-cache`, `reactor-jobs`, `reactor-jobs-server` | `cargo check --workspace` clean |
| 2 | `reactor-cache`: `CacheBackend` trait + `PostgresBackend` | Queue + KV working against Postgres |
| 3 | Jobs metadata schema + `JobsStore` trait + `PgJobsStore` | Schema applies, trait definitions complete |
| 4 | Auth middleware → `JobCtx`, admin job CRUD | Jobs can be created, listed, deleted |
| 5 | Trigger CRUD + cron parser | Triggers created with validation |
| 6 | In-process scheduler: cron polling, enqueue runs | Cron triggers fire and create runs |
| 7 | Worker pool: dequeue, invoke reactor-functions | Runs execute via functions |
| 8 | Step checkpoint + replay | Steps persist and replay on retry |
| 9 | TypeScript SDK: `ctx.step`, `ctx.state`, `ctx.emit`, `ctx.sleep` | SDK npm package working |
| 10 | State persistence: load/save per run | `ctx.state.get/set` works |
| 11 | Event triggers: emit → match → trigger | `ctx.emit` triggers downstream jobs |
| 12 | Webhook + manual triggers | Both invoke paths work |
| 13 | Retry + DLQ: backoff, max attempts, DLQ routing | Failed runs land in DLQ |
| 14 | Concurrency control | Per-job semaphore, 429 on overflow |
| 15 | `ctx.sleep` durable wait | Sleeping runs wake up correctly |
| 16 | Logs SSE + audit + metrics | Observability complete |
| 17 | Doctor + README + cross-capability harness | v0 exit checklist passes |

### v0 exit checklist

- [ ] Server boots; migrations apply; doctor green
- [ ] Job CRUD + trigger CRUD + run listing
- [ ] Cron + webhook + event + manual triggers all dispatch runs
- [ ] Steps checkpoint and replay correctly on crash mid-run
- [ ] State persists across steps and across retries
- [ ] `ctx.emit` triggers downstream jobs
- [ ] Retry with exponential backoff lands failed runs in DLQ
- [ ] Concurrency cap returns 429 / queues correctly
- [ ] reactor-cache trait swappable (Postgres backend ships v0)
- [ ] Cross-capability harness: signup → create job → trigger → assert run + steps + state
- [ ] Logs SSE streams from job runs

---

## 17. Decision log

| Question | Decision | Rationale |
|---|---|---|
| **Queue backend** | Postgres via reactor-cache (SKIP LOCKED) | Simple, transactional, no extra infra. Redis in v0.2. |
| **Scheduler model** | In-process tokio tasks | Single-server sufficient for v0; extract to workers v0.2. |
| **SDK language** | TypeScript-first (Bun-native) | Matches reactor-functions primary runtime; Rust SDK v0.2. |
| **Step model** | Checkpointed steps with cached outputs | Enables replay without re-execution; paves way for workflows. |
| **State persistence** | Per-run KV in `_reactor_jobs.state` | Simple, queryable, transactional with run lifecycle. |
| **Event pub-sub** | Internal via `_reactor_jobs.events` table | No external broker at v0; Kafka/SQS integration v0.2. |
| **Concurrency control** | Per-job semaphore in scheduler | Prevents resource exhaustion; matches Lambda reserved concurrency. |
| **DLQ strategy** | Separate table, admin retry endpoint | Standard pattern; keeps runs table clean. |
| **Webhook auth** | Encrypted token containing job/trigger/org | No database lookup on hot path; token is self-contained. |
| **reactor-data integration** | Built-in `ctx.data` client with internal token | Jobs can query/mutate user tables; same auth surface as functions. |

---

## 18. Open questions (deferred)

1. **Workflow primitives**: `ctx.branch()`, `ctx.loop()`, `ctx.parallel()` for v0.2 — data model supports it.
2. **Visual builder**: Depends on workflow primitives; likely a Sites-hosted UI.
3. **Redis backend**: reactor-cache abstraction ready; wire Redis client v0.2.
4. **External queues**: SQS, RabbitMQ, Kafka triggers for enterprise use cases.
5. **Run retention**: How long to keep completed runs; default 30 days proposed.
6. **Cross-job transactions**: Saga pattern or two-phase commit across jobs.
7. **Priority queues**: Some jobs more urgent than others; separate queues or priority field.
8. **Rate limiting**: Per-trigger rate limits beyond concurrency; may belong in gateway.

---

*End of design doc. Land code against checklist §16 in order, one PR per row, this doc updated as decisions change.*
