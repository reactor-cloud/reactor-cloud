# reactor-cloud Control Plane Design

> Design document for the reactor.cloud managed service control plane

## Status: Draft
## Author: AI Assistant
## Date: 2026-05-17

---

## 1. Overview

### Problem Statement

Currently, deploying Reactor to Fly.io requires:
- Manual `fly.toml` and Dockerfile configuration
- Manual secret management via `flyctl secrets`
- Confusion between admin tokens and user JWTs
- Manual domain verification in the database
- No CLI-driven infrastructure provisioning

### Goals

1. **One-command provisioning**: `reactor cloud create my-project` spins up complete infrastructure
2. **Unified authentication**: Single auth flow for CLI → control plane → provider
3. **Provider abstraction**: Start with Fly.io, design for multi-cloud
4. **Self-hosted compatibility**: Keep `reactor-server` clean; control plane is optional
5. **Multi-tenant**: Support multiple orgs/projects on reactor.cloud

### Non-Goals (v0)

- Kubernetes support (future)
- Auto-scaling policies (manual scaling first)
- Multi-region active-active (single region first)
- Custom domains with automatic SSL (Fly.io handles this)

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           reactor.cloud                                  │
│                                                                          │
│  ┌─────────────┐     ┌──────────────────────┐     ┌─────────────────┐  │
│  │ reactor-cli │────▶│ reactor-cloud-api    │────▶│ Provider Layer  │  │
│  │             │     │ (control plane)      │     │                 │  │
│  │ - login     │     │                      │     │ ┌─────────────┐ │  │
│  │ - create    │     │ - Auth (OAuth/keys)  │     │ │ Fly.io API  │ │  │
│  │ - deploy    │     │ - Project CRUD       │     │ └─────────────┘ │  │
│  │ - logs      │     │ - Deployment queue   │     │ ┌─────────────┐ │  │
│  │ - destroy   │     │ - Secret vault       │     │ │ AWS (future)│ │  │
│  └─────────────┘     │ - Domain management  │     │ └─────────────┘ │  │
│                      │ - Billing hooks      │     └─────────────────┘  │
│                      └──────────────────────┘                          │
│                               │                                         │
│                               ▼                                         │
│                      ┌──────────────────────┐                          │
│                      │ PostgreSQL           │                          │
│                      │ (control plane DB)   │                          │
│                      │                      │                          │
│                      │ - orgs, users        │                          │
│                      │ - projects           │                          │
│                      │ - deployments        │                          │
│                      │ - provider_configs   │                          │
│                      │ - secrets (encrypted)│                          │
│                      └──────────────────────┘                          │
└─────────────────────────────────────────────────────────────────────────┘
                                │
                                │ Provisions
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Customer Infrastructure                           │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Fly.io Machine (per project)                                       │ │
│  │                                                                     │ │
│  │  ┌────────────────┐  ┌──────────────┐  ┌──────────────────────┐   │ │
│  │  │ reactor-server │  │ PostgreSQL   │  │ Fly Volume           │   │ │
│  │  │ (all caps)     │  │ (embedded)   │  │ (/data)              │   │ │
│  │  └────────────────┘  └──────────────┘  └──────────────────────┘   │ │
│  │                                                                     │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│  DNS: {project}.reactor.cloud → Fly.io anycast                          │
│  Custom domains: Added via control plane, verified via TXT records      │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Crate Structure

```
crates/
  reactor-cloud/              # Control plane library
    src/
      lib.rs
      api/                    # REST API handlers
        mod.rs
        projects.rs           # Project CRUD
        deployments.rs        # Deployment management
        domains.rs            # Domain/DNS management
        secrets.rs            # Secret management
        logs.rs               # Log streaming proxy
      auth/
        mod.rs
        oauth.rs              # GitHub/Google OAuth
        api_keys.rs           # API key management
        middleware.rs         # Auth middleware
      providers/
        mod.rs                # Provider trait
        fly.rs                # Fly.io implementation
        types.rs              # Common types
      queue/
        mod.rs                # Background job queue
        provision.rs          # Provisioning jobs
        deploy.rs             # Deployment jobs
        teardown.rs           # Cleanup jobs
      store/
        mod.rs
        postgres.rs           # PostgreSQL store
      config.rs
      error.rs
      router.rs

  reactor-cloud-server/       # Control plane binary
    src/
      main.rs

  reactor-cli/                # Existing CLI (extended)
    src/
      commands/
        cloud.rs              # NEW: cloud subcommands
        # ... existing commands
```

