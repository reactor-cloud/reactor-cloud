# Reactor.cloud BaaS — Architecture Specification

**Status:** Draft v0 (architectural intent), May 2026
**Relationship to `SPEC.md`:** `SPEC.md` defines the v1 LLM-gateway product (Reactor.cloud today). This document defines the long-horizon BaaS product Reactor.cloud becomes once the gateway has product-market fit. Treat it as the north star. Nothing here is built at v1; it shapes how v1 is built.

---

## 1. Product Thesis

Reactor.cloud becomes an **AI-first BaaS** designed for agents to provision, build, and operate full applications through a single CLI and a single API surface.

The bet: every existing BaaS (Supabase, Firebase, Vercel, Render, Convex) was designed for humans clicking dashboards. Reactor.cloud is designed for agents calling APIs. That changes:
- The **CLI / SDK** is the primary interface, not the dashboard.
- The **API surface is small, orthogonal, and stable** so an agent can hold it all in context.
- The **local dev experience** is one binary that boots in seconds, not a Docker Compose graveyard.
- The **same project definition** runs locally, on a single VPS, or on managed cloud — without code changes.

We get there by writing **thin Rust control-plane layers** in front of well-known OSS components (Postgres, Redis, GoTrue, Firecracker, etc.), the same way the gateway is a thin Rust layer in front of Bedrock and OpenRouter today.

---

## 2. Capabilities (the surface)

The complete BaaS exposes eight capability domains. Each is a stable HTTP surface; each is also exposed via SDK and CLI; each is implemented behind a Rust trait with multiple adapters.

| # | Capability | What it does | Reference product |
|---|---|---|---|
| 1 | **Identity** | Auth, users, orgs, roles, MFA, IDPs, JWT issuance | Auth0 / Clerk / Supabase Auth |
| 2 | **Data** | Typed tables, queries, mutations, RLS, realtime subscriptions | PostgREST / Supabase DB |
| 3 | **Storage** | Blob upload/download, signed URLs, lifecycle | S3 / R2 / Supabase Storage |
| 4 | **Functions** | One-shot serverless functions (HTTP handlers) | Vercel Functions / Supabase Edge Functions |
| 5 | **Jobs** | Durable, retryable, scheduled background work | trigger.dev / Inngest / Temporal |
| 6 | **Sites** | App hosting (Next.js, SvelteKit, etc.) and static-file hosting | Vercel / Netlify / Cloudflare Pages |
| 7 | **Gateway** | LLM routing, metering, observability (already shipped) | Reactor.cloud today |
| 8 | **Connect** | Third-party API connectors, data sync, webhooks | Airbyte / Fivetran / Nango |

Realtime, queues, and cache are *primitives* that several of the above depend on, not user-facing capabilities of their own.

---

## 3. Spec Grades (deployment topologies)

The same `Reactor.cloud-server` binary runs in five topologies. The user picks one in `Reactor.cloud.toml`; the rest is invisible.

| Grade | Target | Identity | Data | Storage | Functions | Jobs | Sites |
|---|---|---|---|---|---|---|---|
| **G1 — Tauri** | Desktop dev app, single binary | embedded | SQLite + RLS sim | local FS | in-process WASM or shell | in-process scheduler | static FS + dev server proxy |
| **G2 — Single server** | One VPS, docker-compose, homelabs | embedded or GoTrue | Postgres | local FS or MinIO | Docker subprocess | embedded scheduler + Postgres queue | nginx-style static + reverse proxy to functions |
| **G3a — Managed cloud (default)** | Fly Machines / Render / Railway | Reactor.cloud Identity OR Supabase Auth | Neon / Supabase Postgres | R2 / S3 / Tigris | Fly Machines / Cloudflare Workers | Fly Machines + durable Postgres | Fly Machines + CDN |
| **G3b — Enterprise self-host** | Customer's AWS / GCP | Reactor.cloud Identity on EKS | RDS / Aurora | S3 | Lambda / Fargate | Step Functions / SQS-backed | ALB + S3/CloudFront |
| **G3c — Reactor.cloud Cloud (hosted)** | We run it | Reactor.cloud Identity | per-region Postgres | per-region object storage | Firecracker microVMs (Vercel Sandbox–style) | durable Postgres + workers | edge CDN |

