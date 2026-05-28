# `reactor-server` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Eighth crate of the Reactor.cloud BaaS. Owns the *unified server binary* per `docs/ReactorCloud_spec.md` §3 (G1/G2 topologies) and §9 (workspace structure).
**Reader:** Whoever (human or agent) is about to build, extend, or operate the unified Reactor server.

This document describes *contracts* — composition model, configuration surface, lifecycle, admin API, topology mapping — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Provide a **single binary that runs every Reactor capability in one process** against one Postgres + one filesystem. This is the G1 (Tauri-embedded) and G2 (single VPS) target.
2. Be **library composition, not subprocess orchestration**. Each capability crate (`reactor-auth`, `reactor-data`, `reactor-storage`, `reactor-functions`, `reactor-jobs`, `reactor-sites`) already exposes `router(state)`, `Config`, and `migrator()`. `reactor-server` only wires them.
3. **Keep per-capability `*-server` binaries** as the production deployment shape (G3a/G3b/G3c) — they remain the source of truth for the HTTP surface; `reactor-server` mounts the same routers, never a parallel one.
4. **Auth is in-process** when running merged: `InProcessAuthClient` is selected automatically; no inter-service HTTP for token verification.
5. **One unified `Reactor.toml`** describes the project; `reactor-server` fans it out to per-capability config structs at boot. Env-var overrides match the existing `REACTOR_*` convention.
6. **Admin / deploy surface** is a stable set of `/_admin/*` HTTP routes that the (future) `reactor-cli` calls for `reactor deploy`, `reactor logs`, and `reactor doctor` against any grade.
7. **Cargo features** trim adapters per topology — a Tauri G1 build drops `aws-sdk-lambda`, `aws-sdk-s3`, `runtime-bun`, etc., while a G3a build drops nothing.
8. **Graceful shutdown** drains every capability's background task (jobs scheduler, jobs worker pool, signed-URL janitor, future schedulers) before exiting.

## 2. Non-goals (v0)