---

## 4. API Design

### Authentication

```
POST /cloud/v1/auth/login
  - Initiates OAuth flow (GitHub/Google)
  - Returns: { redirect_url }

GET /cloud/v1/auth/callback
  - OAuth callback
  - Sets session, returns API key

POST /cloud/v1/auth/api-keys
  - Create API key for CI/CD
  - Returns: { key_id, key_secret } (secret shown once)

DELETE /cloud/v1/auth/api-keys/:id
  - Revoke API key
```

### Projects

```
GET /cloud/v1/projects
  - List projects for authenticated org
  - Returns: [{ id, name, region, status, url, created_at }]

POST /cloud/v1/projects
  - Create new project (provisions infrastructure)
  - Body: { name, region, plan?, config? }
  - Returns: { id, name, status: "provisioning" }
  - Async: Provisions Fly machine, volume, secrets

GET /cloud/v1/projects/:id
  - Get project details
  - Returns: { id, name, region, status, url, endpoints, resources }

DELETE /cloud/v1/projects/:id
  - Destroy project (tears down infrastructure)
  - Async: Removes Fly machine, volume, DNS

PATCH /cloud/v1/projects/:id
  - Update project settings
  - Body: { config?, scale? }
```

### Deployments

```
POST /cloud/v1/projects/:id/deployments
  - Deploy bundle to project
  - Body: multipart/form-data with bundle.tar.gz
  - Returns: { deployment_id, status: "queued" }

GET /cloud/v1/projects/:id/deployments
  - List deployments
  - Returns: [{ id, version, status, created_at, deployed_at }]

GET /cloud/v1/projects/:id/deployments/:deployment_id
  - Get deployment status
  - Returns: { id, status, phases[], logs_url }

POST /cloud/v1/projects/:id/deployments/:deployment_id/promote
  - Promote deployment to production (if using preview)

POST /cloud/v1/projects/:id/deployments/:deployment_id/rollback
  - Rollback to previous deployment
```

### Domains

```
GET /cloud/v1/projects/:id/domains
  - List domains
  - Returns: [{ host, status, verification_record }]

POST /cloud/v1/projects/:id/domains
  - Add custom domain
  - Body: { host }
  - Returns: { host, status: "pending", verification_record }

DELETE /cloud/v1/projects/:id/domains/:host
  - Remove domain

POST /cloud/v1/projects/:id/domains/:host/verify
  - Verify domain ownership (checks DNS TXT record)
```

### Secrets

```
GET /cloud/v1/projects/:id/secrets
  - List secret names (not values)
  - Returns: [{ name, updated_at }]

PUT /cloud/v1/projects/:id/secrets
  - Set secrets (encrypted at rest)
  - Body: { secrets: { KEY: "value" } }
  - Triggers machine restart to inject

DELETE /cloud/v1/projects/:id/secrets/:name
  - Remove secret
```

### Logs

```
GET /cloud/v1/projects/:id/logs
  - Stream logs (SSE)
  - Query: ?follow=true&since=2026-05-17T00:00:00Z
  - Proxies to Fly.io log streaming
```

---

## 5. Provider Abstraction

```rust
// crates/reactor-cloud/src/providers/mod.rs

#[async_trait]
pub trait CloudProvider: Send + Sync {
    /// Provision infrastructure for a new project
    async fn provision(&self, req: ProvisionRequest) -> Result<ProvisionResult, ProviderError>;
    
    /// Deploy a bundle to existing infrastructure
    async fn deploy(&self, req: DeployRequest) -> Result<DeployResult, ProviderError>;
    
    /// Tear down infrastructure
    async fn teardown(&self, project_id: &str) -> Result<(), ProviderError>;
    
    /// Get project status
    async fn status(&self, project_id: &str) -> Result<ProjectStatus, ProviderError>;
    
    /// Stream logs
    async fn logs(&self, project_id: &str, opts: LogOptions) -> Result<LogStream, ProviderError>;
    
    /// Scale resources
    async fn scale(&self, project_id: &str, scale: ScaleConfig) -> Result<(), ProviderError>;
    
    /// Set secrets/env vars
    async fn set_secrets(&self, project_id: &str, secrets: HashMap<String, String>) -> Result<(), ProviderError>;
    
    /// Add custom domain
    async fn add_domain(&self, project_id: &str, domain: &str) -> Result<DomainConfig, ProviderError>;
    
    /// Remove custom domain
    async fn remove_domain(&self, project_id: &str, domain: &str) -> Result<(), ProviderError>;
}

pub struct ProvisionRequest {
    pub project_name: String,
    pub region: String,
    pub machine_size: MachineSize,
    pub volume_size_gb: u32,
    pub initial_secrets: HashMap<String, String>,
}

pub struct ProvisionResult {
    pub provider_project_id: String,  // e.g., Fly app name
    pub public_url: String,           // e.g., https://myapp.fly.dev
    pub internal_url: Option<String>, // e.g., myapp.internal:8000
    pub resources: ProvisionedResources,
}

pub struct ProvisionedResources {
    pub machine_id: String,
    pub volume_id: String,
    pub ip_addresses: Vec<String>,
}
```

