# `reactor-analytics` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Product analytics capability for the Reactor.cloud BaaS. Adds an ingestion + query surface for user-behavior events (pageviews, custom events, identify, errors), complementing the existing per-capability `audit_events` tables.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, event schema, SDK shape, schema, query plane — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Provide a **product analytics capability** in the BaaS stack: ingest events from anonymous + authenticated users, identify-on-signup, store, query, and dashboard.
2. Reactor is the **first consumer** (marketing site, dashboard, Studio, JS SDK self-telemetry), but the capability is a first-class BaaS surface: any Reactor customer can `reactor.track('signup_completed', { plan: 'pro' })` from their own app.
3. **Storage portability from day one**: `AnalyticsStore` trait shipped at v0 with a `PgAnalyticsStore` implementation. ClickHouse / Timescale adapters are additive at v0.2, mirroring `DataStore` / `BlobStore` patterns.
4. **Two ingestion modes** at v0: anonymous (public project key, top-of-funnel) and authenticated (bearer JWT, trusted server-side enrichment).
5. **Hybrid event schema**: a small set of typed system events (`$pageview`, `$identify`, `$alias`, `$session_start`, `$session_end`, `$autocapture`, `$error`) get first-class columns; user events are free-form `event` + `properties jsonb`.
6. **Privacy-first defaults**: IP truncated to /24, DNT honored by default, opt-in/opt-out API, right-to-erasure endpoint.
7. **Single, expressive read endpoint** over raw events — designed for agents/LLMs to drive: one well-documented `POST /analytics/v1/query` with a JSON request that supports filter, group, time-bucket, funnel, retention. The query plane is the only surface a Studio agent needs to learn.
8. **Reuse the shared `reactor-policy` engine** for per-event-stream authorization (e.g. "marketing can see funnels, support cannot see raw events").
9. **Reuse `AuthClient`** — supports both `InProcessAuthClient` and `RemoteAuthClient` topologies.
10. Be the only crate allowed to touch the `_reactor_analytics.*` Postgres metadata + event schema.

## 2. Non-goals (v0)

- **Session replay** (DOM recording, rrweb) — separate beast; v0.2+ as `reactor-replay` or never.
- **Heatmaps** — derive from autocapture clicks; defer to v0.2.
- **A/B testing / feature flags** — explicitly a different product (Edge-Config-shaped); not in this crate.
- **Funnel / retention / cohort persistence as first-class objects** — v0 computes them on demand via the query endpoint; saved insights are v0.2.
- **Web Vitals / RUM** (LCP, INP, CLS) — captured via `$performance` event in v0.2; not auto-collected by SDK in v0.
- **Server-to-server SDKs in other languages** (Python, Go, Ruby) — TypeScript-first; defer.
- **ClickHouse / Timescale adapters at runtime** — trait shipped, adapters v0.2.
- **CDC mirroring of audit events into analytics** — explicit boundary (see §5.5); revisit v0.2.
- **Cross-org event federation** — events live in exactly one org tenant.
- **Dashboarding UI** — Studio renders it; this crate ships the data plane only.
- **Billing-grade event counting** — ingestion counters feed Prometheus metrics; control-plane billing reads those, this crate does not own pricing.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, IDs, AuthClient trait
├── reactor-policy/                # (existing) shared policy engine
├── reactor-auth/                  # (existing)
├── reactor-data/                  # (existing)
├── reactor-storage/               # (existing)
├── reactor-functions/             # (existing)
├── reactor-jobs/                  # (existing)
├── reactor-sites/                 # (existing)
│
├── reactor-analytics/             # NEW — the analytics library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_analytics.*
│   │   ├── 001_metadata.sql       # projects, project_keys, identities, sessions
│   │   ├── 002_events.sql         # partitioned events table + indexes
│   │   ├── 003_rollups.sql        # daily/hourly rollup tables
│   │   ├── 004_policies.sql       # policy storage (per stream)
│   │   └── 005_erasure.sql        # erasure log (GDPR)
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # AnalyticsConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # AnalyticsState, AnalyticsCtx
│       ├── error.rs               # AnalyticsError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs
│       │   ├── ingest.rs          # POST /track, /batch, /identify, /alias
│       │   ├── query.rs           # POST /query (the single read endpoint)
│       │   ├── admin.rs           # projects + project keys CRUD
│       │   ├── consent.rs         # opt-in / opt-out / status
│       │   └── erasure.rs         # GDPR delete / export
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   ├── auth.rs            # bearer + X-Reactor-Org → AnalyticsCtx (authed)
│       │   ├── project_key.rs     # X-Reactor-Project-Key → AnalyticsCtx (anonymous)
│       │   └── dnt.rs             # honor DNT / Sec-GPC
│       │
│       ├── ingest/
│       │   ├── mod.rs             # ingest pipeline orchestration
│       │   ├── enrich.rs          # IP truncate, UA parse, geo, referrer parse
│       │   ├── identify.rs        # anon ↔ user_id stitching
│       │   ├── validate.rs        # event shape + property type checking
│       │   ├── system_events.rs   # $pageview, $identify, $alias, $session_*, $autocapture, $error
│       │   ├── sampling.rs        # per-key sampling + per-org quota
│       │   └── batch.rs           # batch decompose, error-per-event response
│       │
│       ├── query/
│       │   ├── mod.rs             # QueryRequest type + dispatcher
│       │   ├── ast.rs             # filter / group / time-bucket / funnel / retention DSL
│       │   ├── compile.rs         # QueryRequest → SQL (per-backend)
│       │   ├── ops/
│       │   │   ├── mod.rs
│       │   │   ├── events.rs      # raw event scan
│       │   │   ├── aggregate.rs   # count / unique users / sum / avg, with group_by + time_bucket
│       │   │   ├── funnel.rs      # ordered-step funnel with conversion window
│       │   │   ├── retention.rs   # cohort N-day return rate
│       │   │   ├── breakdown.rs   # top-N by property
│       │   │   └── path.rs        # user journey paths (top sequences)
│       │   └── limits.rs          # per-request row caps + time-range caps
│       │
│       ├── store/
│       │   ├── mod.rs             # AnalyticsStore trait
│       │   └── postgres.rs        # PgAnalyticsStore (sqlx, partitioned tables)
│       │
│       ├── rollup/
│       │   ├── mod.rs             # rollup scheduler (called by reactor-jobs cron)
│       │   └── daily.rs           # daily aggregates
│       │
│       ├── policy.rs              # AnalyticsCtx ↔ reactor-policy bridge
│       └── audit.rs               # admin-action audit writer (project keys, erasures)
│
├── reactor-analytics-server/      # standalone bin (microservices topology)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # axum bind + migrate + serve
│       └── cli/
│           ├── mod.rs
│           └── doctor.rs          # connectivity diagnostics
```

Conventions:
- `reactor-analytics` depends on `reactor-core`, `reactor-policy`. It **never** depends on `reactor-auth`; auth is consumed through the `AuthClient` trait.
- `reactor-jobs` is a *consumer* (for rollup cron); `reactor-analytics` does not depend on it. Rollups can be triggered by any scheduler.
- `reactor-sites` may inject the JS SDK snippet via the site manifest; this is opt-in and not a build-time dependency.

---

## 4. Core types

### 4.1 ID & types reuse

All IDs are `ReactorId` (UUIDv7) from `reactor-core`. Analytics-specific types:

| Type | Rust | Notes |
|---|---|---|
| `ProjectId` | `ReactorId` | A logical app / surface within an org |
| `ProjectKey` | `String` | Public ingestion key, prefix `rapk_` (Reactor Analytics Project Key) |
| `EventId` | `ReactorId` | Per-event UUIDv7 (sortable by time) |
| `AnonymousId` | `String` | Client-assigned, ≤64 chars, no PII, stored in localStorage |
| `SessionId` | `String` | Client-assigned, rotates on 30min inactivity |
| `IdentityId` | `ReactorId` | Server-side stitched identity row |

### 4.2 `AnalyticsCtx` (request-local)

Constructed by middleware once per request:

```rust
// reactor-analytics/src/state.rs
#[derive(Debug, Clone)]
pub struct AnalyticsCtx {
    pub mode:           IngestMode,       // Anonymous { project_key } | Authenticated { auth }
    pub project_id:     ProjectId,        // resolved from key or from X-Reactor-Project
    pub org_id:         OrgId,            // owning org of the project
    pub request_id:     String,
    pub client_ip:      Option<IpAddr>,   // truncated to /24 at enrichment
    pub user_agent:     Option<String>,   // raw, parsed during enrichment
    pub dnt:            bool,             // DNT or Sec-GPC header present
    pub consent:        ConsentState,     // Granted | Denied | Unknown
}