**Invariant:** *the same SDK call, the same CLI command, the same project file works on every grade.* Adapters differ; semantics are uniform up to documented contract weakening (e.g., realtime delivery guarantees).

---

## 4. The PostgREST-Shaped Data Surface

The Data capability is a single HTTP API modeled on PostgREST, intentionally familiar to anyone who's used Supabase or Postgrest directly. This is the contract; SQLite and Postgres are both translation targets.

### 4.1 Surface

```
GET    /data/v1/{table}                 -- select with filters: ?col=eq.value&order=col.desc&limit=20
POST   /data/v1/{table}                 -- insert (single or array)
PATCH  /data/v1/{table}?id=eq.{id}      -- update
DELETE /data/v1/{table}?id=eq.{id}      -- delete
POST   /data/v1/rpc/{function_name}     -- call a stored procedure / Rust-defined function
GET    /data/v1/{table}?subscribe=1     -- SSE/WebSocket realtime stream

Headers:
  Authorization: Bearer <jwt>           -- enforces RLS as that user
  Prefer: return=representation         -- standard PostgREST verbs
  Prefer: count=exact                   -- standard PostgREST verbs
  X-Reactor.cloud-Org: <org_id>                 -- explicit tenant scope when JWT spans multiple orgs
```

### 4.2 Schema migrations

A subset SQL dialect that compiles to both Postgres and SQLite:

```sql
-- Reactor.cloud-migrations/001_init.sql
create table todos (
  id          Reactor.cloud_id primary key,         -- maps to uuid (PG) or text (SQLite)
  org_id      Reactor.cloud_id not null,
  title       text not null,
  done        bool not null default false,
  created_at  timestamptz not null default now()
) with (rls = true);

policy todos_owner on todos
  for select, update, delete
  using (org_id = current_org_id());

policy todos_insert on todos
  for insert
  check (org_id = current_org_id());
```

The Rust `MigrationCompiler` rewrites this to native dialect at apply time. Forbidden constructs (PG-only types, JSONB ops, partial indexes) are rejected at lint time so a project can't accidentally become non-portable.

### 4.3 RLS on SQLite

SQLite has no native RLS. The Rust `DataStore` enforces policy in code:

1. Every query goes through a query rewriter that reads policies for the target table.
2. Policies are compiled to SQL `WHERE` clauses appended to the user's query.
3. `INSERT`/`UPDATE` go through a `CHECK` evaluator before commit.
4. `current_org_id()`, `current_user_id()`, `auth.role()` etc. resolve from the verified JWT.

On Postgres the policies are also installed natively as a defense-in-depth backstop. Same policy text, two enforcement points.

### 4.4 Realtime

- **G1, G2:** in-process tokio broadcast channels keyed by table+filter.
- **G3a, G3c:** NATS JetStream cluster.
- **G3b:** customer-chosen (NATS, Redis Streams, or AWS SNS).

Contract: **at-least-once delivery, best-effort ordering per (table, primary key)**. Customers needing exactly-once must idempotency-key on the consumer side. Documented loudly.

---

## 5. Identity — the case for owning it

The user's instinct is right: this is the one Rust component where ownership pays back fastest *for the BaaS framing*, even though it's not the right v1 move (we use Supabase Auth at v1 per `SPEC.md` direction).

### 5.1 Why own it

1. **Multi-tenancy is the whole product.** Every BaaS lives or dies by its tenancy model. Renting one means renting your product's spine.
2. **Permission system as a differentiator.** Owning identity lets us ship a permission DSL that's first-class in the data layer (`policy ... using (auth.has_permission('todos:read'))`) instead of bolted on.
3. **Fits the embedded-Tauri story.** GoTrue + Postgres in a desktop app is heavy. Native Rust auth in-process with SQLite is a few MB.
4. **Email/Google/GitHub + TOTP is small.** This is genuinely the slice that's reasonable to write — not full Zitadel.