### Fly.io Implementation

```rust
// crates/reactor-cloud/src/providers/fly.rs

pub struct FlyProvider {
    client: reqwest::Client,
    api_token: String,
    org_slug: String,
    docker_registry: String,
}

impl FlyProvider {
    pub fn new(api_token: String, org_slug: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_token,
            org_slug,
            docker_registry: "registry.fly.io".to_string(),
        }
    }
}

#[async_trait]
impl CloudProvider for FlyProvider {
    async fn provision(&self, req: ProvisionRequest) -> Result<ProvisionResult, ProviderError> {
        // 1. Create Fly app
        let app = self.create_app(&req.project_name, &req.region).await?;
        
        // 2. Create volume
        let volume = self.create_volume(&app.name, &req.region, req.volume_size_gb).await?;
        
        // 3. Allocate IPs
        let ips = self.allocate_ips(&app.name).await?;
        
        // 4. Set secrets
        self.set_secrets_internal(&app.name, &req.initial_secrets).await?;
        
        // 5. Deploy reactor-server image
        let machine = self.create_machine(&app.name, &MachineConfig {
            image: format!("{}/reactor-cloud/reactor-server:latest", self.docker_registry),
            size: req.machine_size,
            mounts: vec![Mount {
                volume: volume.id.clone(),
                path: "/data".to_string(),
            }],
            env: self.build_env(&req),
            services: vec![Service {
                internal_port: 8000,
                protocol: "tcp",
                ports: vec![
                    Port { port: 443, handlers: vec!["tls", "http"] },
                    Port { port: 80, handlers: vec!["http"] },
                ],
            }],
        }).await?;
        
        Ok(ProvisionResult {
            provider_project_id: app.name.clone(),
            public_url: format!("https://{}.fly.dev", app.name),
            internal_url: Some(format!("{}.internal:8000", app.name)),
            resources: ProvisionedResources {
                machine_id: machine.id,
                volume_id: volume.id,
                ip_addresses: ips,
            },
        })
    }
    
    // ... other implementations
}
```

---

## 6. CLI Integration

### New Commands

```bash
# Authentication
reactor cloud login                    # OAuth browser flow
reactor cloud logout
reactor cloud whoami

# Project management
reactor cloud create <name>            # Create project
  --region <region>                    # fly region (iad, lhr, etc.)
  --plan <plan>                        # starter, pro, enterprise
  
reactor cloud list                     # List projects
reactor cloud status [project]         # Project status
reactor cloud destroy <project>        # Tear down

# Deployment (extends existing deploy)
reactor deploy                         # Deploys to current context
  --context cloud                      # Deploy to reactor.cloud
  --project <project>                  # Override project

# Logs
reactor cloud logs [project]           # Stream logs
  --follow                             # Tail mode
  --since <duration>                   # e.g., 1h, 30m

# Domains
reactor cloud domains list
reactor cloud domains add <domain>
reactor cloud domains verify <domain>
reactor cloud domains remove <domain>

# Secrets
reactor cloud secrets list
reactor cloud secrets set KEY=value KEY2=value2
reactor cloud secrets unset KEY

# Scaling (future)
reactor cloud scale --cpu 2 --memory 2gb
```

### Context Management