- **Sub-process supervisors** (no spawning capability binaries from `reactor-server`). If you want isolation, run the per-capability binaries directly.
- **Cross-tenant routing / multi-project hosting.** One `reactor-server` instance serves one project. Multi-tenancy of *users within a project* is the existing job of `reactor-auth`. Multi-project hosting is a G3c control-plane concern, deferred.
- **Hot-reload of capability code** at runtime. Function/job/site bundles hot-reload (their normal admin API does that). The Rust binary itself does not.
- **Custom plug-in capabilities.** The set of capabilities is fixed at compile time. A v1 plug-in story is on the roadmap; v0 is closed-set.
- **Built-in reverse proxy / TLS termination.** `reactor-server` is HTTP-only. TLS / HTTPS / domain routing is the deployer's job (Caddy / nginx / a CDN). G1 binds to `127.0.0.1`; G2 docs show a 5-line Caddy config.
- **A new control protocol.** Admin endpoints are plain HTTP+JSON; the same shape used by each capability today.
- **A separate config schema.** `Reactor.toml` reuses the existing per-capability struct shapes (just nested under `[auth]`, `[data]`, etc.) so there's exactly one source of truth.
- **Embedded Postgres.** `reactor-server` requires a reachable Postgres. SQLite path is a future concern (spec §3 G1).
- **Reload on `SIGHUP`.** Config changes require restart at v0.
- **A REST gateway for the LLM gateway capability.** That binary stays separate at v0; spec §10 phase B refactors it later.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, AuthClient trait
├── reactor-policy/                # (existing) shared policy engine
├── reactor-cache/                 # (existing) KV / queue primitives
├── reactor-auth/                  # (existing) library
├── reactor-auth-server/           # (existing) standalone bin (kept)
├── reactor-data/                  # (existing) library
├── reactor-data-server/           # (existing) standalone bin (kept)
├── reactor-storage/               # (existing) library
├── reactor-storage-server/        # (existing) standalone bin (kept)
├── reactor-functions/             # (existing) library
├── reactor-functions-server/      # (existing) standalone bin (kept)
├── reactor-jobs/                  # (existing) library
├── reactor-jobs-server/           # (existing) standalone bin (kept)
├── reactor-sites/                 # (planned) library
├── reactor-sites-server/          # (planned) standalone bin (kept)
│
└── reactor-server/                # NEW — the unified binary
    ├── Cargo.toml                 # depends on every capability lib (feature-gated)
    └── src/
        ├── main.rs                # entrypoint: parse args, init tracing, hand off
        ├── lib.rs                 # `run(config) -> Result<()>` so Tauri can embed
        ├── config.rs              # ReactorConfig (the unified shape) + loaders
        ├── boot/
        │   ├── mod.rs
        │   ├── pool.rs            # shared PgPool, http client, cache backend
        │   ├── auth.rs            # build AuthService + InProcessAuthClient once
        │   ├── migrate.rs         # fan out migrations across capabilities
        │   └── tracing.rs         # unified tracing layer + per-capability filters
        ├── compose/
        │   ├── mod.rs             # ServerCapabilities (feature-gated bundle)
        │   ├── auth.rs            # auth router + state assembly
        │   ├── data.rs            # data router + state assembly
        │   ├── storage.rs         # storage router + state assembly
        │   ├── functions.rs       # functions router + state assembly
        │   ├── jobs.rs            # jobs router + state assembly + bg tasks
        │   └── sites.rs           # sites router + state assembly
        ├── admin/
        │   ├── mod.rs             # /_admin/* router
        │   ├── deploy.rs          # POST /_admin/deploy (CLI deploy hook)
        │   ├── logs.rs            # GET  /_admin/logs   (SSE tail across caps)
        │   ├── doctor.rs          # GET  /_admin/doctor (capability self-check)
        │   └── auth.rs            # admin-token middleware
        ├── shutdown.rs            # graceful shutdown coordination (watch channel)
        └── cli/
            ├── mod.rs
            ├── doctor.rs          # `reactor-server doctor` subcommand
            └── migrate.rs         # `reactor-server migrate` subcommand