pub enum IngestMode {
    Anonymous { project_key_id: ReactorId },
    Authenticated { auth: AuthCtx },
}

impl AnalyticsCtx {
    pub fn is_anonymous(&self) -> bool { matches!(self.mode, IngestMode::Anonymous { .. }) }
    pub fn user_id(&self) -> Option<&UserId> { /* ... */ }
    pub fn has_permission(&self, perm: &str) -> bool { /* ... */ }
}
```

### 4.3 Canonical event shape

```rust
// reactor-analytics/src/ingest/mod.rs
#[derive(Debug, Clone, Deserialize)]
pub struct IngestEvent {
    /// Required. Event name. Reserved names start with `$`.
    pub event: String,

    /// Required from client; server falls back to one if missing.
    pub anonymous_id: Option<String>,

    /// Optional; set on authenticated ingestion or via $identify.
    pub user_id: Option<String>,

    /// Optional; client rotates every 30min of inactivity.
    pub session_id: Option<String>,

    /// Client-provided event timestamp (ISO-8601). Server clamps to ±1 day of receipt time.
    pub timestamp: Option<DateTime<Utc>>,

    /// User-defined properties.
    pub properties: serde_json::Value,

    /// Client-provided context (page url, referrer, screen, locale, sdk version, ...).
    pub context: ClientContext,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientContext {
    pub page:       Option<PageContext>,    // { url, path, title, referrer, search, hash }
    pub screen:     Option<ScreenContext>,  // { width, height, density }
    pub locale:     Option<String>,         // BCP-47
    pub timezone:   Option<String>,         // IANA
    pub library:    Option<LibraryContext>, // { name, version } — "@reactor/analytics", "0.1.0"
    pub utm:        Option<UtmContext>,     // { source, medium, campaign, term, content }
    // ip / user_agent / geo: server-enriched, ignored from client
}
```

### 4.4 Stored event (server-canonical)

```rust
#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub id:             EventId,
    pub received_at:    DateTime<Utc>,     // server clock, used for partitioning
    pub timestamp:      DateTime<Utc>,     // client-asserted, clamped
    pub org_id:         OrgId,
    pub project_id:     ProjectId,
    pub event:          String,            // 'page_viewed', '$pageview', 'checkout_completed'
    pub anonymous_id:   String,
    pub user_id:        Option<String>,
    pub session_id:     Option<String>,
    pub properties:     serde_json::Value,
    pub context:        serde_json::Value, // canonicalized
    pub url:            Option<String>,    // extracted from context.page (hot column for $pageview)
    pub path:           Option<String>,    // hot column
    pub referrer_host:  Option<String>,    // hot column
    pub utm_source:     Option<String>,    // hot column
    pub country:        Option<String>,    // geo-enriched from IP, 2-letter
    pub device_type:    Option<String>,    // 'desktop' | 'mobile' | 'tablet' | 'bot'
    pub ingest_ip_h24:  Option<String>,    // /24 truncated, never raw
    pub library_name:   Option<String>,
    pub library_version:Option<String>,
}
```

### 4.5 `AnalyticsStore` trait

```rust
// reactor-analytics/src/store/mod.rs
#[async_trait]
pub trait AnalyticsStore: Send + Sync + 'static {
    /// Project & key management
    async fn create_project(&self, org_id: &OrgId, p: ProjectCreate) -> Result<Project, AnalyticsError>;
    async fn get_project(&self, project_id: &ProjectId) -> Result<Option<Project>, AnalyticsError>;
    async fn list_projects(&self, org_id: &OrgId) -> Result<Vec<Project>, AnalyticsError>;
    async fn delete_project(&self, project_id: &ProjectId) -> Result<(), AnalyticsError>;

    async fn create_project_key(&self, project_id: &ProjectId, k: ProjectKeyCreate) -> Result<ProjectKey, AnalyticsError>;
    async fn lookup_project_key(&self, key: &str) -> Result<Option<ProjectKeyRecord>, AnalyticsError>;
    async fn revoke_project_key(&self, key_id: &ReactorId) -> Result<(), AnalyticsError>;

    /// Ingestion (bulk, one transaction per batch)
    async fn write_events(&self, events: &[StoredEvent]) -> Result<WriteOutcome, AnalyticsError>;

    /// Identity stitching
    async fn upsert_identity(&self, org_id: &OrgId, project_id: &ProjectId,
                             anonymous_id: &str, user_id: &str,
                             traits: &serde_json::Value) -> Result<(), AnalyticsError>;
    async fn alias(&self, org_id: &OrgId, project_id: &ProjectId,
                   from_anonymous_id: &str, to_user_id: &str) -> Result<(), AnalyticsError>;

    /// Query (single dispatch — see §8)
    async fn query(&self, ctx: &AnalyticsCtx, q: &QueryRequest) -> Result<QueryResult, AnalyticsError>;

    /// GDPR
    async fn erase_user(&self, project_id: &ProjectId, user_id: &str) -> Result<EraseOutcome, AnalyticsError>;
    async fn erase_anonymous(&self, project_id: &ProjectId, anonymous_id: &str) -> Result<EraseOutcome, AnalyticsError>;
    async fn export_user(&self, project_id: &ProjectId, user_id: &str) -> Result<Vec<StoredEvent>, AnalyticsError>;

    /// Quota
    async fn current_month_event_count(&self, org_id: &OrgId) -> Result<u64, AnalyticsError>;
}
```

A batch insert returning a `WriteOutcome { accepted: usize, rejected: Vec<RejectReason> }` is the unit that supports the per-event error response (see §5.2.2).

---

## 5. HTTP surface (v0)

Base path: `/analytics/v1`. Standard Reactor envelope (`{ data, error }`).

### 5.1 Health

`GET /analytics/v1/health` → `{ status: 'ok' | 'degraded', store: 'ok', queue: 'ok' }`

### 5.2 Ingestion

#### 5.2.1 `POST /analytics/v1/track` — single event (anonymous or authenticated)

Headers:
- `X-Reactor-Project-Key: rapk_…` **(anonymous mode)** OR
- `Authorization: Bearer <jwt>` + `X-Reactor-Project: <project_id>` **(authenticated mode)**
- `Content-Type: application/json`

Body:
```json
{
  "event": "checkout_started",
  "anonymous_id": "anon_01HF…",
  "user_id": "u_42",
  "session_id": "sess_01HF…",
  "timestamp": "2026-05-22T14:01:33.221Z",
  "properties": { "cart_value": 49.0, "currency": "EUR" },
  "context": { "page": { "url": "https://x.com/checkout", "path": "/checkout" } }
}
```

Response: `204 No Content` on accept, `202 Accepted` if queued, `400` on validation failure, `429` on rate limit / quota exceeded.

Latency target: p99 < 20 ms (insert is non-blocking on Postgres write; see §6.2).

#### 5.2.2 `POST /analytics/v1/batch` — batch ingestion

Body: `{ "events": [ IngestEvent, IngestEvent, … ] }`. Max 100 events per batch, max 1 MiB body.

Response: per-event status array, so a single bad event doesn't fail the batch:
```json
{ "data": { "accepted": 98, "rejected": [
    { "index": 17, "reason": "unknown_event_name_starts_with_dollar" },
    { "index": 42, "reason": "properties_too_large", "limit_bytes": 32768 }
  ]}}