```bash
# ~/.config/reactor/contexts.toml
[contexts.cloud]
endpoint = "https://api.reactor.cloud"
type = "cloud"
# Token stored in OS keychain

[contexts.local]
endpoint = "http://localhost:8000"
type = "self-hosted"
token = "..." # or in keychain

[contexts.prod]
endpoint = "https://reactor-cloud.fly.dev"
type = "self-hosted"
# Token in keychain
```

---

## 7. Database Schema

```sql
-- Control plane database (separate from reactor-server DBs)

CREATE SCHEMA reactor_cloud;

-- Organizations (multi-tenant)
CREATE TABLE reactor_cloud.orgs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT UNIQUE NOT NULL,
    plan TEXT NOT NULL DEFAULT 'starter', -- starter, pro, enterprise
    stripe_customer_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Users
CREATE TABLE reactor_cloud.users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT UNIQUE NOT NULL,
    name TEXT,
    avatar_url TEXT,
    oauth_provider TEXT NOT NULL, -- github, google
    oauth_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (oauth_provider, oauth_id)
);

-- Org memberships
CREATE TABLE reactor_cloud.org_members (
    org_id UUID REFERENCES reactor_cloud.orgs(id) ON DELETE CASCADE,
    user_id UUID REFERENCES reactor_cloud.users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member', -- owner, admin, member
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);

-- API keys
CREATE TABLE reactor_cloud.api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES reactor_cloud.orgs(id) ON DELETE CASCADE,
    user_id UUID REFERENCES reactor_cloud.users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL, -- bcrypt hash of key
    key_prefix TEXT NOT NULL, -- first 8 chars for identification
    scopes TEXT[] NOT NULL DEFAULT '{}', -- empty = all permissions
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Projects
CREATE TABLE reactor_cloud.projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES reactor_cloud.orgs(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    region TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'provisioning', -- provisioning, running, stopped, failed, destroying
    
    -- Provider details
    provider TEXT NOT NULL DEFAULT 'fly',
    provider_project_id TEXT, -- e.g., fly app name
    provider_config JSONB NOT NULL DEFAULT '{}',
    
    -- URLs
    public_url TEXT,
    internal_url TEXT,
    
    -- Resources
    machine_size TEXT NOT NULL DEFAULT 'shared-cpu-1x',
    volume_size_gb INT NOT NULL DEFAULT 10,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    UNIQUE (org_id, slug)
);

-- Deployments
CREATE TABLE reactor_cloud.deployments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES reactor_cloud.projects(id) ON DELETE CASCADE,
    version INT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued', -- queued, deploying, deployed, failed, superseded
    
    -- Bundle info
    bundle_hash TEXT NOT NULL,
    bundle_size_bytes BIGINT NOT NULL,
    manifest JSONB NOT NULL,
    
    -- Phases tracking (same as reactor-server deploy)
    phases JSONB NOT NULL DEFAULT '[]',
    
    -- Timing
    queued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    
    -- Who deployed
    deployed_by_user_id UUID REFERENCES reactor_cloud.users(id),
    deployed_by_api_key_id UUID REFERENCES reactor_cloud.api_keys(id),
    
    -- Error info
    error_message TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_deployments_project ON reactor_cloud.deployments(project_id, version DESC);

-- Domains
CREATE TABLE reactor_cloud.domains (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES reactor_cloud.projects(id) ON DELETE CASCADE,
    host TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, verifying, active, failed
    verification_token TEXT NOT NULL,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (host)
);

-- Secrets (encrypted at rest)
CREATE TABLE reactor_cloud.secrets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES reactor_cloud.projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    encrypted_value BYTEA NOT NULL, -- AES-256-GCM encrypted
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, name)
);

-- Audit log
CREATE TABLE reactor_cloud.audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES reactor_cloud.orgs(id) ON DELETE SET NULL,
    user_id UUID REFERENCES reactor_cloud.users(id) ON DELETE SET NULL,
    project_id UUID REFERENCES reactor_cloud.projects(id) ON DELETE SET NULL,
    action TEXT NOT NULL, -- project.create, deployment.start, secret.set, etc.
    details JSONB NOT NULL DEFAULT '{}',
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_org ON reactor_cloud.audit_events(org_id, created_at DESC);
CREATE INDEX idx_audit_project ON reactor_cloud.audit_events(project_id, created_at DESC);

-- Background jobs (for async provisioning/deployment)
CREATE TABLE reactor_cloud.jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    queue TEXT NOT NULL DEFAULT 'default',
    job_type TEXT NOT NULL, -- provision, deploy, teardown
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, running, completed, failed, dead
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 3,
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_jobs_queue ON reactor_cloud.jobs(queue, status, scheduled_at) 
    WHERE status IN ('pending', 'running');
```

