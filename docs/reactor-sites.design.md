# `reactor-sites` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Sixth crate of the Reactor.cloud BaaS. Owns the Sites capability per `docs/ReactorCloud_spec.md` §2/§3/§6.3/§10-H.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, bundle format, framework adapters, routing, schema, policy integration — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Provide **app hosting (Next.js, Hono, SvelteKit, etc.) and static-file hosting** as a thin orchestration layer over `reactor-functions` and `reactor-storage`. A site is *not* a new compute primitive — it is a composer.
2. Use a **Vercel Build Output API-shaped bundle format** so framework adapters compile to a known target and the platform never becomes a bespoke framework host.
3. Expose a **`FrameworkAdapter` trait** (build-time, client-side) and a **`SiteHost` trait** (runtime, server-side) so topologies can vary (local dev, single VPS, managed cloud, enterprise self-host) without changing the bundle format.
4. **Reuse `reactor-functions` runtimes unchanged**: a site's SSR functions are regular function bundles, deployed via the functions admin API, invoked via the functions invoke endpoint.
5. **Reuse `reactor-storage`** for static assets: static files live in a system bucket `_reactor_sites`, served via signed URLs or proxied depending on topology.
6. **Preview deployments first-class**: every deployment has its own asset namespace and functions; previews are deployments without promotion, auto-resolvable at `{deployment_id}.preview.<site>.reactor.app`.
7. Support **ISR (Incremental Static Regeneration) and on-demand revalidation** via `reactor-cache` with a Postgres backstop, orchestrated by `reactor-jobs` background revalidation.
8. Support **custom domains with TLS** via ACME (G1/G2) or CDN handoff (G3), with certificate renewal driven by `reactor-jobs`.
9. Be the only crate allowed to touch the `_reactor_sites.*` Postgres metadata schema.

## 2. Non-goals (v0)

- **Edge-based router** (Cloudflare Worker / CloudFront Function) — v0 uses origin-based routing; manifest is kept edge-compatible for v0.2.
- **Per-route function splitting** — one function per site by default; adapter overrides at semantic seams (RSC/SSR, API, ISR revalidate), never arbitrary per-route splitting.
- **Managed Git integration** (push-to-deploy) — deploy via CLI or API; Git integration is a Sites SDK / CLI concern, not the server.
- **Preview comments UI** — previews are deployments; commenting is a dashboard feature, not Sites.
- **Built-in image optimization** — use reactor-storage signed URLs with CDN transforms; `reactor-images` is a potential v0.2 capability.
- **Analytics / Web Vitals** — defer to third-party or build `reactor-analytics` later.
- **A/B testing framework** — use policies or edge config; not built into Sites.
- **WebSocket origins** — Sites serves HTTP; WebSocket is a `reactor-realtime` capability.
- **Multi-region static** — v0 stores assets in a single region; multi-region replication is v0.2.
- **Container-image deployments** — Sites uses function bundles, not arbitrary Docker images.
- **SvelteKit / Astro / Nuxt adapters** — Next.js, Hono, and static ship at v0; others are v0.2.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, IDs, AuthClient trait
├── reactor-policy/                # (existing) shared policy engine
├── reactor-auth/                  # (existing)
├── reactor-data/                  # (existing)
├── reactor-storage/               # (existing) — static assets live here
├── reactor-functions/             # (existing) — SSR functions live here
├── reactor-cache/                 # (existing from jobs) — ISR cache
├── reactor-jobs/                  # (existing) — ACME renewal, ISR revalidation
│
├── reactor-sites/                 # the sites library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_sites.*
│   │   ├── 001_metadata.sql
│   │   ├── 002_routes.sql
│   │   ├── 003_domains.sql
│   │   ├── 004_policies.sql
│   │   ├── 005_isr.sql
│   │   └── 006_audit.sql
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # SitesConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # SitesState, SiteCtx
│       ├── error.rs               # SitesError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs
│       │   ├── serve.rs           # public serve plane (the hot path)
│       │   ├── admin.rs           # site CRUD
│       │   ├── deployments.rs     # bundle upload, promote, rollback
│       │   ├── domains.rs         # custom domain management
│       │   ├── revalidate.rs      # ISR purge endpoint
│       │   └── logs.rs            # SSE log tail (router + function logs)
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   ├── auth.rs            # bearer + X-Reactor-Org → SiteCtx (admin plane)
│       │   └── host_resolver.rs   # Host header → site_id (serve plane)
│       │
│       ├── bundle/
│       │   ├── mod.rs             # SiteBundle, Manifest types
│       │   ├── manifest.rs        # manifest.json schema + validation
│       │   ├── upload.rs          # chunked upload, static → storage, functions → functions API
│       │   └── verify.rs          # SHA256 verification
│       │
│       ├── framework/             # build-time adapters (also usable server-side for git-push)
│       │   ├── mod.rs             # FrameworkAdapter trait
│       │   ├── detect.rs          # framework detection logic
│       │   ├── static_site.rs     # StaticAdapter
│       │   ├── hono.rs            # HonoAdapter
│       │   └── nextjs.rs          # NextjsAdapter
│       │
│       ├── route/
│       │   ├── mod.rs
│       │   ├── matcher.rs         # path-to-regexp style matching
│       │   ├── decision.rs        # RouteDecision enum
│       │   └── table.rs           # RouteTable built from manifest
│       │
│       ├── dispatch/
│       │   ├── mod.rs
│       │   ├── static_dispatch.rs # → reactor-storage signed URL or proxy
│       │   ├── function_dispatch.rs # → reactor-functions HTTP invoke
│       │   └── prerender.rs       # serve cached HTML, trigger revalidation
│       │
│       ├── isr/
│       │   ├── mod.rs
│       │   ├── cache.rs           # ISR cache via reactor-cache
│       │   └── revalidate.rs      # on-demand revalidation logic
│       │
│       ├── domain/
│       │   ├── mod.rs
│       │   ├── verify.rs          # domain verification (DNS TXT or HTTP)
│       │   └── acme.rs            # Let's Encrypt via rustls-acme (G2, feature-gated)
│       │
│       ├── store/
│       │   ├── mod.rs             # SitesStore trait
│       │   └── postgres.rs        # PgSitesStore
│       │
│       └── audit.rs               # admin-event audit writer
│
└── reactor-sites-server/          # standalone bin
    ├── Cargo.toml
    └── src/
        ├── main.rs                # axum bind + serve + admin
        └── cli/
            ├── mod.rs
            └── doctor.rs          # connectivity diagnostics
```

Conventions:
- `reactor-sites` depends on `reactor-core`, `reactor-policy`, `reactor-cache`; it depends on `reactor-functions` and `reactor-storage` **only as HTTP clients**, never as library imports.
- The Postgres adapter lives inside `reactor-sites/src/store/postgres.rs`.
- Cargo features: `framework-static` (default), `framework-hono` (default), `framework-nextjs` (default), `domain-acme` (default off — G2 only).

---

## 4. Core types

### 4.1 IDs

All IDs are `ReactorId` (UUIDv7) from `reactor-core`. Sites-specific types:

| Type | Rust | Notes |
|---|---|---|
| `SiteId` | `ReactorId` | Primary key for sites |
| `SiteDeploymentId` | `ReactorId` | Primary key for site deployments |
| `RouteId` | `ReactorId` | Primary key for deployment routes |
| `DomainId` | `ReactorId` | Primary key for custom domains |
| `Framework` | `enum { Static, Hono, Nextjs, ... }` | Framework that produced the bundle |
| `DeploymentStatus` | `enum { Pending, Ready, Failed, Destroyed }` | Deployment lifecycle |
| `RouteKind` | `enum { Static, Function, Redirect, Prerender }` | How to serve a matched route |

### 4.2 `SiteCtx` (request-local, admin plane)

Constructed by middleware for admin routes:

```rust
// reactor-sites/src/state.rs
#[derive(Debug, Clone)]
pub struct SiteCtx {
    pub auth:       AuthCtx,
    pub request_id: String,
    pub org_id:     OrgId,
}