```

#### 5.2.3 `POST /analytics/v1/identify` — explicit identity assert

Body: `{ "anonymous_id": "anon_…", "user_id": "u_42", "traits": { "email": "x@y.com", "plan": "pro" } }`

Writes a `$identify` event + upserts the identity row. Subsequent events with the same `anonymous_id` get the `user_id` attached server-side (best-effort; client should also send `user_id` directly when known).

#### 5.2.4 `POST /analytics/v1/alias` — merge an anonymous id into a user id

Body: `{ "from": "anon_…", "to": "u_42" }`. Writes `$alias` event + updates identity table. Idempotent.

#### 5.2.5 `POST /analytics/v1/erase` — GDPR right-to-erasure (authenticated)

Body: `{ "user_id": "u_42" }` or `{ "anonymous_id": "anon_…" }`. Deletes all events for that subject in the project. Writes an entry in `_reactor_analytics.erasures` for audit.

### 5.3 Query — the single agent-friendly endpoint

`POST /analytics/v1/query` (authenticated)

This is **the** read surface. One endpoint, one JSON schema, many shapes — designed for an LLM to learn once and use everywhere. See §8 for the full grammar.

Request:
```json
{
  "project_id": "p_…",
  "kind": "aggregate",           // 'events' | 'aggregate' | 'funnel' | 'retention' | 'breakdown' | 'path'
  "time_range": { "from": "2026-05-01T00:00:00Z", "to": "2026-05-22T00:00:00Z" },
  "filter": { "all": [ { "event": { "eq": "checkout_completed" } },
                       { "prop": { "currency": { "eq": "EUR" } } } ] },
  "group_by": [ { "prop": "plan" } ],
  "time_bucket": "1d",
  "measure": "unique_users",     // 'count' | 'unique_users' | 'unique_sessions' | 'sum:<prop>' | 'avg:<prop>' | 'p50/p95/p99:<prop>'
  "limit": 100
}
```

Funnel example:
```json
{
  "project_id": "p_…",
  "kind": "funnel",
  "time_range": { "last": "30d" },
  "steps": [
    { "event": "page_viewed", "filter": { "prop": { "path": { "eq": "/" } } } },
    { "event": "signup_started" },
    { "event": "signup_completed" },
    { "event": "first_deploy" }
  ],
  "conversion_window": "7d",
  "group_by": [ { "prop": "utm_source" } ]
}
```

Response:
```json
{ "data": {
    "kind": "funnel",
    "rows": [
      { "group": { "utm_source": "twitter" }, "steps": [12000, 4200, 3100, 2400], "conversion": 0.20 },
      { "group": { "utm_source": "hn"      }, "steps": [ 8000, 3900, 3700, 3200], "conversion": 0.40 }
    ],
    "execution_ms": 412,
    "rows_scanned": 184320
  }}