```

Conventions:
- `reactor-server` depends on each capability *library* (never on its `*-server` bin). Capability binaries are deployment artifacts; the library is the API.
- Capabilities are gated behind Cargo features: `cap-auth`, `cap-data`, `cap-storage`, `cap-functions`, `cap-jobs`, `cap-sites`. Default features are the full G2 bundle. A G1/Tauri build can drop heavy adapters via runtime sub-features (e.g. `cap-functions/runtime-wasm` only).
- The crate exposes both a `bin` (`reactor-server`) and a `lib` (`reactor_server::run(config)`) so the Tauri shell embeds the same code path.

## 4. Composition model

### 4.1 One process, library composition

Each capability already follows the same shape:

```rust
// reactor-auth/src/lib.rs
pub fn router(state: AuthState) -> axum::Router { /* /auth/v1/... */ }
pub fn migrator() -> sqlx::migrate::Migrator { /* ... */ }
pub struct AuthConfig { /* ... */ }
pub struct AuthState { /* ... */ }
```

`reactor-server` composes them by:
1. Building shared resources once (`PgPool`, `reqwest::Client`, cache backend, tracing).
2. Building each capability's `*State` from the shared resources + its config slice.
3. Calling `router(state)` and merging.
4. Spawning each capability's background tasks behind a shared shutdown channel.

```rust
// crates/reactor-server/src/lib.rs (sketch)
pub async fn run(cfg: ReactorConfig) -> anyhow::Result<()> {
    let shared    = boot::pool::init(&cfg).await?;        // PgPool, http, cache, tracing
    boot::migrate::run_all(&shared, &cfg).await?;
    let auth      = boot::auth::build(&shared, &cfg).await?; // AuthService + InProcessAuthClient

    let caps      = compose::ServerCapabilities::build(&shared, &auth, &cfg).await?;
    let app       = caps.router()
        .merge(admin::router(&shared, &caps, &cfg))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    caps.spawn_background(shutdown_rx.clone());

    let listener  = TcpListener::bind(cfg.server.bind).await?;
    let server    = axum::serve(listener, app)
        .with_graceful_shutdown(async move { let _ = shutdown_rx.clone().changed().await; });

    shutdown::wait_signal().await;
    let _ = shutdown_tx.send(true);
    caps.join_background().await;
    server.await?;
    Ok(())
}
```

### 4.2 Topology mapping (recap)

| Grade | Binary | Rationale |
|---|---|---|
| G1 — Tauri | `reactor-server` library, embedded | One process, in-binary, `127.0.0.1` only |
| G2 — single VPS | `reactor-server` bin | One process per host, shared Postgres |
| G3a/G3b/G3c | per-capability `reactor-{cap}-server` bins | Independent scaling and blast-radius isolation |

The HTTP surface is byte-identical across all topologies because every binary mounts the same `cap::router(state)`. CLI and SDK code never branch on topology.

### 4.3 Subprocess fallback (escape hatch, not v0 default)

In G2 a deployer may want to run `reactor-functions-server` separately (e.g. to keep wasmtime in its own address space). That works today because each capability has a stable HTTP contract. `reactor-server` then mounts every capability *except* functions, and `[functions]` in `Reactor.toml` carries `mode = "remote"` with a URL. The capability composer reads the mode and either builds an in-process router or registers a reverse-proxy stub. **The reverse-proxy stub is not built at v0**; this section documents the option, not the implementation.

## 5. Core types

### 5.1 `ReactorConfig`

The single config struct loaded at boot. Each field is a reused capability config (same struct, same defaults), nested under a section name that matches `Reactor.toml`.

```rust
// crates/reactor-server/src/config.rs
#[derive(Debug, Clone, Deserialize)]
pub struct ReactorConfig {
    pub project:    ProjectConfig,
    pub server:     ServerConfig,
    pub database:   DatabaseConfig,        // shared PgPool tuning
    pub tracing:    TracingConfig,
    pub admin:      AdminConfig,           // /_admin/* token + bind override

    // Capability slices — every field is `Option` so a capability can be omitted entirely
    pub auth:       Option<reactor_auth::AuthConfig>,
    pub data:       Option<reactor_data::DataConfig>,
    pub storage:    Option<reactor_storage::StorageConfig>,
    pub functions:  Option<reactor_functions::FunctionsConfig>,
    pub jobs:       Option<reactor_jobs::JobsConfig>,
    pub sites:      Option<reactor_sites::SitesConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub id:   String,                       // ReactorId, immutable
    pub grade: Grade,                       // tauri | single | cloud-managed | cloud-self | reactor-cloud
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "defaults::bind")]
    pub bind: SocketAddr,                   // default 0.0.0.0:8000
    #[serde(default = "defaults::request_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub cors: CorsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,                        // single PG URL shared by all capabilities
    #[serde(default = "defaults::pool_max")]
    pub pool_max: u32,                      // default 20
    #[serde(default = "defaults::acquire_secs")]
    pub acquire_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub token: String,                      // bearer for /_admin/*; required
    #[serde(default)]
    pub allow_remote: bool,                 // default false (admin only on 127.0.0.1)
}
```

Loading priority (figment, mirrors existing per-capability convention):

1. `Reactor.toml` (or `--config <path>`) — the project file.
2. Environment variables prefixed `REACTOR_*` (nested via `__`, e.g. `REACTOR_AUTH__JWT_ISSUER`).
3. Per-capability `REACTOR_AUTH_*`, `REACTOR_DATA_*`, etc. (kept for compatibility with the standalone binaries — same env spelling works in the merged binary).
4. CLI flags (`--bind`, `--admin-token`).

### 5.2 `Reactor.toml` shape

```toml
[project]
name  = "my-app"
id    = "proj_01HZ..."

[server]
bind  = "127.0.0.1:8000"
cors  = { allow_origins = ["http://localhost:5173"] }