impl SiteCtx {
    pub fn user_id(&self) -> Option<&UserId> { self.auth.user_id() }
    pub fn active_org(&self) -> &OrgId { &self.org_id }
    pub fn has_permission(&self, perm: &str) -> bool {
        self.auth.has_permission(perm)
    }
}
```

The serve plane does **not** construct a `SiteCtx` — it resolves site by `Host` header and applies per-site policies without requiring a JWT.

### 4.3 `FrameworkAdapter` trait (build-time)

Lives client-side (CLI) and optionally server-side (for Git-push deploys later):

```rust
// reactor-sites/src/framework/mod.rs
#[async_trait]
pub trait FrameworkAdapter: Send + Sync {
    fn name(&self) -> Framework;
    
    /// Returns true if this adapter handles the given project directory.
    fn detect(&self, project_dir: &Path) -> bool;
    
    /// Build the project into a SiteBundle.
    async fn build(&self, project_dir: &Path, opts: &BuildOpts) -> Result<SiteBundle, SitesError>;
}

#[derive(Debug)]
pub struct BuildOpts {
    pub output_dir: PathBuf,
    pub env: HashMap<String, String>,
    pub node_version: Option<String>,
}

#[derive(Debug)]
pub struct SiteBundle {
    pub manifest: Manifest,
    pub static_dir: PathBuf,           // directory of static assets
    pub functions: Vec<FunctionBundle>, // reactor-functions bundles
    pub prerender: Option<PathBuf>,    // optional prerendered HTML directory
}

#[derive(Debug)]
pub struct FunctionBundle {
    pub name: String,                  // e.g. "ssr", "api", "isr-revalidate"
    pub manifest: FunctionManifest,    // reactor-functions manifest
    pub code_dir: PathBuf,
}
```

### 4.4 `SiteHost` trait (runtime, server-side)

```rust
// reactor-sites/src/dispatch/mod.rs
#[async_trait]
pub trait SiteHost: Send + Sync + 'static {
    /// Upload static assets for a deployment.
    async fn upload_static(
        &self,
        deployment_id: &SiteDeploymentId,
        files: impl Stream<Item = Result<(String, Bytes), io::Error>> + Send,
    ) -> Result<u64, SitesError>;
    
    /// Publish the route table for a deployment.
    async fn publish_routes(
        &self,
        deployment_id: &SiteDeploymentId,
        routes: &[DeploymentRoute],
    ) -> Result<(), SitesError>;
    
    /// Resolve a request to a route decision.
    async fn route(
        &self,
        site_id: &SiteId,
        deployment_id: &SiteDeploymentId,
        host: &str,
        path: &str,
        method: &http::Method,
    ) -> Result<RouteDecision, SitesError>;
    
    /// Purge a deployment's resources.
    async fn purge(&self, deployment_id: &SiteDeploymentId) -> Result<(), SitesError>;
}

#[derive(Debug, Clone)]
pub enum RouteDecision {
    StaticFile {
        storage_key: String,
        cache: CacheRules,
        content_type: Option<String>,
    },
    Function {
        function_id: FunctionId,
        sub_path: String,
    },
    Redirect {
        location: String,
        status: u16,
        permanent: bool,
    },
    Prerender {
        storage_key: String,
        revalidate_after: Option<Duration>,
        fallback: Option<Box<RouteDecision>>,
    },
    NotFound,
}

#[derive(Debug, Clone, Default)]
pub struct CacheRules {
    pub max_age: Option<Duration>,
    pub s_maxage: Option<Duration>,
    pub stale_while_revalidate: Option<Duration>,
    pub immutable: bool,
}
```

### 4.5 `SitesStore` trait

```rust
// reactor-sites/src/store/mod.rs
#[async_trait]
pub trait SitesStore: Send + Sync + 'static {
    // Site CRUD
    async fn create_site(&self, s: &NewSite) -> Result<Site, SitesError>;
    async fn get_site(&self, org: &OrgId, name: &str) -> Result<Option<Site>, SitesError>;
    async fn get_site_by_id(&self, id: &SiteId) -> Result<Option<Site>, SitesError>;
    async fn list_sites(&self, org: &OrgId) -> Result<Vec<Site>, SitesError>;
    async fn delete_site(&self, id: &SiteId) -> Result<(), SitesError>;
    
    // Deployments
    async fn create_deployment(&self, d: &NewDeployment) -> Result<SiteDeployment, SitesError>;
    async fn get_deployment(&self, id: &SiteDeploymentId) -> Result<Option<SiteDeployment>, SitesError>;
    async fn current_deployment(&self, site_id: &SiteId) -> Result<Option<SiteDeployment>, SitesError>;
    async fn promote_deployment(&self, id: &SiteDeploymentId) -> Result<(), SitesError>;
    async fn list_deployments(&self, site_id: &SiteId, limit: u32) -> Result<Vec<SiteDeployment>, SitesError>;
    async fn update_deployment_status(&self, id: &SiteDeploymentId, status: DeploymentStatus, detail: Option<&str>) -> Result<(), SitesError>;
    
    // Deployment routes
    async fn set_deployment_routes(&self, deployment_id: &SiteDeploymentId, routes: &[DeploymentRoute]) -> Result<(), SitesError>;
    async fn get_deployment_routes(&self, deployment_id: &SiteDeploymentId) -> Result<Vec<DeploymentRoute>, SitesError>;
    
    // Deployment functions (back-ref to reactor-functions)
    async fn add_deployment_function(&self, deployment_id: &SiteDeploymentId, function_id: &FunctionId, role: &str) -> Result<(), SitesError>;
    async fn get_deployment_functions(&self, deployment_id: &SiteDeploymentId) -> Result<Vec<DeploymentFunction>, SitesError>;
    
    // Custom domains
    async fn create_domain(&self, d: &NewDomain) -> Result<Domain, SitesError>;
    async fn get_domain(&self, host: &str) -> Result<Option<Domain>, SitesError>;
    async fn list_domains(&self, site_id: &SiteId) -> Result<Vec<Domain>, SitesError>;
    async fn update_domain_status(&self, id: &DomainId, status: DomainStatus, cert_ref: Option<&str>) -> Result<(), SitesError>;
    async fn delete_domain(&self, id: &DomainId) -> Result<(), SitesError>;
    
    // Site lookup by host (for serve plane)
    async fn get_site_by_host(&self, host: &str) -> Result<Option<(Site, SiteDeployment)>, SitesError>;
    
    // ISR cache
    async fn get_isr_entry(&self, site_id: &SiteId, path: &str) -> Result<Option<IsrCacheEntry>, SitesError>;
    async fn set_isr_entry(&self, entry: &IsrCacheEntry) -> Result<(), SitesError>;
    async fn invalidate_isr(&self, site_id: &SiteId, path_or_tag: &str) -> Result<u32, SitesError>;
    
    // Policies
    async fn get_site_policies(&self, site_id: &SiteId) -> Result<Vec<SitePolicy>, SitesError>;
    async fn upsert_policy(&self, p: &NewSitePolicy) -> Result<SitePolicy, SitesError>;
    async fn delete_policy(&self, id: &ReactorId) -> Result<(), SitesError>;
    
    // Audit
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), SitesError>;
}
```

### 4.6 No `SitesClient` trait

Same posture as `reactor-data` and `reactor-functions`: there is **no `SitesClient` trait** in `reactor-core`. Other capabilities (Jobs for ACME renewal) consume Sites via HTTP. If a unified Reactor binary needs in-process routing, `reactor_sites::router(state)` is embeddable behind `tower::Service`.

---

## 5. HTTP surface (v0)

Sites exposes **two request planes**: the public serve plane (handles end-user traffic) and the admin plane (manages sites, deployments, domains).

### 5.1 Health

```
GET    /sites/v1/health
       → 200 { "status": "ok", "version": "0.1.0", "frameworks": ["static", "hono", "nextjs"] }