```

### 5.4 Projects & keys

| Verb | Path | Permission |
|---|---|---|
| `POST` | `/analytics/v1/projects` | `analytics:project:create` |
| `GET` | `/analytics/v1/projects` | `analytics:project:read` |
| `DELETE` | `/analytics/v1/projects/{id}` | `analytics:project:delete` |
| `POST` | `/analytics/v1/projects/{id}/keys` | `analytics:project:admin` |
| `GET` | `/analytics/v1/projects/{id}/keys` | `analytics:project:admin` |
| `DELETE` | `/analytics/v1/projects/{id}/keys/{key_id}` | `analytics:project:admin` |

### 5.5 Consent

| Verb | Path | Notes |
|---|---|---|
| `POST` | `/analytics/v1/consent/opt-out` | Body: `{ anonymous_id, scope }` — server tombstones the anon_id; future events are dropped. |
| `POST` | `/analytics/v1/consent/opt-in` | Removes the tombstone. |
| `GET` | `/analytics/v1/consent/status?anonymous_id=…` | Returns current consent state. |

### 5.6 Headers (cross-cutting)

| Header | Purpose |
|---|---|
| `X-Reactor-Project-Key` | Anonymous ingestion key (write-only). |
| `X-Reactor-Project` | Project ID (authenticated ingestion + all queries). |
| `Authorization: Bearer <jwt>` | Authenticated mode. |
| `X-Reactor-Client: js/<version>` | Library attribution (existing convention). |
| `DNT: 1` / `Sec-GPC: 1` | If present and DNT-honor enabled, event is dropped with `204` (no leak that we dropped it). |

### 5.7 Error envelope

Reuses the standard Reactor error shape. Ingestion-specific codes:

| Code | HTTP | Meaning |
|---|---|---|
| `analytics.project_key.invalid` | 401 | Key unknown / revoked |
| `analytics.quota.exceeded` | 429 | Monthly event quota exhausted for the org |
| `analytics.rate_limit` | 429 | Per-key burst limit exceeded |
| `analytics.event.too_large` | 413 | Properties + context > 32 KiB |
| `analytics.event.system_reserved` | 400 | Custom event name starts with `$` |
| `analytics.batch.too_large` | 413 | > 100 events or > 1 MiB |
| `analytics.consent.denied` | 204 | Opt-out / DNT — silent drop, body empty |
| `analytics.query.timeout` | 504 | Query exceeded `REACTOR_ANALYTICS_QUERY_TIMEOUT_MS` |
| `analytics.query.range_too_wide` | 400 | Time range > 1y on raw events (rollups required) |

---

## 6. Database schema (`_reactor_analytics`)

### 6.1 Projects, keys, identities

```sql
create schema if not exists _reactor_analytics;

create table _reactor_analytics.projects (
  id           uuid primary key,
  org_id       uuid not null,
  name         text not null,
  created_at   timestamptz not null default now(),
  deleted_at   timestamptz,
  unique (org_id, name)
);
create index on _reactor_analytics.projects (org_id);

create table _reactor_analytics.project_keys (
  id              uuid primary key,
  project_id      uuid not null references _reactor_analytics.projects(id) on delete cascade,
  key_prefix      text not null,           -- 'rapk_'
  key_hash        bytea not null,          -- argon2id hash of the full key
  key_last4       text not null,           -- for UI display
  name            text not null,           -- 'web-prod', 'web-staging'
  sampling_rate   double precision not null default 1.0 check (sampling_rate between 0 and 1),
  allowed_origins text[],                  -- nullable = no CORS check
  created_at      timestamptz not null default now(),
  revoked_at      timestamptz
);
create unique index on _reactor_analytics.project_keys (key_hash);
create index on _reactor_analytics.project_keys (project_id);

create table _reactor_analytics.identities (
  org_id          uuid not null,
  project_id      uuid not null,
  anonymous_id    text not null,
  user_id         text,                    -- nullable until identified
  first_seen_at   timestamptz not null default now(),
  last_seen_at    timestamptz not null default now(),
  traits          jsonb not null default '{}'::jsonb,
  primary key (project_id, anonymous_id)
);
create index on _reactor_analytics.identities (project_id, user_id) where user_id is not null;

create table _reactor_analytics.consent_tombstones (
  project_id      uuid not null,
  anonymous_id    text not null,
  reason          text not null,           -- 'opt_out' | 'dnt' | 'erased'
  created_at      timestamptz not null default now(),
  primary key (project_id, anonymous_id)
);
```

### 6.2 Events — partitioned

```sql
create table _reactor_analytics.events (
  id              uuid not null,
  received_at     timestamptz not null,
  timestamp       timestamptz not null,
  org_id          uuid not null,
  project_id      uuid not null,
  event           text not null,
  anonymous_id    text not null,
  user_id         text,
  session_id      text,
  url             text,
  path            text,
  referrer_host   text,
  utm_source      text,
  country         text,
  device_type     text,
  ingest_ip_h24   text,
  library_name    text,
  library_version text,
  properties      jsonb not null default '{}'::jsonb,
  context         jsonb not null default '{}'::jsonb,
  primary key (received_at, id)
) partition by range (received_at);

-- v0 ships monthly partitions; reactor-jobs cron creates next month's partition
-- (see §10). Default-partition catches mis-clocked clients.
create table _reactor_analytics.events_default partition of _reactor_analytics.events default;

create index on _reactor_analytics.events using brin (received_at);
create index on _reactor_analytics.events (project_id, received_at desc);
create index on _reactor_analytics.events (project_id, event, received_at desc);
create index on _reactor_analytics.events (project_id, user_id, received_at desc)
  where user_id is not null;