---

## 8. Security Considerations

### Authentication Flow

1. **CLI Login**: 
   - `reactor cloud login` opens browser
   - OAuth with GitHub/Google
   - Callback returns short-lived token
   - CLI exchanges for API key
   - API key stored in OS keychain

2. **API Key Security**:
   - Keys are `rc_xxxxxxxxxxxxxxxxxxxxxxxxxxxx` format
   - Only key prefix stored in DB (for listing)
   - Full key hashed with bcrypt
   - Optional expiration
   - Optional scope restrictions

3. **Secret Encryption**:
   - Secrets encrypted with AES-256-GCM
   - Key derived from master key + project ID
   - Master key in environment (or Vault/KMS)

### Provider Credentials

- Fly.io API token stored per-org (encrypted)
- Or: reactor.cloud uses a single Fly org with app namespacing
- Future: Support BYOK (bring your own key) for enterprise

---

## 9. Implementation Plan

### Phase 1: Core Control Plane (v0.1)
- [ ] `reactor-cloud` crate structure
- [ ] PostgreSQL schema and store
- [ ] Fly.io provider implementation
- [ ] Basic REST API (projects, deployments)
- [ ] CLI `cloud` subcommands
- [ ] OAuth login flow

### Phase 2: Production Ready (v0.2)
- [ ] Background job queue for async ops
- [ ] Domain management with verification
- [ ] Secret management with encryption
- [ ] Log streaming proxy
- [ ] Audit logging
- [ ] Error handling and retries

### Phase 3: Polish (v0.3)
- [ ] API key management
- [ ] Usage metering and billing hooks
- [ ] Dashboard UI (web)
- [ ] Multi-region support
- [ ] Preview deployments

### Phase 4: Scale (v1.0)
- [ ] Auto-scaling policies
- [ ] Multi-provider support (AWS, GCP)
- [ ] Enterprise features (SSO, RBAC)
- [ ] SLA and support tiers

---

## 10. Open Questions

1. **Single Fly org vs per-customer orgs?**
   - Single org: Simpler billing, namespaced apps (`rc-{org}-{project}`)
   - Per-customer: Better isolation, customer brings own account

2. **Bundle storage?**
   - Store bundles in S3/R2 for replay?
   - Or: Just stream through to deployment?

3. **Database per project vs shared?**
   - Current: Each project has embedded PostgreSQL
   - Future: Option for managed PostgreSQL (Neon, Supabase)?

4. **Pricing model?**
   - Per-project flat fee?
   - Usage-based (compute + storage)?
   - Hybrid?

---

## Appendix: Example Flow

```bash
# 1. Login
$ reactor cloud login
Opening browser for authentication...
✓ Logged in as carlos@reactor.cloud (org: reactor)

# 2. Create project
$ reactor cloud create my-saas --region iad
Creating project 'my-saas' in iad...
  ✓ Provisioning Fly.io app
  ✓ Creating 10GB volume
  ✓ Allocating IPv4 and IPv6
  ✓ Deploying reactor-server
  ✓ Configuring DNS

Project created!
  URL: https://my-saas.reactor.cloud
  Admin: https://my-saas.reactor.cloud/_admin

# 3. Deploy
$ cd my-app
$ reactor deploy --context cloud --project my-saas
Packaging bundle... (1.2MB)
Uploading to reactor.cloud...
Deploying...
  ✓ Bundle validated
  ✓ Migrations applied (2 new)
  ✓ Functions deployed (3 functions)
  ✓ Sites deployed (1 site, 47 assets)

Deployment complete! (v3)
  https://my-saas.reactor.cloud

# 4. Add custom domain
$ reactor cloud domains add app.mysaas.com
Add this TXT record to verify ownership:
  _reactor-verify.app.mysaas.com  →  rc_verify_abc123xyz

$ reactor cloud domains verify app.mysaas.com
✓ Domain verified and active

# 5. Set secrets
$ reactor cloud secrets set STRIPE_KEY=sk_live_xxx RESEND_KEY=re_xxx
✓ Secrets updated (restarting to apply)
```