[database]
url       = "postgres://reactor@127.0.0.1/reactor"
pool_max  = 20

[admin]
token        = "{{ env REACTOR_ADMIN_TOKEN }}"
allow_remote = false

[tracing]
filter = "info,reactor_auth=debug"
fmt    = "json"                  # json | pretty

[auth]
data_key      = "{{ env REACTOR_AUTH_DATA_KEY }}"
public_url    = "http://localhost:8000"
jwt_issuer    = "reactor-auth"
# ... existing AuthConfig fields ...

[data]
schema_dir = "./migrations"
# ... existing DataConfig fields ...

[storage]
backend = "fs"
path    = "./.reactor/blobs"

[functions]
runtimes = ["wasm", "bun"]       # subset of available runtimes
bundle_dir = "./.reactor/functions"

[jobs]
worker_count          = 4
scheduler_interval_ms = 1000

[sites]
# populated by `reactor sites add ...`
```

A capability not listed in the file is **not mounted**. This is how a Tauri build that omits Functions stays small.

### 5.3 `SharedResources`

Built once in `boot::pool` and handed to every capability composer. Keeps the surface for capability composers tiny and uniform.

```rust
// crates/reactor-server/src/boot/pool.rs
#[derive(Clone)]
pub struct SharedResources {
    pub pg:       sqlx::PgPool,
    pub http:     reqwest::Client,            // for outbound fetches (functions, sites)
    pub cache:    Arc<dyn reactor_cache::Backend>,
    pub clock:    Arc<dyn reactor_core::Clock>,
    pub shutdown: tokio::sync::watch::Receiver<bool>,
}
```

### 5.4 `ServerCapabilities`

Holds the assembled state and routers. Each field is `Option` so a feature-gated absence is well-typed.

```rust
// crates/reactor-server/src/compose/mod.rs
pub struct ServerCapabilities {
    pub auth:      Option<AuthSlot>,
    pub data:      Option<DataSlot>,
    pub storage:   Option<StorageSlot>,
    pub functions: Option<FunctionsSlot>,
    pub jobs:      Option<JobsSlot>,
    pub sites:     Option<SitesSlot>,
}

struct AuthSlot      { state: reactor_auth::AuthState,           router: axum::Router }
struct DataSlot      { state: reactor_data::DataState,           router: axum::Router }
struct StorageSlot   { state: reactor_storage::StorageState,     router: axum::Router }
struct FunctionsSlot { state: reactor_functions::FunctionsState, router: axum::Router, scheduler: Option<JoinHandle<()>> }
struct JobsSlot      { state: reactor_jobs::JobsState,           router: axum::Router, scheduler: JoinHandle<()>, workers: JoinHandle<()> }
struct SitesSlot     { state: reactor_sites::SitesState,         router: axum::Router }