create index on _reactor_analytics.events (project_id, anonymous_id, received_at desc);
```

**Hot columns rationale**: `url`, `path`, `referrer_host`, `utm_source`, `country`, `device_type` cover the 80% of queries; everything else stays in `properties`/`context` jsonb. Adding a hot column is an additive migration.

**Write path**: ingest writes are non-blocking from the request handler — handler validates + enriches, pushes onto an in-process channel; a background batcher flushes every 200 ms or 500 events with `COPY ... FROM STDIN` for throughput. On batcher backpressure, handler returns `202` instead of `204`. (See §10.)

### 6.3 Rollups (v0.2 deferred)

> **Deferred to v0.2**: Rollups are not implemented in v0. The query plane scans raw events only, with `QUERY_RAW_RANGE_DAYS` (default 90d) cap.

```sql
create table _reactor_analytics.rollups_daily (
  project_id      uuid not null,
  day             date not null,
  event           text not null,
  group_keys      jsonb not null,          -- e.g. { "utm_source": "twitter" } or {} for no group
  count           bigint not null,
  unique_users    bigint not null,
  unique_sessions bigint not null,
  primary key (project_id, day, event, group_keys)
);
create index on _reactor_analytics.rollups_daily (project_id, event, day desc);
```

Populated by a `reactor-jobs` cron job that calls `POST /analytics/v1/internal/rollup?day=YYYY-MM-DD` (secret-gated). Query planner (§8) prefers rollups for any query whose `time_bucket >= 1d` and whose `group_by` is fully covered by `group_keys`.

### 6.4 Erasures (GDPR audit)

```sql
create table _reactor_analytics.erasures (
  id              uuid primary key,
  project_id      uuid not null,
  subject_kind    text not null,           -- 'user' | 'anonymous'
  subject_id      text not null,
  rows_deleted    bigint not null,
  actor_user_id   uuid,
  request_id      text not null,
  created_at      timestamptz not null default now()
);
create index on _reactor_analytics.erasures (project_id, created_at desc);
```

### 6.5 Audit events (admin actions)

Project create/delete, key issue/revoke, manual erasure, opt-out. Same shape as other capabilities.

```sql
create table _reactor_analytics.audit_events (
  id              uuid primary key,
  ts              timestamptz not null default now(),
  actor_user_id   uuid,
  actor_apikey_id uuid,
  org_id          uuid,
  project_id      uuid,
  request_id      text not null,
  event_type      text not null,           -- 'project.create' | 'key.issue' | 'key.revoke' | 'erasure.perform' | ...
  details         jsonb not null default '{}'::jsonb
);
create index on _reactor_analytics.audit_events (org_id, ts desc);
create index on _reactor_analytics.audit_events (project_id, ts desc);
```

### 6.6 Role grants

The `reactor_analytics_app` role:
- `USAGE` on `_reactor_analytics` schema.
- `SELECT, INSERT, UPDATE, DELETE` on all tables in the schema (UPDATE limited to identities + project_keys revocation; events are insert-only at the app layer).
- `EXECUTE` on partition-management helper functions.
- Membership in a least-privilege role; no superuser.

### 6.7 Why **not** unify with `_reactor_*.audit_events`

Audit and analytics have orthogonal requirements:

| Property | Audit | Analytics |
|---|---|---|
| Transactional with mutation | Required | Forbidden (would slow ingestion) |
| Volume | Low | High |
| Schema | Closed, typed | Open jsonb |
| Retention | Long (compliance) | Short by default, configurable |
| Access | Org admins | Org analysts |
| Loss tolerance | Zero | Sampling acceptable |

A future v0.2 may surface a unified read view (`v_org_activity`) joining both. The physical separation stays.

---

## 7. Privacy & compliance

### 7.1 IP handling

- Raw IP is **never persisted**. Middleware extracts it once for geo lookup, then truncates to /24 (IPv4) or /48 (IPv6) before it reaches the store. `ingest_ip_h24` column holds the truncated value.
- Geo enrichment uses MaxMind GeoLite2 country DB (bundled monthly via build script; configurable via `REACTOR_ANALYTICS_GEO_DB`).

### 7.2 DNT / Sec-GPC

- Default: **honored**. If `DNT: 1` or `Sec-GPC: 1` is present and `REACTOR_ANALYTICS_HONOR_DNT=1` (default), the event is dropped silently with `204`.
- Customers may opt their projects out of DNT honoring via `analytics:project:admin` action (`POST /projects/{id} { honor_dnt: false }`).

### 7.3 Consent

- The JS SDK exposes `analytics.optIn()` / `analytics.optOut()` (see §9). Opt-out writes a server tombstone in `consent_tombstones`; future events for that `anonymous_id` are dropped server-side regardless of client behavior.
- Tombstones persist across browser sessions because they're keyed on `anonymous_id` which lives in localStorage with a 1-year cookie fallback.

### 7.4 Right to erasure

- `POST /analytics/v1/erase` deletes all event rows for the subject across all partitions, plus the identity row, plus issues a tombstone. The operation is logged in `erasures` and `audit_events`.
- Subject can be specified by `user_id` or `anonymous_id`.
- For Cloud customers, the dashboard exposes this as a per-user "Forget" button.

### 7.5 Property allowlist / blocklist

- Per-project config: `denied_properties: ["email", "password", "ssn", "credit_card"]`. Ingestion drops these keys before write.
- Default blocklist enforced for `$autocapture` events (form values are never captured).

---

## 8. Query plane

One endpoint, one JSON request shape, many `kind`s. Designed so that an LLM can produce valid queries from a schema dump alone.

### 8.1 `QueryRequest` grammar

```rust
pub struct QueryRequest {
    pub project_id:   ProjectId,
    pub kind:         QueryKind,
    pub time_range:   TimeRange,           // { from, to } or { last: "30d" }
    pub filter:       Option<FilterExpr>,
    pub group_by:     Vec<GroupKey>,
    pub time_bucket:  Option<TimeBucket>,  // '1m' | '5m' | '1h' | '1d' | '1w' | '1mo'
    pub measure:      Option<Measure>,     // aggregate kinds only
    pub steps:        Option<Vec<FunnelStep>>,  // funnel kind only
    pub conversion_window: Option<Duration>,    // funnel kind only
    pub cohort:       Option<CohortDef>,        // retention kind only
    pub return_event: Option<String>,            // retention kind only
    pub limit:        Option<u32>,
    pub order_by:     Option<OrderSpec>,
}

pub enum QueryKind {
    Events,         // raw event rows (for debugging; capped to 1000)
    Aggregate,      // count/unique/sum/avg over filter, with optional group_by + time_bucket
    Funnel,         // ordered-step conversion analysis
    Retention,      // N-period return rate by cohort
    Breakdown,      // top-N by property
    Path,           // top sequences of events per user
}

pub enum FilterExpr {
    All(Vec<FilterExpr>),       // AND
    Any(Vec<FilterExpr>),       // OR
    Not(Box<FilterExpr>),
    Event { op: StringOp, value: serde_json::Value },
    Prop  { name: String, op: ValueOp, value: serde_json::Value },
    User  { op: StringOp, value: serde_json::Value },
    Anon  { op: StringOp, value: serde_json::Value },
    Time  { op: TimeOp, value: serde_json::Value },
}
```

### 8.2 Compiler

`query::compile` turns `QueryRequest` into a backend-specific SQL statement. For `PgAnalyticsStore`:

- Filters become parameterized WHERE clauses.
- `group_by: [{ prop: "utm_source" }]` becomes `GROUP BY context->>'utm_source'` (or hot column if available).
- `time_bucket: "1d"` becomes `date_trunc('day', timestamp)`.
- **(v0.2 deferred)** The planner consults `rollups_daily` first: if `time_bucket >= 1d` AND `group_by` is fully covered by stored `group_keys` AND `filter` reduces to `event = ? AND project_id = ?` AND the time range falls inside the rollup's coverage, the query is rewritten to scan rollups. In v0, all queries scan raw events only.

### 8.3 Limits (DoS protection)

Configurable via env:

| Env | Default | Meaning |
|---|---|---|
| `REACTOR_ANALYTICS_QUERY_TIMEOUT_MS` | `30000` | Statement timeout per query |
| `REACTOR_ANALYTICS_QUERY_MAX_ROWS` | `100000` | Hard cap on rows scanned (statement-level) |
| `REACTOR_ANALYTICS_QUERY_RAW_RANGE_DAYS` | `90` | `kind: 'events'` rejects ranges wider than this |
| `REACTOR_ANALYTICS_QUERY_AGG_RANGE_DAYS` | `730` | Aggregate / funnel / retention range cap |

### 8.4 Why one endpoint instead of `/funnels`, `/retention`, …

- One docs page. One JSON Schema. One MCP tool descriptor. Studio's agent learns it once and uses it for everything.
- New `kind`s are additive: ship `path` in v0.2 without changing the URL surface.
- Saved insights (v0.2) become "named QueryRequests"; the executor stays unchanged.

---

## 9. JavaScript SDK surface (`@reactor/analytics`)

Lives in the existing `sdks/js/packages/` monorepo as `@reactor/analytics`, re-exported through `@reactor/client` as `reactor.analytics`.

### 9.1 Initialization

```ts
import { createClient } from '@reactor/client';