### 5.2 Minimum viable surface

```
POST   /auth/v1/signup              { email, password }
POST   /auth/v1/token?grant_type=password
POST   /auth/v1/token?grant_type=refresh_token
POST   /auth/v1/token?grant_type=otp                -- magic link confirm
POST   /auth/v1/logout
POST   /auth/v1/recover             { email }       -- password reset email
POST   /auth/v1/verify              { token, type } -- email confirm, password reset, magic link
POST   /auth/v1/factors             { factor_type: 'totp' }
POST   /auth/v1/factors/{id}/verify { code }
POST   /auth/v1/factors/{id}/challenge

GET    /auth/v1/authorize?provider=google           -- OAuth start (Google, GitHub)
GET    /auth/v1/callback                            -- OAuth callback

GET    /auth/v1/user                                -- current user
PATCH  /auth/v1/user                                -- update email/password/metadata

# Multi-tenancy
GET    /auth/v1/orgs                                -- orgs the user belongs to
POST   /auth/v1/orgs                                -- create org
POST   /auth/v1/orgs/{id}/invitations
POST   /auth/v1/orgs/{id}/members/{user_id}/role
DELETE /auth/v1/orgs/{id}/members/{user_id}

# Permission DSL (Reactor.cloud-specific extension)
GET    /auth/v1/permissions                         -- effective permissions for current (user, org)
```

The shape is intentionally close to GoTrue's so existing Supabase clients keep working. Drop-in replacement on grade 3a is a feature, not a constraint.

### 5.3 Tenancy model

- **User** — global identity (email, password hash, MFA factors, IDP links).
- **Org** — tenant boundary. Owns data, billing, API keys.
- **Membership** — `(user, org, role)`. Roles are project-defined, not built-in.
- **Permission** — `(role, resource, action)`. Composable, queryable from policies.
- **API key** — non-human credential, scoped to (org, [permissions]). Same JWT format as user tokens; `sub` is `apikey:<id>`.

### 5.4 What we explicitly do *not* build

- SAML / enterprise SSO (defer to v3 or proxy via WorkOS).
- WebAuthn (v2.5+).
- Audit log UI (the events go to `usage_events`-style storage; UI is much later).
- A full account console (the dashboard is part of `Reactor.cloud-web`, not auth).

### 5.5 Crypto / standards rules