impl ServerCapabilities {
    pub async fn build(shared: &SharedResources, auth: &AuthBundle, cfg: &ReactorConfig) -> Result<Self> { /* ... */ }
    pub fn router(&self) -> axum::Router { /* merge all slot routers */ }
    pub fn spawn_background(&self, rx: watch::Receiver<bool>) { /* launch scheduler + workers */ }
    pub async fn join_background(self) { /* await all task handles */ }
    pub fn doctor_probes(&self) -> Vec<DoctorProbe> { /* per-capability probes */ }
}
```

## 6. HTTP surface (composed)

Path namespacing keeps merged routers conflict-free:

| Capability | Prefix | Owner |
|---|---|---|
| Auth | `/auth/v1/*`, `/auth/v1/_internal/*` | `reactor_auth::router` |
| Data | `/data/v1/*` | `reactor_data::router` |
| Storage | `/storage/v1/*` | `reactor_storage::router` |
| Functions (invoke) | `/fn/v1/*` | `reactor_functions::router` |
| Functions (admin) | `/fn/v1/admin/*` | same |
| Jobs | `/jobs/v1/*` | `reactor_jobs::router` |
| Sites | `/sites/v1/*` (admin) and configured domains (serve) | `reactor_sites::router` |
| Health (composite) | `GET /health` | `reactor-server` |
| Admin (deploy/logs/doctor) | `/_admin/*` | `reactor-server` |
| Metrics | `GET /metrics` | `reactor-server` (aggregates capability metrics) |

`GET /health` returns 200 only if every mounted capability's own `/health` returns 200; otherwise 503 with the failing capability listed. This is the endpoint a load balancer or Tauri readiness check polls.

## 7. Admin & deploy surface (CLI contract)

The CLI talks to one place: `/_admin/*`, gated by the admin bearer token. This is the v0 contract `reactor-cli` will be built against.

```
POST   /_admin/deploy                       Body: multipart with bundle.tar.zst
                                            Effect: applies migrations, swaps function bundles,
                                                    reloads job manifest, swaps site bundle.
                                            Returns: SSE stream of deploy events.
                                            Requires: admin token + project id match

GET    /_admin/doctor                       Returns: { capability -> ProbeResult }
GET    /_admin/logs?capability=&since=&follow=1
                                            SSE stream merging tracing output across capabilities
GET    /_admin/version                      Returns: { reactor: "0.1.0", capabilities: { ... } }
POST   /_admin/migrate                      Re-runs migrations (idempotent); used by CLI for safety
POST   /_admin/shutdown                     Graceful shutdown; useful for `reactor stop` in dev
```

The deploy bundle layout is fixed:

```
deploy.tar.zst
├── manifest.json                    # describes which capabilities are touched
├── migrations/
│   ├── data/                        # applied via reactor_data::migrator()
│   └── (others as needed)
├── functions/<name>/bundle.tar.zst  # one per function deployment
├── jobs/manifest.json               # job + trigger registrations
└── sites/<site-name>/bundle.tar.zst # site bundle per spec §6.3.b
```

Deploy is **transactional per-capability, sequential across capabilities**: data migrations first (rollback-safe), then storage policies, then functions, then jobs, then sites. If any capability fails, prior capabilities are not rolled back automatically — the response calls out the partial state and the CLI surfaces it (matches existing per-capability deploy semantics; cross-capability rollback is a v1 concern).

## 8. Lifecycle (G2 walk-through)

This is the boot path on a single VPS using `reactor-server`. Times in `[ms]` are budget; quality-bar (spec §13) is `< 1s` to listening.

```
[ 0]  parse argv, load Reactor.toml + env
[10]  init tracing (json/pretty per cfg)
[20]  build shared PgPool                 (pool_max=20)
[40]  ping pg + select 1                  (fail fast)
[50]  run boot::migrate::run_all in topological order:
        - reactor_auth::migrator()            -> _reactor_auth.*
        - reactor_data::migrator()            -> public + _reactor_data.*
        - reactor_storage::migrator()         -> _reactor_storage.*
        - reactor_functions::migrator()       -> _reactor_functions.*
        - reactor_jobs::migrator()            -> _reactor_jobs.*
        - reactor_sites::migrator()           -> _reactor_sites.*
[200] build AuthService + InProcessAuthClient (shared by every other capability)
[250] build per-capability state slots, with auth client injected
[400] mount each capability's router under its prefix
[420] mount /_admin/* + /health + /metrics
[450] spawn jobs scheduler + worker pool
[460] spawn signed-URL token janitor
[470] bind TcpListener and start serving
[470] tracing::info!("listening", bind = ..., capabilities = [...])
```

Failure modes (boot-time):
- **Migration failure** → exit 1 with the failing capability + SQL error. CLI surfaces this and prevents a half-applied deploy.
- **Missing required config** (`auth.data_key`, `database.url`, `admin.token`) → `figment` error with key path; exit 2.
- **Port bind failure** → exit 3.
- **AuthService cannot derive signing key** (KMS unavailable) → exit 4.

Failure modes (runtime):
- A capability can fail health (e.g. functions runtime crashed): `/health` flips to 503 for that capability; the rest stay up. The orchestrator (systemd / Tauri shell) decides whether to restart.

## 9. AuthClient selection

Shared rule: `reactor-server` always uses `InProcessAuthClient`. Per-capability binaries continue to support both `monolith` and `microservices` deployment modes per their existing config. The composer in `compose/auth.rs`:

```rust
let auth_state  = build_auth_state(shared, cfg.auth.as_ref())?;
let auth_client = Arc::new(reactor_auth::client::InProcessAuthClient::new(auth_state.service.clone()));
let bundle      = AuthBundle { state: auth_state, client: auth_client };
```

Every other capability's state takes `Arc<dyn AuthClient>` from `bundle.client`. No HTTP, no JWT round-trip across loopback, no `internal_secret` plumbing inside one process. The internal HTTP secret remains required for the standalone binaries — it does not apply when `reactor-server` builds the merged graph.

If `[auth] = ...` is omitted from `Reactor.toml`, `reactor-server` errors at boot. There is no "no-auth" mode. (A future grade-3 mode where Reactor delegates identity to Supabase Auth still requires an `AuthClient` — it's just `SupabaseAuthClient` instead of `InProcessAuthClient`. That adapter is out of scope for v0 of `reactor-server`.)

## 10. Background tasks

Each capability declares its bg tasks and `reactor-server` owns the lifecycle.

| Capability | Background tasks | Shutdown contract |
|---|---|---|
| `reactor-jobs` | scheduler poll loop, worker pool | drain in-flight runs, then exit; honors `worker_drain_secs` |
| `reactor-storage` | signed-URL token janitor, multipart cleanup | finish current pass, then exit |
| `reactor-functions` | warm-pool reaper (Bun), deployment reconciler | finish current pass; destroy idle handles |
| `reactor-data` | (none at v0; realtime arrives later) | n/a |
| `reactor-auth` | key rotation timer | finish current rotation, then exit |
| `reactor-sites` | bundle GC, certificate renewal (G2) | finish current pass |

All tasks subscribe to a shared `tokio::sync::watch::Receiver<bool>` from `shutdown.rs`. On signal, the channel flips and tasks return their `JoinHandle`s; `ServerCapabilities::join_background` awaits all of them. Total drain budget defaults to 30s and is configurable per capability.

## 11. Cargo features and topology trims

```toml
# crates/reactor-server/Cargo.toml
[features]
default = ["g2-full"]

# Topology bundles
g1-tauri      = ["cap-auth", "cap-data", "cap-storage", "cap-functions/runtime-wasm", "cap-jobs"]
g2-full       = ["cap-auth", "cap-data", "cap-storage", "cap-functions", "cap-jobs", "cap-sites"]
g3-cap-mix    = []   # for the per-capability bins; reactor-server isn't used at G3

# Capability gates
cap-auth      = ["dep:reactor-auth"]
cap-data      = ["dep:reactor-data"]
cap-storage   = ["dep:reactor-storage"]
cap-functions = ["dep:reactor-functions"]
cap-jobs      = ["dep:reactor-jobs"]
cap-sites     = ["dep:reactor-sites"]
```

Sub-features pass through to capabilities (e.g. `cap-functions/runtime-wasm` enables the wasmtime runtime, drops Bun and Lambda). The Tauri build target sets `default-features = false` and picks `g1-tauri`, trimming the AWS SDK and the Bun warm pool.

Binary-size budget (spec §13):
- G1 Tauri-embedded `reactor-server`: ≤ 30 MB stripped.
- G2 default `reactor-server`: ≤ 60 MB stripped.

## 12. Observability

- **Tracing** is initialized once. Every capability uses `tracing::info_span!(...)` which already carries the capability name as a target; the merged binary gets fan-in for free.
- **Metrics** at `/metrics` aggregates Prometheus counters/histograms each capability registers with the shared `metrics` registry (each capability already has a `routes/metrics.rs`). `reactor-server` exposes a single endpoint instead of per-capability ones.
- **Logs** at `/_admin/logs` is an SSE stream that fans out the same tracing layer in JSON, with optional `?capability=auth` filter. Useful for `reactor logs tail` against a remote G2 deployment.
- **Health** at `/health` aggregates per-capability `GET /<cap>/v1/health` (in-process call, not HTTP).
- **Doctor** at `/_admin/doctor` runs each capability's structured probe set (DB ping, runtime presence, storage backend reachability, etc.) and returns a single JSON report.

## 13. Test strategy

1. **Per-crate** tests stay where they are. `reactor-server` does not duplicate them.
2. **Composition tests** in `crates/reactor-server/tests/` boot the whole binary with `testcontainers::Postgres` + a tempdir-backed FS storage. All tests reach the system through HTTP; capability state is *never* hand-constructed in test code:
   - `boot.rs` — verifies migrations apply cleanly across capabilities and `/health` returns 200.
   - `deploy_e2e.rs` — push a fixture bundle (data migration + 1 function + 1 job + 1 site) via `/_admin/deploy`, assert each capability reports the new artifact.
   - `auth_data_e2e.rs` — sign up via auth, hit data with the issued JWT, verify RLS works in-process.
   - `functions_invoke_e2e.rs` — deploy a wasm bundle via `/_admin/deploy`, invoke it through `/fn/v1/`.
   - `jobs_trigger_e2e.rs` — register a job via `/_admin/deploy`, trigger it, verify the embedded scheduler picks it up.
   - `migrate_idempotent.rs` — `POST /_admin/migrate` twice; second call is a no-op.
   - `doctor.rs` — `GET /_admin/doctor` returns 200 for a healthy stack and a structured failure for a deliberately broken capability (e.g. wrong storage path).
   - `shutdown.rs` — SIGTERM mid-flight, verify drain completes without orphan tasks.
3. **Topology contract tests** assert that the routes mounted by `reactor-server` are a *superset* of routes mounted by each per-capability binary. Implemented by introspecting `axum::Router::routes()` (or a per-capability `route_table()` helper if introspection is unavailable). This stops the unified binary from drifting from the standalone ones.

## 14. Roadmap

**v0 (this doc):**
- Crate scaffold, `ReactorConfig`, shared resources, composer, jobs/storage/functions background tasks.
- Cargo features for G1 / G2 trims.
- `/health` (composite), `/metrics` (aggregated), `/_admin/version`.
- **`POST /_admin/deploy` (synchronous JSON response).** Bundle layout per §7. Orchestrator calls each capability's existing service methods sequentially: data migrations → storage policies → functions deployments → jobs manifest → sites bundle. Best-effort, no cross-capability rollback.
- **`POST /_admin/migrate`** — re-run all migrators idempotently.
- **`GET /_admin/doctor`** — fan-in of each capability's existing doctor probes.
- **`POST /_admin/shutdown`** — graceful shutdown trigger.
- E2E composition tests (§13) that exercise the full deploy path, not hand-built capability state.
- Replaces nothing — runs alongside the per-capability binaries. v0 is `curl`-driven; no CLI dependency.

**v0.1:**
- Streaming deploy progress: SSE event stream from `/_admin/deploy` (build/apply/swap phases per capability).
- `GET /_admin/logs` SSE multiplexing across capabilities with `?capability=` filter.
- `reactor-cli` deploy/logs/doctor commands wired against this admin surface.
- Tauri embedding via `reactor_server::run(cfg)` from inside the desktop shell.

**v0.2:**
- "Remote capability" stub for the subprocess-fallback escape hatch (§4.3).
- SQLite path for G1 standalone binaries (Postgres-only at v0).

**v1:**
- Cross-capability deploy rollback (currently sequential best-effort).
- Plug-in capability surface so non-core capabilities (e.g. `reactor-realtime`, `reactor-search`) can register routers + bg tasks without forking `reactor-server`.
- Multi-region awareness for G3c hosted topology.

---

*End. Update via PR; this file describes intent. Implementation lands behind a follow-up plan in `.cursor/plans/reactor-server_v0_*.plan.md`.*