const reactor = createClient({
  url: 'https://api.reactor.cloud',
  key: 'rapk_…',                       // project public key
  analytics: {
    enabled: true,                     // default true if key present
    autoPageview: true,                // default true
    autoIdentify: true,                // default true; ties into auth.onAuthStateChange
    autoCapture: false,                // default false (clicks, form submits)
    autoErrors: true,                  // default true; window.onerror + unhandledrejection
    sessionTimeoutMs: 30 * 60 * 1000,  // 30min
    flushIntervalMs: 5000,
    flushAt: 20,                       // batch size
    transport: 'auto',                 // 'beacon' | 'fetch' | 'auto' (beacon on pagehide, fetch otherwise)
    persistence: 'localStorage',       // 'localStorage' | 'cookie' | 'memory'
    honorDNT: true,
    deniedProperties: ['email', 'password'],
  },
});

reactor.analytics.track('checkout_started', { cart_value: 49 });
reactor.analytics.identify('u_42', { email: 'x@y.com', plan: 'pro' });
reactor.analytics.alias('u_42');                      // merge current anon → user
reactor.analytics.page('/checkout', { title: 'Checkout' });   // manual pageview
reactor.analytics.optOut();
reactor.analytics.optIn();
reactor.analytics.reset();                            // sign-out: new anon_id, clear identity
reactor.analytics.flush();                            // force flush queue
```

### 9.2 Auto-identify integration

When `autoIdentify: true`, the SDK subscribes to `reactor.auth.onAuthStateChange`:

| Auth event | Analytics action |
|---|---|
| `SIGNED_IN` | `identify(user_id, traits)` + `alias(anonymous_id → user_id)` |
| `SIGNED_OUT` | `reset()` — generates new anonymous_id, clears stored user_id |
| `TOKEN_REFRESHED` | no-op (analytics doesn't need fresh tokens for anonymous mode) |
| `USER_UPDATED` | `identify(user_id, updatedTraits)` |

### 9.3 Transport & batching

- **Queue** in memory; flush triggers: every `flushIntervalMs`, or when queue size hits `flushAt`, or on `pagehide`/`visibilitychange: hidden`.
- **Beacon on unload** via `navigator.sendBeacon` (cap 64 KiB; events that exceed split into multiple beacons).
- **Fetch otherwise** with keepalive: true.
- **Retry**: exponential backoff with jitter, capped at 3 attempts; on persistent failure events are dropped and a single `$ingest_failure` event is queued for the next successful flush (no infinite buffer).
- **Compression**: `Content-Encoding: gzip` for batches > 4 KiB.

### 9.4 Auto-pageview

- Initial pageview on `DOMContentLoaded`.
- SPA navigation: hooks `history.pushState` / `replaceState` and listens to `popstate`. Fires `$pageview` with `{ url, path, referrer, title }`.
- Debounced (100 ms) to avoid double-firing on rapid route changes.

### 9.5 Auto-error capture

When `autoErrors: true`:

```ts
window.addEventListener('error', (e) => track('$error', {
  message: e.message, filename: e.filename, lineno: e.lineno, colno: e.colno,
  stack: e.error?.stack?.slice(0, 4096),
}));
window.addEventListener('unhandledrejection', (e) => track('$error', {
  message: String(e.reason), kind: 'unhandledrejection',
  stack: e.reason?.stack?.slice(0, 4096),
}));
```

- Stack traces truncated to 4 KiB.
- A simple in-SDK fingerprint (`hash(message + first stack frame)`) prevents flooding: same error within 60 s is coalesced into a `count` property bump rather than re-sent.
- React error boundaries: ship a helper `reactor.analytics.captureError(err, { componentStack })` for explicit reporting from frameworks. (No automatic React/Vue/Svelte instrumentation in v0.)

### 9.6 Auto-capture (opt-in)

When `autoCapture: true`, the SDK attaches a single delegated listener to `document` for `click` and `submit`. Captured properties: `tag`, `id`, `classes`, `text` (truncated to 256 chars, **never** form input values), `href` (for anchors), `data-*` attributes (allowlist via config).

### 9.7 Anonymous ID

- Format: `anon_` + base32(UUIDv7).
- Storage: `localStorage` primary; on unavailable (Safari ITP, incognito), cookie fallback (`reactor_anon`, max-age 1y, SameSite=Lax, Secure).
- Memory-only mode (`persistence: 'memory'`) for ephemeral environments.

### 9.8 Server-side use

`@reactor/analytics` works in Node 20+, Bun, Deno, edge runtimes too. In server contexts:
- Always provide `anonymous_id` and `user_id` explicitly (no localStorage).
- `auto*` flags default `false`.
- Use the authenticated endpoint with a service-role JWT or a server-side project key.

---

## 10. Server-side ingestion from Reactor runtimes

Reactor's own runtimes get a first-class `ctx.analytics` so user code on Reactor can emit events without an HTTP roundtrip.

### 10.1 Functions / Jobs / Sites

```ts
// Inside a function or job
export default async function (req, ctx) {
  await ctx.analytics.track('order_placed', {
    order_id: order.id,
    value: order.total,
  });
  // user_id, org_id, project_id, session_id are auto-filled from ctx
}
```

Implementation:
- The Bun/wasm runtime injects a thin client backed by an in-process channel into the same batcher as the HTTP ingest path.
- For `lambda` runtime, falls back to the authenticated HTTP endpoint with the function's service token.
- Events emitted server-side get `library: { name: '@reactor/runtime', version }` and a special `context.server: true` flag.

### 10.2 Sites injection (optional)

A site manifest may opt-in to having the JS SDK snippet auto-injected:

```json
{
  "analytics": {
    "enabled": true,
    "project_key": "rapk_…",
    "auto_pageview": true,
    "auto_errors": true
  }
}
```

`reactor-sites` rewrites the served HTML to inject `<script src="https://api.reactor.cloud/analytics/v1/snippet.js" data-key="…"></script>` before `</head>`. This is the only `reactor-sites` ↔ `reactor-analytics` runtime touchpoint (and it's HTTP, no Cargo dep).

### 10.3 Rollup scheduler (v0.2 deferred)

> **Deferred to v0.2**: Rollup scheduler is not implemented in v0. Partition management is handled manually or via external scheduler.

A `reactor-jobs` cron job (defined in this crate's `migrations/006_rollup_job.sql` as a system job) runs daily at 00:30 UTC and calls `POST /analytics/v1/internal/rollup?day=YYYY-MM-DD` (gated by `X-Reactor-Internal-Secret`). The handler computes the daily rollup for every project and inserts into `rollups_daily`.

A second cron job at 00:00 UTC creates the next month's partition (`events_2026_06`) and detaches partitions older than the per-project retention.

---

## 11. Auth & policy integration

### 11.1 Middleware

Two middleware chains, selected by the route:

| Route prefix | Middleware |
|---|---|
| `POST /analytics/v1/track`, `/batch`, `/identify`, `/alias` | `project_key.rs` OR `auth.rs` (whichever header is present; reject if both missing) |
| `POST /analytics/v1/query`, `/erase`, all `/projects/*` | `auth.rs` (bearer JWT required) |
| `POST /analytics/v1/consent/*` | `project_key.rs` |
| `POST /analytics/v1/internal/rollup` | shared-secret header |

### 11.2 Permission scheme

| Permission | Scope |
|---|---|
| `analytics:project:create` | Create projects in the org |
| `analytics:project:read` | List/get projects |
| `analytics:project:admin` | Update project config, manage keys |
| `analytics:project:delete` | Delete a project |
| `analytics:{project_id}:ingest` | Authenticated server-side ingest |
| `analytics:{project_id}:query` | Run queries against a project |
| `analytics:{project_id}:erase` | GDPR erasure |
| `analytics:*:*` | Full analytics access |

### 11.3 Policy engine integration (v0)

`reactor-policy` is reused for per-query authorization. v0 ships one builtin namespace `analytics.*`:

- `analytics.project_id` — the project being queried
- `analytics.kind` — the `QueryKind`
- `analytics.event` — for `Events` kind, the requested event name(s)
- `auth.user_id`, `auth.has_permission(...)` — from the shared builtin set

Example policy stored in `_reactor_analytics.policies`:

```
policy analyst_read on analytics
  for query
  using (auth.has_permission('analytics:' || analytics.project_id || ':query')
         and analytics.kind != 'events')
```

This lets an org grant "analysts can run aggregates but cannot dump raw event rows."

### 11.4 Topology wiring

Same `Arc<dyn AuthClient>` selection as other capabilities — `InProcessAuthClient` for monolith, `RemoteAuthClient` for microservices.

---

## 12. Configuration

`reactor-analytics-server` reads from env (12-factor).

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_ANALYTICS_DATABASE_URL` | yes | — | Postgres connection string |
| `REACTOR_ANALYTICS_BIND` | no | `0.0.0.0:8006` | HTTP bind address |
| `REACTOR_ANALYTICS_BATCH_INTERVAL_MS` | no | `200` | Background batcher flush interval |
| `REACTOR_ANALYTICS_BATCH_MAX_ROWS` | no | `500` | Max rows per COPY batch |
| `REACTOR_ANALYTICS_BATCH_QUEUE_DEPTH` | no | `50000` | In-process channel capacity |
| `REACTOR_ANALYTICS_RETENTION_DAYS_DEFAULT` | no | `90` | Default per-project retention |
| `REACTOR_ANALYTICS_QUOTA_PER_ORG_MONTHLY` | no | `1000000` | Free-tier event cap |
| `REACTOR_ANALYTICS_QUERY_TIMEOUT_MS` | no | `30000` | Per-query statement timeout |
| `REACTOR_ANALYTICS_QUERY_MAX_ROWS` | no | `100000` | Per-query row scan cap |
| `REACTOR_ANALYTICS_HONOR_DNT` | no | `1` | Drop events when DNT/Sec-GPC present |
| `REACTOR_ANALYTICS_GEO_DB` | no | bundled | Path to MaxMind GeoLite2 country DB |
| `REACTOR_ANALYTICS_MAX_PROPERTIES_BYTES` | no | `32768` | Per-event properties+context cap |
| `REACTOR_ANALYTICS_INTERNAL_SECRET` | yes | — | For `/internal/rollup` |
| `REACTOR_ANALYTICS_DEPLOYMENT` | no | `monolith` | `monolith` or `microservices` |
| `REACTOR_ANALYTICS_AUTH_URL` | yes (microservices) | — | URL of reactor-auth-server |
| `REACTOR_ANALYTICS_AUTH_DATABASE_URL` | yes (monolith) | — | Postgres URL for auth schema |
| `REACTOR_ANALYTICS_AUTH_DATA_KEY` | yes (monolith) | — | Column encryption key for auth |
| `REACTOR_ANALYTICS_METRICS` | no | `0` | Set to `1` for Prometheus `/metrics` |
| `REACTOR_LOG` | no | `info` | Tracing filter |

---

## 13. Tracing, metrics, audit

- **Tracing**: `tracing` + JSON subscriber; every request has a `request_id` span; ingestion fields include `project_id`, `event`, `accepted`, `rejected`, `enrich_ms`, `enqueue_ms`. Query fields include `kind`, `time_range`, `rows_scanned`, `rolled_up`, `execution_ms`.
- **Metrics** (Prometheus, `REACTOR_ANALYTICS_METRICS=1`):
  - `analytics_events_ingested_total{project_id, source}` — `source` ∈ `{anonymous, authenticated, server}`
  - `analytics_events_dropped_total{project_id, reason}` — `reason` ∈ `{dnt, opt_out, quota, rate_limit, validation}`
  - `analytics_ingest_latency_seconds{phase}` — `phase` ∈ `{enrich, enqueue, flush}`
  - `analytics_batch_size{store}` (histogram) — rows per COPY
  - `analytics_queue_depth{store}` (gauge)
  - `analytics_query_duration_seconds{kind, rolled_up}`
  - `analytics_query_rows_scanned{kind}` (histogram)
  - `analytics_org_monthly_events{org_id}` (gauge, for billing reads)
- **Audit**: every admin mutation (project, key, erasure, consent override) writes to `_reactor_analytics.audit_events` in the same transaction.

---

## 14. Test surface

- **Unit**: filter expression evaluation, query AST → SQL, IP truncation, UA parsing, identity stitching, rollup planner choice, beacon/fetch transport selection (SDK).
- **Integration**: `testcontainers` Postgres; ingest 100k events and verify partitions, indexes, rollup parity vs raw scan.
- **Conformance**: `tests/store_conformance.rs` runs the same scenarios against any `AnalyticsStore` impl (so a future ClickHouse adapter can drop in).
- **Cross-capability**: `tests/auth_integration.rs` runs `{InProcessAuthClient, RemoteAuthClient}` against the full ingest + query matrix.
- **SDK**: Playwright tests for autoPageview on SPA navigation, autoIdentify on signin/signout, beacon-on-unload, DNT honored, multi-tab session isolation, retry on 500.
- **Privacy regression**: contract tests asserting raw IP never appears in any row, that opt-out tombstones suppress subsequent events server-side, that `erase` removes all matching rows across partitions.
- **Query fuzzing**: property-based tests generate arbitrary `QueryRequest` JSON and assert the compiler produces valid SQL or a clean `400` (never a 500).

---

## 15. Cargo workspace additions

```toml
# Cargo.toml (workspace)
members = [
  # ... existing ...
  "crates/reactor-analytics",
  "crates/reactor-analytics-server",
]
```

`crates/reactor-analytics/Cargo.toml` direct deps (workspace-pinned):
- `reactor-core`, `reactor-policy`
- `axum`, `tower`, `tower-http`
- `sqlx` (postgres, runtime-tokio, macros)
- `tokio`, `tokio-util`
- `serde`, `serde_json`
- `tracing`, `tracing-subscriber`
- `uuid`, `chrono`
- `flate2` (gzip request bodies)
- `maxminddb` (geo lookup)
- `woothee` or `uap-rs` (UA parsing)
- `url`

`crates/reactor-analytics-server/Cargo.toml`:
- `reactor-analytics`, `reactor-auth` (only behind `monolith` feature)
- `clap`, `tracing-subscriber`, `tokio`

---

## 16. Integration into `reactor-server` (monolith)

The umbrella `reactor-server` binary mounts the analytics router at `/analytics/v1` alongside auth, data, storage, functions, jobs, sites. Single Postgres pool can be shared (analytics tables live in their own schema). Migrations run in dependency order: core schemas → analytics last.

---

## 17. JS SDK package boundaries

Update to §2 of `docs/reactor-js-sdk_design.md`:

```
sdks/js/packages/
├── reactor-js/
├── auth-js/
├── data-js/
├── storage-js/
├── functions-js/
├── jobs-js/
├── sites-js/
├── analytics-js/                   # NEW — @reactor/analytics
├── shared/
└── realtime-js/
```

| Package | Purpose | Depends on |
|---|---|---|
| `@reactor/analytics` | track / identify / alias / page / capture-errors / consent / query (admin) | `shared`, optionally `auth` for autoIdentify wiring |

`@reactor/client` composes it as `reactor.analytics`.

---

## 18. Implementation plan (exit checklist)

| # | Deliverable | Test |
|---|---|---|
| 1 | Crate skeleton, config, error, state, `AnalyticsStore` trait, `PgAnalyticsStore` skeleton | `cargo check` |
| 2 | Migrations 001 (metadata) + 002 (events partitioned) + role grants | `sqlx migrate run` against testcontainers Postgres |
| 3 | Project + key CRUD routes + audit writer | integration: create project, issue key, list, revoke |
| 4 | Anonymous ingest middleware (`X-Reactor-Project-Key`) + auth ingest middleware | integration: both modes accept events |
| 5 | Enrichment (IP truncate, UA parse, geo, referrer) + validation | unit + golden tests |
| 6 | Background batcher (channel + COPY flusher) + `/track` + `/batch` | integration: 10k events ingested under target latency |
| 7 | `$identify`, `$alias`, identity stitching | integration: anon → user merge applied to subsequent events |
| 8 | System events: `$pageview`, `$session_start/end`, `$autocapture`, `$error` | unit: each writes correct hot columns |
| 9 | DNT honoring + consent endpoints + tombstones | integration: opt-out drops subsequent events |
| 10 | ~~Rollup migrations 003 + rollup endpoint + daily cron registration~~ | **(v0.2 deferred)** |
| 11 | Query plane: `kind: events`, `aggregate`, `breakdown` | integration: well-known queries |
| 12 | Query plane: `kind: funnel`, `retention`, `path` | integration: golden funnel + retention numbers |
| 13 | ~~Query planner: prefers rollups when applicable~~ | **(v0.2 deferred)** |
| 14 | GDPR `/erase` + `/export` + erasure log | integration: erase user, assert zero rows remain |
| 15 | Per-org quota + per-key sampling + 429s | integration: exceed quota → 429 |
| 16 | Policy engine integration (per-query authz) | integration: analyst role can aggregate but not dump events |
| 17 | `@reactor/analytics` JS SDK (track, identify, alias, page, optIn/Out, reset, flush) + transport + persistence | vitest + Playwright in `examples/` |
| 18 | SDK autoPageview (SPA hooks) + autoIdentify (wires to `auth.onAuthStateChange`) + autoErrors | Playwright: SPA route change + signin + thrown error |
| 19 | Runtime `ctx.analytics.track()` in Functions + Jobs + Sites | integration: server-emitted event lands in same project |
| 20 | `reactor-sites` snippet injection (manifest opt-in) | integration: deploy site with `analytics.enabled`, assert script tag |
| 21 | Tracing, Prometheus metrics, `/metrics` endpoint | unit + scrape test |
| 22 | `reactor-analytics-server` bin: `serve`, `migrate`, `doctor`, README quickstart | E2E harness boots in both monolith and microservices topology |

---

## 19. Open questions (v0.2+)

| # | Question | Trigger |
|---|---|---|
| 1 | ClickHouse adapter or Timescale adapter — when does Postgres stop being enough? | When p99 on common aggregates > 1s or storage > 1 TB. |
| 2 | Saved insights as first-class objects | When Studio needs to pin and re-run queries. |
| 3 | Audit ↔ Analytics unified read view | When a Reactor customer asks "show me deploys overlaid with traffic." |
| 4 | Web Vitals auto-capture (LCP/INP/CLS) | Customer demand + bundle-size headroom in SDK. |
| 5 | Session replay | Probably never in core; would live in `reactor-replay`. |
| 6 | Event schema registry (typed events with JSON Schema validation) | When customers ask for tighter contracts. |
| 7 | Streaming subscriptions (`reactor.analytics.live(query)`) | When realtime ships as a capability. |
| 8 | Multi-language server SDKs (Python, Go) | When non-JS customer demand justifies the maintenance cost. |