- JWTs: **RS256**, JWKS at `/auth/v1/keys`, key rotation with overlap window (active + previous kid both valid for 7 days after rotation).
- Password hashing: **argon2id** (same as the gateway's API key hashing).
- TOTP: RFC 6238, 30s window, 1-step drift.
- OAuth state + PKCE mandatory for `/authorize`.
- Tokens: short-lived access (1h), refresh (30d, rotating, single-use).

---

## 6. Functions vs Jobs vs Sites

These three are easy to conflate; the distinction is load-bearing.

### 6.1 Functions — request/response

Stateless HTTP handler. Cold-start tolerant. Bounded duration.

```
Reactor.cloud functions deploy ./api/checkout.ts
→ POST https://{project}.Reactor.cloud.app/fn/checkout
```

Contract: a function is a Web `Request` → `Promise<Response>` (Web Standard APIs). Languages at v1: TypeScript (via Bun runtime or QuickJS), Rust (compile to WASM). Adapters per grade:

| Grade | Adapter |
|---|---|
| G1 | in-process JS runtime (`rquickjs` or `boa`) for TS; `wasmtime` for Rust |
| G2 | Docker subprocess per function, kept warm with LRU |
| G3a | Fly Machines (one machine per function, scale-to-zero) or Cloudflare Workers |
| G3b | AWS Lambda |
| G3c | Vercel-Sandbox-style Firecracker microVMs |

### 6.2 Jobs — durable background work

Survives crashes, retries with backoff, runs on schedule, can pause/resume. Modeled after trigger.dev / Inngest.

```ts
// jobs/process-signup.ts
export default job("process-signup", async (ctx, payload) => {
  const user = await ctx.step("create-user", () => createUser(payload));
  await ctx.step("send-welcome", () => sendEmail(user));
  await ctx.step("seed-data", () => seedTenant(user.org_id));
});

// trigger:
await Reactor.cloud.jobs.trigger("process-signup", { email });

// schedule:
Reactor.cloud.jobs.cron("0 * * * *", "hourly-rollup");
```

Contract: each `ctx.step()` is recorded, deduped by step ID, retried independently. State machine persisted in the project's own Postgres/SQLite (same DB as user data, separate schema `_Reactor.cloud_jobs`).

| Grade | Adapter |
|---|---|
| G1 | in-process scheduler, SQLite-backed durable state |
| G2 | embedded scheduler in `Reactor.cloud-server`, Postgres-backed |
| G3a | dedicated Fly Machines pool, Postgres-backed |
| G3b | Step Functions or Temporal customer-deployed |
| G3c | Reactor.cloud-managed worker pool, sharded by project |

We **do not** require a separate queue product (SQS, Redis Streams) at G1/G2. Postgres `SKIP LOCKED` is the queue. This is the trigger.dev v3 approach and it scales further than people expect.

### 6.3 Sites — frontends

Two flavors, one mental model:

**6.3.a Static sites.** Pure HTML/CSS/JS/assets. Zero compute.

```
Reactor.cloud sites deploy ./dist --domain shop.example.com
```

| Grade | Adapter |
|---|---|
| G1 | local FS, served by `Reactor.cloud-server` on `localhost:<port>` |
| G2 | local FS, served with HTTP cache headers |
| G3a | upload to R2/S3, fronted by CDN (Cloudflare) |
| G3b | S3 + CloudFront |
| G3c | Reactor.cloud CDN edge |

**6.3.b App hosting.** Full Next.js / SvelteKit / Astro / Nuxt with SSR, ISR, edge runtime. This is the Vercel-shaped product.

```
Reactor.cloud sites deploy ./ --framework nextjs
```

The build pipeline does framework detection, runs the framework's official build, and produces a *Reactor.cloud Site Bundle*: a directory of static assets plus a manifest of compute routes that map to Functions:

```
.Reactor.cloud-bundle/
  static/                ← copied to CDN
  functions/
    [...slug].fn.js      ← deploys as a Reactor.cloud Function
  manifest.json          ← routing rules, cache headers, ISR config
```

This means **app hosting reduces to static + functions**. We don't build a bespoke serving stack; we lean on the two we already have. Borrows directly from Vercel's Build Output API design — there's no need to invent a new contract.

| Grade | Adapter |
|---|---|
| G1 | dev server runs `next dev` (or framework equivalent) under a reverse proxy |
| G2 | run the framework's Node server directly under a reverse proxy |
| G3a | static → R2/CDN, functions → Fly Machines |
| G3b | static → S3/CloudFront, functions → Lambda |
| G3c | static → Reactor.cloud CDN, functions → Firecracker |

### 6.4 Why this triangle works

- **One mental model** for the agent: "static assets" + "functions" + "scheduled/durable jobs". An app is a composition of those.
- **Frameworks compile to the model**, not the other way around. We never become a bespoke Next.js host with all the version-coupling pain Vercel eats.
- **Locally and in the cloud the bundle is identical.** The agent debugging `next build` output sees the same file layout that ships to prod.

---

## 7. Project Model

Everything an agent builds is a **Project**. A project is a directory with `Reactor.cloud.toml` plus convention-located source for each capability it uses.

```
my-app/
├── Reactor.cloud.toml
├── migrations/                    ← data
│   └── 001_init.sql
├── functions/                     ← functions
│   └── checkout.ts
├── jobs/                          ← jobs
│   ├── process-signup.ts
│   └── nightly-rollup.ts
├── sites/                         ← sites (one or many)
│   ├── web/                       ← Next.js app
│   └── status-page/               ← static HTML
└── .Reactor.cloud/                        ← git-ignored local state (G1)
    ├── data.sqlite
    ├── blobs/
    └── logs/
```

`Reactor.cloud.toml`:

```toml
[project]
name        = "my-app"
id          = "proj_01HZ..."          # generated, immutable

[grade]
profile     = "tauri"                 # tauri | single | cloud-managed | cloud-self | Reactor.cloud-cloud

[identity]
provider    = "embedded"              # embedded | gotrue | supabase | Reactor.cloud
providers.google = { client_id = "...", client_secret_env = "GOOGLE_SECRET" }
providers.github = { client_id = "...", client_secret_env = "GITHUB_SECRET" }
mfa.totp    = true

[data]
backend     = "sqlite"                # sqlite | postgres
url         = "./.Reactor.cloud/data.sqlite"  # or "postgres://..."

[storage]
backend     = "fs"                    # fs | s3 | r2
path        = "./.Reactor.cloud/blobs"

[functions]
runtime     = "wasm"                  # wasm | docker | lambda | fly | workers

[jobs]
scheduler   = "embedded"              # embedded | external

[[sites]]
path        = "sites/web"
framework   = "nextjs"
domains     = ["app.example.com"]

[[sites]]
path        = "sites/status-page"
framework   = "static"
domains     = ["status.example.com"]
```

The project file is the deployment unit. `Reactor.cloud deploy` reads it, builds bundles per site/function/job, applies migrations in dependency order, and pushes to whatever grade is configured.

---

## 8. CLI Surface

The CLI is the canonical agent interface. Every command works on every grade.

```
# Project lifecycle
Reactor.cloud init [template]                  # scaffold a new project
Reactor.cloud dev                              # boot local stack (G1) and watch
Reactor.cloud deploy [--env prod|staging]      # build + push to configured grade
Reactor.cloud destroy [--env]                  # tear down a deployment
Reactor.cloud env [list|set|get|pull]          # env var management
Reactor.cloud logs [tail|search]               # unified log surface across all capabilities
Reactor.cloud doctor                           # health + connectivity checks

# Identity
Reactor.cloud users [list|invite|disable]
Reactor.cloud orgs  [list|create|members|roles]
Reactor.cloud keys  [list|create|revoke]

# Data
Reactor.cloud db [migrate|reset|shell|dump|restore|studio]
Reactor.cloud db generate-types                # emit TS types from schema

# Storage
Reactor.cloud storage [ls|cp|rm|sign-url]

# Functions
Reactor.cloud functions [list|deploy|invoke|logs <name>]

# Jobs
Reactor.cloud jobs [list|trigger <name>|runs|schedule]

# Sites
Reactor.cloud sites [list|deploy|domains|rollback]

# Gateway (existing)
Reactor.cloud gateway [logs|usage|balance|keys]
```

All commands accept `--json` for piping. All commands accept `--project ./path` to operate on a non-cwd project.

---

## 9. Workspace Structure

One Cargo workspace, separable artifacts.

```
crates/
  Reactor.cloud-core              # traits, types, errors, config (Reactor.cloud.toml parsing)
  Reactor.cloud-identity          # IdentityProvider trait + impls
    Reactor.cloud-identity-embedded
    Reactor.cloud-identity-gotrue
    Reactor.cloud-identity-supabase
  Reactor.cloud-data              # DataStore trait + impls + dialect compiler + RLS engine
    Reactor.cloud-data-sqlite
    Reactor.cloud-data-postgres
  Reactor.cloud-storage           # BlobStore trait + impls
    Reactor.cloud-storage-fs
    Reactor.cloud-storage-s3
  Reactor.cloud-functions         # FunctionRuntime trait + impls
    Reactor.cloud-functions-wasm
    Reactor.cloud-functions-docker
    Reactor.cloud-functions-lambda
    Reactor.cloud-functions-fly
  Reactor.cloud-jobs              # JobScheduler trait + impls
    Reactor.cloud-jobs-embedded
    Reactor.cloud-jobs-external
  Reactor.cloud-sites             # SiteHost trait + framework adapters
    Reactor.cloud-sites-static
    Reactor.cloud-sites-nextjs
    Reactor.cloud-sites-sveltekit
    Reactor.cloud-sites-astro
  Reactor.cloud-gateway           # existing LLM gateway, lifted into the workspace
  Reactor.cloud-realtime          # EventBus trait
  Reactor.cloud-kvq               # KvAndQueue trait
  Reactor.cloud-server            # axum HTTP API; wires capabilities behind traits
  Reactor.cloud-cli               # the `Reactor.cloud` binary
  Reactor.cloud-sdk-types         # canonical types for SDK codegen
apps/
  Reactor.cloud-tauri             # desktop shell, embeds Reactor.cloud-server in-process
  Reactor.cloud-cloud-image       # Dockerfile + Helm + Fly machine image
  Reactor.cloud-web               # marketing + dashboard (Next.js — already exists)
sdk/
  ts                      # @Reactor.cloud/sdk (already exists)
  python                  # later
  go                      # later
```

Cargo features per crate select adapters at compile time. The Tauri build excludes `s3`, `lambda`, `gotrue` features, etc., for binary size.

---

## 10. Build Sequence

This is the order I'd actually build the BaaS, with explicit checkpoints. Each step is shippable on its own.

### Phase A — v1 (already in flight per `SPEC.md`)
*Goal: the LLM gateway is paying its own bills.*

- A1. Ship `gateway`, `worker`, `web` against Supabase x2 (EU + US).
- A2. Ship `cli` for log tailing + balance + keys.
- A3. Ship `@Reactor.cloud/sdk` v1.
- A4. Soft launch.

### Phase B — Reframe as a project
*Goal: nothing changes for paying gateway customers; internally we start treating Reactor.cloud as a "project platform" that happens to currently expose only the Gateway capability.*

- B1. **Define `Reactor.cloud-core` traits** for Identity, Data, Storage, Functions, Jobs, Sites, Gateway. No impls yet — just the contracts.
- B2. **Define `Reactor.cloud.toml` schema** and the project model. Document it.
- B3. **Refactor `gateway` to implement the Gateway trait** and consume `Reactor.cloud-core` config.
- B4. **Refactor `cli` to be project-aware** (`Reactor.cloud deploy` works for a Gateway-only project today).
- B5. Add `Reactor.cloud init --template gateway` that scaffolds a one-capability project.

Checkpoint: existing customers see no change; Reactor.cloud now has a project skeleton.

### Phase C — Identity (Reactor.cloud-owned, opt-in)
*Goal: own the auth layer per §5; Supabase Auth remains the supported alternative.*

- C1. `Reactor.cloud-identity-embedded` v0: email/password signup + login + JWT issuance + JWKS rotation + RS256.
- C2. Email verification + password reset (requires SMTP config; SMTP-less builds disable these flows).
- C3. Google + GitHub OAuth with PKCE.
- C4. TOTP MFA.
- C5. Org tenancy: orgs, memberships, roles.
- C6. Permission DSL + `current_user_id()` / `current_org_id()` hooks for the Data layer.
- C7. `Reactor.cloud-identity-supabase` adapter (drop-in for existing v1 customers).
- C8. `Reactor.cloud-identity-gotrue` adapter (for self-hosters who want OSS GoTrue).

Checkpoint: a project can authenticate users via Reactor.cloud Identity in any grade.

### Phase D — Data (PostgREST-shaped surface)
*Goal: same API talks to SQLite or Postgres with RLS enforced uniformly.*

- D1. SQL dialect lint + compiler (`Reactor.cloud_id`, portable types, forbidden constructs).
- D2. `MigrationRunner` for both SQLite and Postgres.
- D3. PostgREST-shaped HTTP surface: filters, ordering, pagination, embedded resources.
- D4. RLS policy parser + executor (Rust-side enforcement on SQLite; native + Rust-side on Postgres).
- D5. RPC endpoints (call Rust-defined or SQL-defined functions).
- D6. Realtime subscriptions over SSE (G1/G2) and NATS (G3).
- D7. `Reactor.cloud db generate-types` for TS (and Python/Go later).

Checkpoint: a project can model and query data through the Reactor.cloud Data API with the same code locally and in cloud.

### Phase E — Storage
*Goal: blobs work.*

- E1. `Reactor.cloud-storage-fs` — local FS adapter; signed URLs via HMAC tokens served by `Reactor.cloud-server`.
- E2. `Reactor.cloud-storage-s3` — S3 / R2 / Tigris adapter; native presigned URLs.
- E3. Upload/download streaming with multipart.
- E4. Lifecycle policies (TTL, automatic cleanup).
- E5. Per-object ACLs that compose with Identity permissions.

### Phase F — Functions
*Goal: deploy and invoke a Web-handler-style function on every grade.*

- F1. Function bundle format (manifest + entrypoint).
- F2. `Reactor.cloud-functions-wasm` (Rust → WASM, runs in `wasmtime`).
- F3. `Reactor.cloud-functions-docker` (per-function container, LRU keep-warm).
- F4. TypeScript runtime via embedded JS engine (G1) or Bun (G2+).
- F5. `Reactor.cloud-functions-fly` (Fly Machines adapter).
- F6. `Reactor.cloud-functions-lambda` (AWS adapter).
- F7. Streaming responses (SSE / chunked) supported through the trait.

### Phase G — Jobs
*Goal: trigger.dev–style durable jobs on Postgres `SKIP LOCKED`.*

- G1. Job runtime + step recorder (state machine in `_Reactor.cloud_jobs` schema).
- G2. Cron scheduler.
- G3. Retry policy + exponential backoff + dead-letter queue.
- G4. `ctx.waitFor`, `ctx.waitUntil`, `ctx.sleep` — pause-and-resume primitives.
- G5. `Reactor.cloud jobs runs` UI in CLI for inspecting executions.

### Phase H — Sites
*Goal: ship a Next.js app or a pile of HTML with one command.*

- H1. Static site host: `Reactor.cloud sites deploy ./dist`.
- H2. Domain + TLS (Let's Encrypt on G2; CDN-managed on G3).
- H3. Bundle format ("Reactor.cloud Site Bundle" — static + manifest of function routes).
- H4. Next.js adapter (build → bundle).
- H5. SvelteKit, Astro, Nuxt adapters.
- H6. ISR + on-demand revalidation (lean on Functions for the regen path).
- H7. Preview deployments per branch.

### Phase I — Tauri (G1)
*Goal: download a 30 MB binary, double-click, develop a project locally.*

- I1. Tauri shell with project explorer.
- I2. Embed `Reactor.cloud-server` in-process; bind to `127.0.0.1:<random>`.
- I3. Built-in log viewer + DB inspector + storage browser.
- I4. "Deploy to Reactor.cloud Cloud" / "Deploy to my Fly account" buttons that shell out to the CLI.
- I5. Auto-update via Tauri's updater.

### Phase J — Reactor.cloud Cloud (G3c, hosted)
*Goal: become the easiest place to run a Reactor.cloud project, billed as one product.*

- J1. Multi-tenant control plane (each customer project = isolated namespace).
- J2. Firecracker-based function runtime (Vercel-Sandbox-shaped).
- J3. Per-region database provisioning (Postgres + per-tenant schema or per-tenant DB).
- J4. CDN for sites.
- J5. Unified billing: gateway tokens + function GB-s + storage GB-mo + DB rows. Same `credits_micro` model as the LLM gateway.

### Phase K — Enterprise self-host (G3b)
*Goal: an enterprise customer can `helm install Reactor.cloud` into their own EKS.*

- K1. Helm chart for `Reactor.cloud-server`.
- K2. Lambda + Step Functions adapters.
- K3. RDS provisioning helpers.
- K4. Documentation and a reference architecture.

---

## 11. Open Architectural Questions

Things to decide before they become regret:

1. **Function runtime choice for TS in G1.** `boa` is pure Rust but slow. `rquickjs` is fast but C-bindings. Bun-as-subprocess is fastest but adds a dependency. Lean: `rquickjs` for Tauri, Bun for G2+.
2. **SQLite vs libsql.** libsql adds replication and a network protocol; if we ever want "G1 syncs to G3c when online" libsql is the path. Worth committing to early.
3. **Realtime delivery contract.** At-least-once vs at-most-once. We've said at-least-once; this means consumers must dedupe. Are we OK shipping that complexity to users?
4. **Permission DSL syntax.** Match Postgres RLS verbatim, or design something nicer (Cedar-style)? Compatibility wins; we'll pay for novelty later.
5. **Where does the dashboard live?** `Reactor.cloud-web` (the existing Next.js app) extends to manage all capabilities, or each capability ships its own UI in Tauri? Probably both with shared components.
6. **Multi-region in G3c.** Per-project region pinning (like `SPEC.md` §3 for the gateway), or project-can-span-regions with read replicas? Pinning is simpler; replicas are the eventual ask.
7. **Pricing model.** Token usage + GB-s + GB-mo is intuitive but Vercel-style. A "credits cover everything" model is simpler for agents to reason about. Lean toward credits.
8. **Admin/audit log surface.** Defer until v2; design the event shape now so we don't have to migrate later.

---

## 12. Non-Goals (explicit)

To keep the surface small enough for agents to reason about, the BaaS will *not* ship at v1 of the BaaS:

- A visual database editor (CLI + generated TS types are the v1 DX).
- A workflow visual builder (jobs are code).
- Email sending as a capability (use Resend; provide a thin SDK helper, not a runtime).
- Analytics / product metrics (defer to PostHog or build later).
- Search (defer to pgvector + a `search()` RPC, then native later).
- Vector search as a first-class capability (it's a Postgres table with `vector` type; not a separate domain).
- A managed git hosting story (deploy from local or from any git CI).

---

## 13. North-Star Quality Bars

These are the thresholds we hold ourselves to before each capability is "shipped":

| Bar | Threshold |
|---|---|
| Cold-boot Tauri app | < 2 seconds to a usable UI |
| `Reactor.cloud dev` startup | < 1 second to listening on the project's port |
| `Reactor.cloud deploy` to G3a | < 30 seconds for a 1-function, 1-page project |
| Function cold start (G3a) | < 200 ms p95 |
| Data API p99 | < 25 ms for a single-row select on a 1M-row table |
| Identity sign-in p99 | < 100 ms (excluding network) |
| CLI binary size | < 30 MB |
| Tauri binary size | < 50 MB |

If we miss these, the agent-first promise dies.

---

*End of architecture spec. Update via PR; this file describes intent, `SPEC.md` describes what's actually being built today.*


reactor/
├── Cargo.toml                     # workspace root
├── rust-toolchain.toml
├── crates/
│   ├── reactor-core/              # shared types, errors, config, ID types
│   ├── reactor-auth/              # the auth library (the medular piece)
│   │   ├── src/
│   │   │   ├── lib.rs             # public API + IdentityProvider trait
│   │   │   ├── router.rs          # axum Router::new() factory
│   │   │   ├── routes/            # one module per /auth/v1/* endpoint group
│   │   │   ├── store/             # IdentityStore trait + pg/sqlite impls
│   │   │   ├── token/             # JWT (RS256) + JWKS + rotation
│   │   │   ├── password/          # argon2id
│   │   │   ├── mfa/totp.rs
│   │   │   ├── oauth/             # Google, GitHub (later)
│   │   │   ├── tenancy/           # users, orgs, memberships, roles
│   │   │   ├── permissions/       # permission DSL + evaluator
│   │   │   └── verify.rs          # token-verification API for other capabilities
│   │   └── migrations/            # sqlx migrations
│   └── reactor-auth-server/       # bin: standalone `reactor-auth-server` on its own port
└── ReactorCloud_spec.md