```

### 5.2 Public serve plane (the hot path)

```
*      /*
       Host: <site-host>           # e.g. app.example.com, mysite.reactor.app, {deployment_id}.preview.mysite.reactor.app
       
       Flow:
       1. Host resolver middleware extracts host, looks up site + current_deployment
       2. For preview subdomain: parse deployment_id, use that deployment instead of current
       3. Route matcher finds matching DeploymentRoute by path + method
       4. Dispatcher executes RouteDecision:
          - StaticFile → redirect to signed URL (G3) or stream from storage (G1/G2)
          - Function → POST reactor-functions /fn/v1/{name}/{sub_path}
          - Redirect → return 301/302/307/308 with Location header
          - Prerender → serve cached HTML, trigger async revalidation if stale
       5. Per-site policies evaluated (e.g. password-protect preview)
       
       Response: whatever the static file / function / redirect returns
       
       → 200/2xx (static file or function response)
       → 3xx    (redirect)
       → 403    policy_denied (per-site policy blocked request)
       → 404    route_unmatched
       → 502    function_dispatch_failed
       → 503    deployment_not_ready

No Authorization header required on the serve plane. Per-site policies can require headers.
```

### 5.3 Admin: sites

```
POST   /sites/v1/_admin/sites
       Body: { "name": "my-app", "framework": "nextjs" }
       → 201 { site }
       Requires: sites:create

GET    /sites/v1/_admin/sites
       → 200 [ site, ... ]
       Lists sites in active org

GET    /sites/v1/_admin/sites/{name}
       → 200 { site, current_deployment, domains }

DELETE /sites/v1/_admin/sites/{name}
       → 204
       Requires: sites:{name}:admin
       Cascades: destroys deployments (including reactor-functions), removes domains
```

Site-name constraints: `^[a-z][a-z0-9-]{0,62}$` (lowercase, hyphens, 1–63 chars).

### 5.4 Admin: deployments

```
POST   /sites/v1/_admin/sites/{name}/deployments
       Content-Type: multipart/form-data
       Parts:
         - manifest: application/json (site manifest)
         - static: application/zip or application/x-tar (static assets)
         - functions[]: application/zip (one per function bundle, optional)
         - prerender: application/zip (prerendered HTML, optional)
       
       → 201 { deployment }
       Requires: sites:{name}:deploy
       
       Pipeline:
       1. Validate manifest schema
       2. Create deployment row (status=pending)
       3. Upload static assets to reactor-storage _reactor_sites/{deployment_id}/static/...
       4. For each function bundle:
          - Deploy via reactor-functions API (synthetic _internal function)
          - Store function_id in deployment_functions table
       5. Wait for all functions to report ready
       6. Insert deployment_routes from manifest
       7. Update deployment status=ready
       8. On any failure: status=failed with detail

POST   /sites/v1/_admin/sites/{name}/promote
       Body: { "deployment_id": "..." }
       → 200 { site }
       Requires: sites:{name}:deploy
       Atomic swap of current_deployment_id; new requests route to new deployment

POST   /sites/v1/_admin/sites/{name}/rollback
       Body: { "to_deployment_id": "..." } (optional; defaults to previous)
       → 200 { site }
       Requires: sites:{name}:admin

GET    /sites/v1/_admin/sites/{name}/deployments
       Query: ?limit=20
       → 200 [ deployment, ... ]
       Most recent first

GET    /sites/v1/_admin/sites/{name}/deployments/{deployment_id}
       → 200 { deployment, routes, functions }
```

Deploy and promote are **separate** operations. Deploy materializes the bundle and runs the function deployment cycle; promote swaps live traffic. Preview deployments are deployments without promotion.

### 5.5 Admin: custom domains

```
POST   /sites/v1/_admin/sites/{name}/domains
       Body: { "host": "app.example.com" }
       → 201 { domain, verification_instructions }
       Requires: sites:{name}:admin
       
       Verification: DNS TXT record or HTTP .well-known file
       Status: pending → verified → active (once TLS provisioned)

GET    /sites/v1/_admin/sites/{name}/domains
       → 200 [ domain, ... ]

DELETE /sites/v1/_admin/sites/{name}/domains/{host}
       → 204
       Requires: sites:{name}:admin
       Releases domain for reuse

POST   /sites/v1/_admin/sites/{name}/domains/{host}/verify
       → 200 { domain, status }
       Triggers verification check
```

### 5.6 Admin: ISR revalidation

```
POST   /sites/v1/_admin/sites/{name}/revalidate
       Body: { "paths": ["/blog/post-1", "/blog/post-2"] }
             { "tags": ["blog"] }
       → 200 { invalidated_count: 2 }
       Requires: sites:{name}:admin or internal revalidation token
       
       Purges ISR cache entries matching paths or tags.
       Triggers background re-render via reactor-jobs.
```

### 5.7 Admin: logs

```
GET    /sites/v1/_admin/sites/{name}/logs
       Query: ?since=2026-05-14T00:00:00Z&limit=200&follow=1
       → 200 (text/event-stream when follow=1, otherwise application/json)
       
       Merges:
       - Router-plane logs (request path, response status, latency)
       - Per-function logs (subscribes to reactor-functions logs for each deployment_function)
       
       SSE event shape:
         event: log
         data: { "ts": "...", "level": "info", "deployment_id": "...", "source": "router|function:ssr", "message": "..." }
       
       Requires: sites:{name}:logs
```

### 5.8 Headers

| Header | Direction | Meaning |
|---|---|---|
| `Authorization: Bearer <jwt>` | inbound | Required for admin routes; not required for serve plane |
| `X-Reactor-Org: <ref>` | inbound | Active org override for admin routes |
| `X-Request-Id` | both | Generated if absent; echoed in response |
| `X-Reactor-Site` | response | `{name}@{deployment_version}` |
| `X-Reactor-Cache` | response | `HIT`, `MISS`, `STALE`, `BYPASS` for ISR |
| `X-Reactor-Duration-Ms` | response | Total serve duration |
| `X-Reactor-Revalidation-Token` | inbound→sites | Internal token for function-driven revalidation |

### 5.9 Error envelope

Same shape as other capabilities:

```json
{
  "error": {
    "code": "route_unmatched",
    "message": "No route matched path '/api/unknown'.",
    "status": 404,
    "request_id": "req_01HZ...",
    "details": {
      "site": "my-app",
      "deployment_id": "dep_01HZ...",
      "path": "/api/unknown"
    }
  }
}
```

Error codes: `site_not_found`, `deployment_not_found`, `deployment_not_ready`, `bundle_invalid`, `manifest_invalid`, `bundle_too_large`, `static_upload_failed`, `function_deploy_failed`, `route_unmatched`, `function_dispatch_failed`, `domain_taken`, `domain_unverified`, `domain_verification_failed`, `policy_denied`, `revalidate_failed`, `acme_challenge_failed`.

---

## 6. Database schema (`_reactor_sites`)

```sql
create schema if not exists _reactor_sites;

-- 6.1 Sites
create table _reactor_sites.sites (
  id                       uuid primary key,
  org_id                   uuid not null,
  name                     citext not null,
  framework                text not null,            -- 'static' | 'hono' | 'nextjs' | ...
  current_deployment_id    uuid,                     -- FK; null until first promote
  created_at               timestamptz not null default now(),
  updated_at               timestamptz not null default now(),
  unique (org_id, name)
);
create index on _reactor_sites.sites (org_id);

-- 6.2 Deployments
create table _reactor_sites.deployments (
  id                       uuid primary key,
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  version                  bigint not null,          -- monotonic per site
  manifest_json            jsonb not null,
  status                   text not null default 'pending',  -- pending, ready, failed, destroyed
  status_detail            text,
  static_asset_count       integer not null default 0,
  static_asset_bytes       bigint not null default 0,
  deployed_at              timestamptz not null default now(),
  deployed_by_user_id      uuid,
  unique (site_id, version)
);
create index on _reactor_sites.deployments (site_id, deployed_at desc);
create index on _reactor_sites.deployments (status) where status in ('pending', 'failed');

alter table _reactor_sites.sites
  add constraint fk_current_deployment
  foreign key (current_deployment_id) references _reactor_sites.deployments(id) on delete set null;

-- 6.3 Deployment routes (ordered route table)
create table _reactor_sites.deployment_routes (
  id                       uuid primary key,
  deployment_id            uuid not null references _reactor_sites.deployments(id) on delete cascade,
  pattern                  text not null,            -- path pattern, e.g. "/api/:path*", "/:slug"
  method_filter            text,                     -- null = any method; "GET,POST" = specific
  route_kind               text not null,            -- 'static' | 'function' | 'redirect' | 'prerender'
  target_ref               text not null,            -- storage key | function_id | redirect URL | prerender storage key
  cache_rules_json         jsonb not null default '{}',
  priority                 integer not null default 0,  -- higher = matched first
  created_at               timestamptz not null default now()
);
create index on _reactor_sites.deployment_routes (deployment_id, priority desc);

-- 6.4 Deployment functions (back-ref to reactor-functions)
create table _reactor_sites.deployment_functions (
  deployment_id            uuid not null references _reactor_sites.deployments(id) on delete cascade,
  function_id              uuid not null,            -- FK to _reactor_functions.functions conceptually
  role                     text not null,            -- 'ssr', 'api', 'isr-revalidate', etc.
  created_at               timestamptz not null default now(),
  primary key (deployment_id, function_id)
);
create index on _reactor_sites.deployment_functions (function_id);

-- 6.5 Custom domains
create table _reactor_sites.domains (
  id                       uuid primary key,
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  host                     text not null unique,     -- e.g. "app.example.com"
  status                   text not null default 'pending',  -- pending, verified, active, failed
  verification_token       text not null,            -- for DNS TXT or HTTP challenge
  verification_method      text not null default 'dns',  -- 'dns' | 'http'
  tls_cert_ref             text,                     -- reference to cert in storage (G2) or CDN (G3)
  tls_expires_at           timestamptz,
  verified_at              timestamptz,
  created_at               timestamptz not null default now()
);
create index on _reactor_sites.domains (site_id);
create index on _reactor_sites.domains (tls_expires_at) where status = 'active';

-- 6.6 Per-site policies
create table _reactor_sites.policies (
  id                       uuid primary key,
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  name                     text not null,
  using_expr_json          jsonb,                    -- PolicyExpr; evaluated for serve-plane requests
  raw_text                 text not null,
  sha256                   bytea not null,
  created_at               timestamptz not null default now(),
  unique (site_id, name)
);
create index on _reactor_sites.policies (site_id);

-- 6.7 ISR cache (Postgres backstop; primary cache in reactor-cache)
create table _reactor_sites.isr_cache (
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  path                     text not null,
  deployment_id            uuid not null references _reactor_sites.deployments(id) on delete cascade,
  body_storage_key         text not null,
  content_type             text,
  etag                     text,
  tags                     text[] not null default '{}',
  revalidate_after         interval,
  last_revalidated_at      timestamptz not null default now(),
  created_at               timestamptz not null default now(),
  primary key (site_id, path)
);
create index on _reactor_sites.isr_cache (site_id, tags) using gin (tags);
create index on _reactor_sites.isr_cache (last_revalidated_at) where revalidate_after is not null;

-- 6.8 Audit events
create table _reactor_sites.audit_events (
  id                       uuid primary key,
  ts                       timestamptz not null default now(),
  actor_user_id            uuid,
  actor_apikey_id          uuid,
  org_id                   uuid,
  site_id                  uuid,
  deployment_id            uuid,
  domain_id                uuid,
  event_type               text not null,
  details                  jsonb not null default '{}',
  request_id               text not null
);
create index on _reactor_sites.audit_events (org_id, ts desc);
create index on _reactor_sites.audit_events (site_id, ts desc);

-- 6.9 Invocations (sampled serve-plane requests)
create table _reactor_sites.invocations (
  id                       uuid primary key,
  site_id                  uuid not null,
  deployment_id            uuid not null,
  org_id                   uuid not null,
  request_id               text not null,
  method                   text not null,
  path                     text not null,
  host                     text not null,
  route_kind               text not null,
  status_code              integer not null,
  duration_ms              integer not null,
  cache_status             text,                     -- HIT, MISS, STALE, BYPASS
  bytes_out                bigint not null default 0,
  created_at               timestamptz not null default now()
);
create index on _reactor_sites.invocations (site_id, created_at desc);
create index on _reactor_sites.invocations (org_id, created_at desc);
-- Sampling: only 1% of requests logged by default (configurable)
```

### 6.10 Role grants

`_reactor_sites` is **not** readable by user application roles. `reactor-sites-server` connects with a dedicated role that has:
- `USAGE` on `_reactor_sites` schema
- Full DML on all tables in `_reactor_sites`
- No access to user data schemas

---

## 7. Bundle format

A Reactor Site Bundle follows the Vercel Build Output API shape. It is produced by a `FrameworkAdapter` and uploaded to the sites server.

### 7.1 Directory structure

```
.reactor-bundle/
├── manifest.json                     # required, schema below
├── static/                           # required, static assets
│   ├── _next/static/...              # immutable, fingerprinted assets
│   ├── public/...                    # user-facing static files
│   └── favicon.ico
├── functions/                        # optional, SSR/API function bundles
│   ├── ssr.fn/                       # each subdir is a reactor-functions bundle
│   │   ├── manifest.json             # reactor-functions manifest
│   │   └── code/
│   │       └── index.ts
│   ├── api.fn/
│   │   ├── manifest.json
│   │   └── code/
│   │       └── index.ts
│   └── isr-revalidate.fn/
│       ├── manifest.json
│       └── code/
│           └── index.ts
└── prerender/                        # optional, prerendered HTML
    ├── index.html
    ├── blog/
    │   ├── post-1.html
    │   └── post-2.html
    └── .prerender-manifest.json      # maps paths to revalidate intervals + tags
```

### 7.2 Manifest schema

```json
{
  "name": "my-app",
  "version": 7,
  "framework": "nextjs",
  
  "routes": [
    {
      "pattern": "/_next/static/:path*",
      "kind": "static",
      "target": "_next/static/$path",
      "cache": { "maxAge": 31536000, "immutable": true }
    },
    {
      "pattern": "/api/:path*",
      "kind": "function",
      "target": "api",
      "methods": ["GET", "POST", "PUT", "DELETE"]
    },
    {
      "pattern": "/blog/:slug",
      "kind": "prerender",
      "target": "prerender/blog/$slug.html",
      "fallback": { "kind": "function", "target": "ssr" },
      "revalidate": 3600,
      "tags": ["blog"]
    },
    {
      "pattern": "/:path*",
      "kind": "function",
      "target": "ssr"
    }
  ],
  
  "functions": {
    "ssr": {
      "runtime": "bun",
      "entrypoint": "code/index.ts",
      "limits": { "timeout_ms": 30000, "memory_mb": 512 }
    },
    "api": {
      "runtime": "bun",
      "entrypoint": "code/index.ts",
      "limits": { "timeout_ms": 30000, "memory_mb": 256 }
    },
    "isr-revalidate": {
      "runtime": "bun",
      "entrypoint": "code/index.ts",
      "limits": { "timeout_ms": 60000, "memory_mb": 512 }
    }
  },
  
  "redirects": [
    { "source": "/old-page", "destination": "/new-page", "permanent": true }
  ],
  
  "headers": [
    {
      "pattern": "/api/:path*",
      "headers": { "Access-Control-Allow-Origin": "*" }
    }
  ],
  
  "env_keys": ["NEXT_PUBLIC_API_URL"],
  "secret_keys": ["DATABASE_URL"]
}
```

### 7.3 Validation rules

- `version` is server-assigned on deploy (client value ignored).
- `framework` must match the site's configured framework.
- `routes` are ordered; first match wins.
- `routes[].target` for `function` kind must reference a key in `functions`.
- `routes[].target` for `static` kind must be a relative path under the site's static root (the dispatcher prefixes `{deployment_id}/static/` internally).
- Each `functions` entry must have a corresponding `functions/{name}.fn/` directory in the bundle.
- Total static asset count capped at 50,000 entries at v0.
- Individual function bundles inherit the 50 MiB reactor-functions cap.

### 7.4 Bundle storage

Static assets are uploaded to reactor-storage's `_reactor_sites` system bucket:

```
_reactor_sites/
└── {deployment_id}/
    ├── static/
    │   ├── _next/static/chunks/...
    │   └── ...
    └── prerender/
        ├── index.html
        └── blog/...
```

Function bundles are deployed via reactor-functions API and stored in the `_reactor_functions` bucket. Sites maintains back-references in `_reactor_sites.deployment_functions`.

---

## 8. Framework adapters

### 8.1 `StaticAdapter`

Simplest case: copies a directory of static files.

**Detection**: `package.json` is absent, or `package.json` has no `build` script and the project contains only HTML/CSS/JS files.

**Build**:
1. Copy project directory to `static/`
2. Generate manifest with a single catch-all static route
3. No functions

**Manifest output**:
```json
{
  "framework": "static",
  "routes": [
    { "pattern": "/:path*", "kind": "static", "target": "$path" }
  ],
  "functions": {}
}
```

### 8.2 `HonoAdapter`

Single-function SSR site for Hono applications.

**Detection**: `package.json` has `hono` as a dependency and an `index.ts` or `src/index.ts` exporting a Hono app.

**Build**:
1. Bundle with Bun: `bun build ./src/index.ts --outdir=.reactor-bundle/functions/ssr.fn/code/`
2. Copy any `public/` directory to `static/`
3. Generate manifest with static route for public assets, catch-all function route

**Manifest output**:
```json
{
  "framework": "hono",
  "routes": [
    { "pattern": "/public/:path*", "kind": "static", "target": "public/$path" },
    { "pattern": "/:path*", "kind": "function", "target": "ssr" }
  ],
  "functions": {
    "ssr": { "runtime": "bun", "entrypoint": "code/index.js", "limits": { "timeout_ms": 30000 } }
  }
}
```

### 8.3 `NextjsAdapter`

Full Next.js App Router support.

**Detection**: `package.json` has `next` as a dependency.

**Build**:
1. Run `next build` with standalone output mode
2. Walk `.next/standalone/` and `.next/static/`
3. Emit 1–3 functions at semantic seams:
   - `ssr`: RSC/SSR handler
   - `api`: API route handlers (if `app/api/` exists)
   - `isr-revalidate`: ISR revalidation handler (if any routes use `revalidate`)
4. Extract prerendered HTML from `.next/server/app/` to `prerender/`
5. Generate `.prerender-manifest.json` with revalidate intervals and tags

**Function splitting rationale**: different timeout/memory profiles, not per-route. Never split arbitrarily per URL.

**Manifest output**:
```json
{
  "framework": "nextjs",
  "routes": [
    { "pattern": "/_next/static/:path*", "kind": "static", "target": "_next/static/$path", "cache": { "immutable": true } },
    { "pattern": "/api/:path*", "kind": "function", "target": "api" },
    { "pattern": "/blog/:slug", "kind": "prerender", "target": "prerender/blog/$slug.html", "fallback": { "kind": "function", "target": "ssr" }, "revalidate": 3600 },
    { "pattern": "/:path*", "kind": "function", "target": "ssr" }
  ],
  "functions": {
    "ssr": { "runtime": "bun", "entrypoint": "code/server.js", "limits": { "timeout_ms": 30000, "memory_mb": 512 } },
    "api": { "runtime": "bun", "entrypoint": "code/server.js", "limits": { "timeout_ms": 30000, "memory_mb": 256 } },
    "isr-revalidate": { "runtime": "bun", "entrypoint": "code/server.js", "limits": { "timeout_ms": 60000, "memory_mb": 512 } }
  }
}
```

---

## 9. Runtime topology

### 9.1 Origin-based router (v0)

At v0, all request routing happens at the origin (reactor-sites-server). This simplifies deployment and debugging.

```
Request → Host resolver → Site lookup → Route matcher → Dispatcher
                                                            │
                                       ┌────────────────────┼────────────────────┐
                                       ▼                    ▼                    ▼
                                  Static file          Function              Prerender
                                       │                    │                    │
                                       ▼                    ▼                    ▼
                              reactor-storage      reactor-functions        ISR cache
                            (signed URL / proxy)       (invoke)           (serve / regen)
```

### 9.2 Topology per grade

| Grade | Static served by | Functions runtime | Router location |
|---|---|---|---|
| G1 (Tauri, dev) | reactor-storage Fs + proxy | wasm / bun | in-process axum |
| G2 (single VPS) | reactor-storage Fs + cache headers | bun | reactor-sites-server |
| G3a (Fly/managed) | reactor-storage S3/R2 + Cloudflare CDN | bun (Fly machines) | origin (v0) |
| G3b (enterprise AWS) | S3 + CloudFront | lambda | origin (v0) |
| G3c (k8s, future) | object store + CDN | KubernetesRuntime adapter | origin or ingress |

### 9.3 Edge-compatible manifest

The manifest format is kept edge-compatible: a future v0.2 can emit a Cloudflare Worker or CloudFront Function from the manifest to handle routing at the edge, with only SSR traffic hitting origin. This is not built at v0.

### 9.4 K8s topology (deferred)

The k8s case is **not a Sites concern** — it's a new `FunctionRuntime` adapter in `reactor-functions` (e.g., `KubernetesRuntime` that creates a Deployment + Service per function). Sites inherits it transparently because Sites only talks to reactor-functions via HTTP.

---

## 10. Deployment pipeline

### 10.1 Sequence

```
CLI                              sites-server                     storage / functions
───                              ────────────                     ───────────────────

1. Build site bundle locally
   (framework adapter)
                        ──upload (multipart)──▶
                                               2. Validate manifest
                                               3. Create deployment row (pending)
                                               4. For each static asset:
                                                                    ──put_object──▶ _reactor_sites/{dep_id}/...
                                               5. For each function bundle:
                                                                    ──POST /fn/v1/_admin/functions──▶
                                                                    (synthetic internal function:
                                                                     site-{slug}-{role}, _internal=true)
                                               6. Wait for all functions ready
                                               7. Insert deployment_routes
                                               8. deployment.status = ready
                        ◀─201 deployment─────

9. Optionally: POST /promote
                        ──promote──▶
                                               10. Atomic swap current_deployment_id
                                               11. Purge route cache (if any)
                        ◀─200 site─────
```

### 10.2 Synthetic internal functions

Sites creates functions in reactor-functions with a special naming convention and metadata:

- Name: `_site-{site_slug}-{role}` (e.g., `_site-my-app-ssr`)
- Metadata: `_internal: true` flag in function row
- Hidden: `reactor functions list` filters out `_internal=true` functions

This keeps function lifecycle management inside reactor-functions while hiding implementation details from users.

### 10.3 Atomic promote

Promote is a single SQL UPDATE that swaps `sites.current_deployment_id`. Route resolution always reads `current_deployment_id` from the site row, so traffic atomically shifts to the new deployment.

In-flight requests to the old deployment continue to completion — function invocations are not interrupted. The route table for the old deployment remains in the database until the deployment is garbage-collected (v0.2).

---

## 11. Route resolution

### 11.1 Matcher

Routes are matched using a path-to-regexp style algorithm:

- `:param` matches a single segment
- `:param*` matches zero or more segments (greedy)
- `:param+` matches one or more segments (greedy)
- Literal segments match exactly
- Query strings are not part of the path match

Routes are sorted by priority (higher first), then by specificity (more literal segments first).

### 11.2 Resolution flow

```rust
fn resolve(routes: &[DeploymentRoute], path: &str, method: &Method) -> RouteDecision {
    for route in routes.iter().sorted_by_key(|r| -r.priority) {
        if let Some(captures) = route.pattern.match(path) {
            if route.method_filter_matches(method) {
                return route.to_decision(captures);
            }
        }
    }
    RouteDecision::NotFound
}
```

### 11.3 Dispatch

| RouteKind | Dispatch |
|---|---|
| `Static` | Resolve storage key, return redirect to signed URL (G3) or stream bytes (G1/G2) with `Cache-Control` from `cache_rules` |
| `Function` | POST to `reactor-functions /fn/v1/{function_name}/{sub_path}`, stream response back |
| `Redirect` | Return 301/302/307/308 with `Location` header |
| `Prerender` | Check ISR cache: if fresh, serve cached HTML; if stale, serve stale + trigger async revalidation; if miss, execute fallback |

---

## 12. ISR and on-demand revalidation

### 12.1 ISR cache

ISR (Incremental Static Regeneration) cache entries are stored in:
1. **reactor-cache** (primary, in-memory with Postgres backing)
2. **`_reactor_sites.isr_cache`** (persistent backstop)

Cache key: `(site_id, path)`

### 12.2 Serve flow

```
Request for /blog/post-1 → matches prerender route
      ▼
ISR cache lookup
  │
  ├─ HIT + fresh → serve cached HTML, X-Reactor-Cache: HIT
  │
  ├─ HIT + stale → serve cached HTML, X-Reactor-Cache: STALE
  │                 └─ enqueue background revalidation (reactor-jobs)
  │
  └─ MISS → execute fallback (function)
            └─ cache response if cacheable
               └─ X-Reactor-Cache: MISS
```

### 12.3 On-demand revalidation

Functions can trigger revalidation by calling back to Sites:

```typescript
// Inside SSR function
await fetch(`${process.env.REACTOR_SITES_INTERNAL_URL}/sites/v1/_admin/sites/${siteName}/revalidate`, {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${process.env.REACTOR_SITES_REVALIDATION_TOKEN}`,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({ paths: ['/blog/post-1'] })
});
```

The revalidation token is an internal secret shared between sites-server and its functions.

### 12.4 Background revalidation

When a stale-while-revalidate request triggers revalidation:
1. Sites server enqueues a revalidation job via reactor-jobs
2. Job invokes the ISR revalidation function with the path
3. Function renders the page, returns HTML
4. Sites server caches the new HTML, updates `last_revalidated_at`

---

## 13. Custom domains and TLS

### 13.1 Domain lifecycle

```
Create domain (status=pending)
      ▼
User adds DNS TXT record or HTTP challenge file
      ▼
POST /verify (or background job polls)
      ▼
Verification succeeds → status=verified
      ▼
ACME challenge (G2) or CDN configuration (G3)
      ▼
TLS certificate issued → status=active
```

### 13.2 Verification methods

**DNS TXT record**:
```
_reactor-verify.app.example.com TXT "reactor-site-verification={token}"
```

**HTTP challenge**:
```
GET http://app.example.com/.well-known/reactor-verify → {token}
```

### 13.3 TLS provisioning

**G1/G2 (ACME via rustls-acme)**:
- Feature-gated behind `domain-acme`
- reactor-jobs schedules certificate renewal 30 days before expiry
- Certificates stored in reactor-storage or local keystore

**G3 (CDN handoff)**:
- Domain verification triggers CDN API to provision certificate
- CDN handles renewal automatically
- Sites server just tracks status

### 13.4 Host resolution

The serve plane resolves hosts in order:
1. Exact match in `_reactor_sites.domains` → site_id
2. Pattern match: `{name}.{org}.reactor.app` → site lookup
3. Preview pattern: `{deployment_id}.preview.{name}.{org}.reactor.app` → site + specific deployment

---

## 14. Preview deployments

### 14.1 Auto-subdomain

Every deployment is addressable at:
```
{deployment_id}.preview.{site_name}.{org}.reactor.app
```

This is automatic — no promotion required. The host resolver extracts the deployment_id from the subdomain and uses that deployment instead of `current_deployment_id`.

### 14.2 Preview policies

Per-site policies can password-protect previews:

```sql
policy preview_auth on site "my-app"
  using (
    not (request.host like '%.preview.%')
    or request.header('x-preview-token') = '{secret}'
  );
```

### 14.3 Preview vs production

| Aspect | Preview | Production |
|---|---|---|
| URL | `{dep_id}.preview.{site}.reactor.app` | `{site}.reactor.app` or custom domain |
| Deployment | Any deployment | `current_deployment_id` only |
| Caching | ISR disabled (always fresh) | ISR enabled |
| Indexing | `X-Robots-Tag: noindex` | None (indexable) |

---

## 15. Policy integration

### 15.1 Builtins

| Builtin | Type | Description |
|---|---|---|
| `site.name` | `text` | The site's name |
| `site.framework` | `text` | `'static' \| 'hono' \| 'nextjs' \| ...` |
| `deployment.version` | `bigint` | Active deployment version |
| `deployment.id` | `text` | Deployment ID |
| `request.method` | `text` | HTTP method |
| `request.path` | `text` | Request path |
| `request.host` | `text` | Host header value |
| `request.header(name)` | `text` | Request header value |

### 15.2 Example policies

```sql
-- Password-protect preview deployments
policy preview_auth on site "my-app"
  using (
    not (request.host like '%.preview.%')
    or request.header('x-preview-token') = 'secret123'
  );

-- Block access to /admin except from specific IPs
policy admin_ip on site "my-app"
  using (
    not (request.path like '/admin%')
    or request.header('cf-connecting-ip') in ('1.2.3.4', '5.6.7.8')
  );

-- Require authentication header for API routes
policy api_auth on site "my-app"
  using (
    not (request.path like '/api%')
    or request.header('authorization') is not null
  );
```

---

## 16. Auth integration

### 16.1 Admin plane

Admin routes use the standard auth middleware:
- `Authorization: Bearer <jwt>` required
- `X-Reactor-Org` optional for org override
- Permissions: `sites:create`, `sites:{name}:deploy`, `sites:{name}:admin`, `sites:{name}:logs`

### 16.2 Serve plane

The serve plane does **not** require authentication by default. Per-site policies can enforce auth requirements. This matches how Vercel/Netlify work — static sites and SSR apps handle their own auth.

### 16.3 Internal service keys

reactor-sites-server holds:
- `REACTOR_SITES_FUNCTIONS_API_KEY`: for deploying/invoking internal functions
- `REACTOR_SITES_STORAGE_API_KEY`: for accessing the `_reactor_sites` system bucket

These are internal secrets, never exposed to end users.

### 16.4 Topology wiring

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

let functions_client = FunctionsClient::new(config.functions_url.clone(), config.functions_api_key.clone());
let storage_client = StorageClient::new(config.storage_url.clone(), config.storage_api_key.clone());
let cache = CacheBackend::new(pool.clone());
let store = PgSitesStore::new(pool);
let policy = PolicyEngine::with_builtins(sites_builtins());

let state = SitesState::new(store, functions_client, storage_client, cache, auth, policy, config);
let app = reactor_sites::router(state);
```

---

## 17. Configuration

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_SITES_DATABASE_URL` | yes | — | Postgres connection string |
| `REACTOR_SITES_BIND` | no | `0.0.0.0:8006` | HTTP bind address |
| `REACTOR_SITES_FUNCTIONS_URL` | yes | — | URL of reactor-functions-server |
| `REACTOR_SITES_FUNCTIONS_API_KEY` | yes | — | Internal API key for reactor-functions |
| `REACTOR_SITES_STORAGE_URL` | yes | — | URL of reactor-storage-server |
| `REACTOR_SITES_STORAGE_API_KEY` | yes | — | Storage API key with `storage:_reactor_sites:*` |
| `REACTOR_SITES_JOBS_URL` | no | — | URL of reactor-jobs-server (for ISR/ACME jobs) |
| `REACTOR_SITES_JOBS_API_KEY` | no | — | Internal API key for reactor-jobs |
| `REACTOR_SITES_WORKDIR` | no | `/var/lib/reactor-sites` | Local workspace for bundle processing |
| `REACTOR_SITES_STATIC_MAX_FILES` | no | `50000` | Max static files per deployment |
| `REACTOR_SITES_STATIC_MAX_BYTES` | no | `536870912` | 512 MiB total static size per deployment |
| `REACTOR_SITES_ISR_DEFAULT_TTL_SECS` | no | `3600` | Default ISR revalidate interval |
| `REACTOR_SITES_REVALIDATION_SECRET` | yes | — | Internal secret for function-driven revalidation |
| `REACTOR_SITES_PREVIEW_SUBDOMAIN` | no | `preview` | Subdomain prefix for preview deployments |
| `REACTOR_SITES_ACME_EMAIL` | no (G2) | — | Email for Let's Encrypt registration |
| `REACTOR_SITES_ACME_DIRECTORY` | no (G2) | LE production | ACME directory URL |
| `REACTOR_SITES_DEPLOYMENT` | no | `monolith` | `monolith` or `microservices` |
| `REACTOR_SITES_AUTH_URL` | yes (microservices) | — | URL of reactor-auth-server |
| `REACTOR_SITES_INTERNAL_SECRET` | yes (microservices) | — | Shared secret for internal endpoints |
| `REACTOR_SITES_AUTH_DATABASE_URL` | yes (monolith) | — | Postgres URL for auth schema |
| `REACTOR_SITES_AUTH_DATA_KEY` | yes (monolith) | — | Auth column-encryption key |
| `REACTOR_SITES_METRICS` | no | `0` | Set to `1` to enable Prometheus `/metrics` |
| `REACTOR_SITES_INVOCATION_SAMPLE_RATE` | no | `0.01` | Sample rate for invocation logging (1%) |
| `REACTOR_LOG` | no | `info` | Tracing filter |

---

## 18. Tracing, metrics, audit

- **Tracing**: `tracing` + JSON subscriber; every request has a `request_id` span; fields include `site`, `deployment_version`, `path`, `route_kind`, `duration_ms`, `status_code`, `cache_status`.

- **Metrics**: Prometheus `/metrics` (gated by `REACTOR_SITES_METRICS=1`):
  - `sites_requests_total{site, route_kind, status}`
  - `sites_request_duration_seconds{site, route_kind, cache_status}`
  - `sites_static_hits_total{site}`
  - `sites_function_dispatches_total{site, function_role}`
  - `sites_isr_hits_total{site, cache_status}` (HIT/MISS/STALE)
  - `sites_isr_revalidations_total{site}`
  - `sites_deployments_total{site, status}` (gauge)
  - `sites_domains_total{site, status}` (gauge)
  - `sites_policy_denied_total{site}`

### 18.1 Audit events

- `site.create`, `site.delete`
- `deployment.create`, `deployment.promote`, `deployment.rollback`, `deployment.fail`, `deployment.destroy`
- `domain.create`, `domain.verify`, `domain.activate`, `domain.delete`
- `policy.create`, `policy.delete`
- `isr.invalidate`

### 18.2 Invocations (sampled)

Sites traffic can be extremely high volume. Invocations are logged at a configurable sample rate (default 1%). The `invocations` table is for debugging and analytics, not billing — use metrics for billing.

---

## 19. Test surface

- **Unit**: manifest validation, route matching, framework detection, cache rules parsing, domain verification token generation.
- **Integration**: `testcontainers` Postgres + mock reactor-functions + mock reactor-storage.
- **Framework conformance**: `tests/framework_conformance.rs` runs each adapter against a sample project:
  - StaticAdapter: HTML/CSS/JS directory → bundle → manifest validates → routes resolve
  - HonoAdapter: Hono app → bundle → function compiles → routes resolve
  - NextjsAdapter: Next.js app → bundle → functions compile → prerender extracted → routes resolve
- **Cross-capability**: `tests/sites_integration.rs` runs:
  - Static site: create → deploy → promote → request `/index.html` → assert 200
  - Hono site: create → deploy → promote → request `/` → function invoked → assert response
  - Next.js site: create → deploy → promote → request prerendered → assert ISR headers → request SSR → function invoked
  - Custom domain: create → add domain → verify → request via domain → assert works
  - Preview: deploy without promote → request via preview subdomain → assert works
  - Policy: add password policy → request without token → 403 → request with token → 200

---

## 20. Cargo workspace additions

Root `Cargo.toml` additions:

```toml
[workspace]
members = [
  # ... existing ...
  "crates/reactor-sites",
  "crates/reactor-sites-server",
]

[workspace.dependencies]
matchit = "0.8"              # path-to-regexp route matching
rustls-acme = "0.10"         # ACME client (feature-gated)
reactor-sites = { path = "crates/reactor-sites" }
```

New Cargo features on `reactor-sites`:

```toml
[features]
default = ["framework-static", "framework-hono", "framework-nextjs"]
framework-static = []
framework-hono = []
framework-nextjs = []
domain-acme = ["dep:rustls-acme"]
```

---

## 21. Build order (v0 slice)

| # | Task | Outcome |
|---|---|---|
| 0 | Land this design doc | Reviewed contract |
| 1 | Workspace skeleton: `reactor-sites`, `reactor-sites-server` | `cargo check --workspace` clean |
| 2 | `SitesConfig` + `SitesState` + `router(state)` + `/sites/v1/health` | Binary boots, health returns 200 |
| 3 | Metadata migrations + `SitesStore` trait + `PgSitesStore` | Schema applies, smoke test green |
| 4 | Auth middleware (admin plane) + site CRUD | Sites can be created, listed, deleted |
| 5 | Bundle manifest schema + validator + chunked upload + static → storage | Static assets upload, deployment row at `pending` |
| 6 | Function dispatch: deploy synthetic internal functions via reactor-functions API | Functions deploy, deployment transitions to `ready` |
| 7 | `SiteHost` trait + serve plane + route matcher + static/function dispatch + promote/rollback + preview subdomains | Static site serves end-to-end; promote/rollback work |
| 8 | `StaticAdapter` (CLI side) + bundle pack/verify | `reactor sites build ./dist` works |
| 9 | `HonoAdapter` (CLI side) | Hono app deploys as static + 1 function |
| 10 | Custom domains + verification + ACME (G2, feature-gated) | `app.example.com` works with auto-cert |
| 11 | `NextjsAdapter` (CLI side) | Next.js app deploys with 1-3 functions + prerender |
| 12 | ISR + on-demand revalidation + reactor-jobs integration | Prerender serves, stale-while-revalidate works |
| 13 | Per-site policies via reactor-policy | Preview password-protect works |
| 14 | Logs SSE merging router + function logs | Unified log stream |
| 15 | Invocations (sampled) + audit + metrics + tracing | Observability complete |
| 16 | Doctor + README + cross-capability harness | v0 exit checklist passes |

### v0 exit checklist

- [ ] Server boots; migrations apply; doctor green (DB, functions, storage)
- [ ] Site CRUD with permissions enforced
- [ ] Static site: deploy → promote → serve → assert 200 with correct cache headers
- [ ] Hono site: deploy → promote → serve → function invoked → assert response
- [ ] Next.js site: deploy → promote → serve prerendered → ISR headers correct → serve SSR → function invoked
- [ ] Deploy and promote are separate operations
- [ ] Preview deployment: deploy without promote → request via preview subdomain → works
- [ ] Custom domain: create → verify (mock DNS) → ACME (G2) → serve via domain
- [ ] ISR: stale request → serve stale + X-Reactor-Cache: STALE → background revalidation triggers
- [ ] On-demand revalidation: POST /revalidate → ISR cache invalidated
- [ ] Policy: preview password-protect → 403 without token → 200 with token
- [ ] Logs SSE: streams router + function logs
- [ ] Audit events: every admin action recorded
- [ ] Metrics: Prometheus endpoint healthy
- [ ] Cross-capability harness: `{static, hono, nextjs} × {InProcess, Remote} × {Fs, S3}` passes

---

## 22. Decision log

| Question | Decision | Rationale |
|---|---|---|
| **Sites architecture** | Composer over reactor-functions + reactor-storage | Sites is not a new compute primitive. Avoids becoming a bespoke framework host. |
| **Bundle format** | Vercel Build Output API-shaped | Well-understood target; framework adapters compile to it; portable. |
| **Router location v0** | Origin-based (reactor-sites-server) | Simpler deployment; manifest kept edge-compatible for v0.2. |
| **Function splitting** | One per site by default; adapter overrides at semantic seams | Operational cost is N×; warm-pool wants 1 function; per-route splitting is Vercel-scale concern. |
| **Sites-owned functions** | Hidden via `_internal` flag in reactor-functions | Users never see synthetic functions; lifecycle managed by Sites. |
| **Promote semantics** | Atomic at site level (single row swap) | In-flight requests finish on old deployment; simple, correct. |
| **Preview deployments** | First-class from PR 7; auto-subdomain | Every deployment is testable; no promotion required to preview. |
| **ISR cache** | reactor-cache (primary) + Postgres backstop | Consistent with Jobs; fast KV with durability. |
| **Custom domains** | ACME via rustls-acme (G2); CDN handoff (G3) | G2 self-sufficient; G3 delegates TLS. |
| **ACME renewal** | reactor-jobs background job | First cross-capability proof point; no in-band renewal. |
| **K8s topology** | Future `KubernetesRuntime` adapter in reactor-functions | Not a Sites concern; Sites talks HTTP. |
| **Invocation logging** | Sampled (default 1%) | Sites traffic can be huge; metrics for billing, samples for debugging. |
| **Non-goals v0** | No edge router, no per-route splitting, no managed Git, no analytics | Focus on the composer model; defer complexity. |

---

## 23. Open questions (deferred to v0.2)

1. **Edge router**: Emit Cloudflare Worker / CloudFront Function from manifest for edge-side routing.
2. **Per-site analytics**: Web Vitals, request logs, error tracking — build `reactor-analytics` or integrate third-party.
3. **Image optimization**: `reactor-images` capability or CDN transform integration.
4. **Password-protect-preview UX**: UI for generating preview tokens; expiring tokens.
5. **Multi-region static**: Replicate `_reactor_sites` bucket across regions for edge latency.
6. **Bundle GC**: How aggressively to prune old deployments. Keep last 10 + promoted?
7. **Framework adapter version pinning**: How to handle Next.js 14 vs 15 vs 16; adapter per version or detection?
8. **SvelteKit / Astro / Nuxt adapters**: Order of rollout; common patterns to extract.
9. **Git-push deploy**: server-side FrameworkAdapter execution; GitHub/GitLab webhook integration.
10. **Deployment aliasing**: `staging.{site}.reactor.app` pointing to a specific deployment without promotion.

---

*End of design doc. Land code against checklist §21 in order, one PR per row, this doc updated as decisions change.*
