# `reactor-auth` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** First crate of the Reactor.cloud BaaS. Owns Identity per `docs/ReactorCloud_spec.md` §5.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — types, traits, endpoints, schema — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Be the **medular** identity layer for every other Reactor capability (Data, Storage, Functions, Jobs, Sites, Gateway).
2. Be **independently deployable** as a standalone HTTP service *and* embeddable as an in-process library in a single binary — without consumer code changes.
3. Expose a **GoTrue-shaped HTTP surface** (so existing client patterns are familiar) but free to improve where it pays off.
4. Encode the **multi-tenancy spine**: User → Org → Membership → Role → Permission. Every other capability inherits this model.
5. Be the only crate allowed to touch the `reactor_auth.*` Postgres schema.

## 2. Non-goals (v0)

- SAML / enterprise SSO (later, possibly via WorkOS proxy)
- WebAuthn (v2.5+)
- Audit-log UI (events emitted, UI deferred)
- A full account console (separate `reactor-web` later)
- Magic-link / passwordless flows (v0.2)
- SCIM provisioning

## 3. Crate layout

```
crates/
├── reactor-core/                  # shared types, errors, IDs, client traits
│   └── src/
│       ├── lib.rs
│       ├── id.rs                  # ReactorId (UUIDv7 newtype)
│       ├── error.rs               # ReactorError base
│       └── auth/                  # AuthClient trait + Claims types (no impls)
│           ├── mod.rs
│           ├── client.rs          # AuthClient trait
│           ├── claims.rs          # Claims, AuthCtx
│           └── error.rs           # AuthError
│
├── reactor-auth/                  # the auth library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against reactor_auth.*
│   │   ├── 001_init.sql
│   │   ├── 002_orgs.sql
│   │   └── ...
│   ├── DESIGN.md                  # (this file, mirrored)
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # AuthConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # AuthState (shared appstate)
│       ├── service.rs             # AuthService (business logic, no HTTP)
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── signup.rs
│       │   ├── token.rs
│       │   ├── user.rs
│       │   ├── recover.rs
│       │   ├── verify.rs
│       │   ├── factors.rs         # MFA (v0.2)
│       │   ├── oauth.rs           # (v0.2)
│       │   ├── orgs.rs
│       │   ├── keys.rs            # JWKS
│       │   └── health.rs
│       ├── store/
│       │   ├── mod.rs             # IdentityStore trait
│       │   └── postgres.rs        # PgIdentityStore impl (sqlx)
│       ├── token/
│       │   ├── mod.rs
│       │   ├── issuer.rs          # JWT signing (RS256)
│       │   ├── verifier.rs        # JWT verification
│       │   ├── keyring.rs         # JWKS + rotation
│       │   └── refresh.rs         # refresh-token rotation
│       ├── password.rs            # argon2id wrappers
│       ├── tenancy/
│       │   ├── mod.rs
│       │   ├── orgs.rs
│       │   ├── members.rs
│       │   └── roles.rs
│       ├── permissions/
│       │   ├── mod.rs
│       │   └── dsl.rs             # parser + evaluator
│       ├── email/
│       │   ├── mod.rs             # EmailSender trait
│       │   ├── smtp.rs            # SmtpEmailSender (lettre-based, works with Resend/Postmark/SES/Mailgun)
│       │   ├── noop.rs            # NoopEmailSender (logs warning, returns Ok when SMTP unconfigured)
│       │   └── templates.rs       # plain-text email templates
│       ├── client/
│       │   ├── mod.rs
│       │   ├── in_process.rs      # InProcessAuthClient
│       │   └── remote.rs          # RemoteAuthClient (HTTP)
│       └── error.rs
│
└── reactor-auth-server/           # standalone bin: `reactor-auth-server`
    ├── Cargo.toml
    └── src/main.rs                # axum bind + tracing + migrate + serve
```

Conventions:
- `reactor-core` has **no runtime deps** beyond `serde`, `thiserror`, `uuid`, `chrono`. It's the smallest stable contract.
- `reactor-auth` depends on `reactor-core` and provides the `AuthClient` *implementations*.
- Future capabilities (`reactor-data`, etc.) depend on `reactor-core` only. They never `use reactor_auth::*`.

---

## 4. Core types (in `reactor-core`)

```rust
// reactor-core/src/id.rs
pub struct ReactorId([u8; 16]);   // wraps Uuid (v7)
impl ReactorId {
    pub fn new() -> Self;          // generates UUIDv7
    pub fn parse(s: &str) -> Result<Self, ParseIdError>;
}
// Display: "01HZ8K..." (canonical 36-char lowercase hyphenated)

pub type UserId = ReactorId;
pub type OrgId  = ReactorId;
pub type SessionId = ReactorId;
pub type RoleId = ReactorId;
```

```rust
// reactor-core/src/auth/claims.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub:         String,             // "user_<id>" or "apikey:<id>"
    pub iss:         String,             // "reactor-auth"
    pub aud:         String,             // "reactor"
    pub exp:         i64,                // unix seconds
    pub iat:         i64,
    pub nbf:         Option<i64>,
    pub email:       Option<String>,
    pub amr:         Vec<AuthMethod>,    // ["pwd"], ["pwd","totp"], ["oauth:google"], ["apikey"]
    pub orgs:        Vec<OrgId>,
    pub default_org: Option<OrgId>,
    pub session_id:  Option<SessionId>,  // absent for apikeys
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod { Pwd, Totp, Oauth(OauthProvider), Apikey, MagicLink }

#[derive(Debug, Clone)]
pub struct AuthCtx {
    pub claims:       Claims,
    pub active_org:   Option<OrgId>,     // resolved from X-Reactor-Org or default_org
    pub permissions:  Vec<String>,       // pre-resolved if cheap, lazy otherwise
}
```

```rust
// reactor-core/src/auth/client.rs
#[async_trait]
pub trait AuthClient: Send + Sync + 'static {
    async fn verify_token(&self, token: &str) -> Result<Claims, AuthError>;

    async fn resolve_ctx(
        &self,
        token: &str,
        requested_org: Option<&OrgId>,
    ) -> Result<AuthCtx, AuthError>;

    async fn get_user(&self, id: &UserId) -> Result<User, AuthError>;

    async fn check_permission(
        &self,
        ctx: &AuthCtx,
        permission: &str,             // e.g. "data:todos:read"
    ) -> Result<bool, AuthError>;

    async fn jwks(&self) -> Result<Jwks, AuthError>;
}
```

Two impls, both in `reactor-auth`:

- **`InProcessAuthClient`** — wraps `Arc<AuthService>`, zero RPC overhead. Used when auth and consumer run in the same binary (G1 Tauri, G2 single-server).
- **`RemoteAuthClient`** — `reqwest`-backed, hits `/auth/v1/*` on a configured URL. Used when auth is deployed as its own service (G3a/G3c microservice mode). Caches JWKS for verification; never makes a network call for `verify_token` on the happy path.

The trait is the **only** way Data/Storage/Functions/Jobs talk to auth.

---

## 5. HTTP surface (v0)

GoTrue-shaped. Diverges only where noted.

### 5.1 Public endpoints (unauthenticated)

```
POST   /auth/v1/signup
       Body: { email, password, metadata?: {} }
       → 201 { user, session: { access_token, refresh_token, expires_at } }

POST   /auth/v1/token?grant_type=password
       Body: { email, password }
       → 200 { access_token, refresh_token, expires_at, user }

POST   /auth/v1/token?grant_type=refresh_token
       Body: { refresh_token }
       → 200 { access_token, refresh_token, expires_at }
       (Refresh is single-use, rotating. Old token revoked atomically.)

POST   /auth/v1/recover
       Body: { email }
       → 204 (always, no enumeration)

POST   /auth/v1/verify
       Body: { type: "signup"|"recovery"|"email_change", token }
       → 200 { user, session? }

GET    /auth/v1/keys                              -- JWKS (JSON Web Key Set)
GET    /auth/v1/.well-known/openid-configuration  -- minimal OIDC discovery
GET    /auth/v1/health                            -- liveness/readiness
```

### 5.2 Authenticated endpoints (Bearer JWT)

**Note:** Path parameters `{ref}` accept either UUID or slug. The server tries UUID parse first, then falls back to slug lookup.

```
POST   /auth/v1/logout                            -- revokes current session
GET    /auth/v1/user                              -- current user
PATCH  /auth/v1/user                              -- update email/password/metadata
DELETE /auth/v1/user                              -- delete self (soft delete)

# Org tenancy (Reactor extension — GoTrue does not have this)
GET    /auth/v1/orgs                              -- orgs the user belongs to
POST   /auth/v1/orgs                              -- create org (becomes owner)
GET    /auth/v1/orgs/{ref}
PATCH  /auth/v1/orgs/{ref}
DELETE /auth/v1/orgs/{ref}

GET    /auth/v1/orgs/{ref}/members
POST   /auth/v1/orgs/{ref}/invitations            -- creates signed link; sends email if SMTP configured
GET    /auth/v1/orgs/{ref}/invitations            -- list pending invitations
DELETE /auth/v1/orgs/{ref}/invitations/{id}       -- revoke invitation
POST   /auth/v1/orgs/{ref}/invitations/{tok}/accept
PATCH  /auth/v1/orgs/{ref}/members/{user_id}/role
DELETE /auth/v1/orgs/{ref}/members/{user_id}

GET    /auth/v1/orgs/{ref}/roles
POST   /auth/v1/orgs/{ref}/roles
PATCH  /auth/v1/orgs/{ref}/roles/{role_id}
DELETE /auth/v1/orgs/{ref}/roles/{role_id}

# Permissions (resolved for active org)
GET    /auth/v1/permissions                       -- effective perms for (user, active_org)

# Internal verify endpoint (used by RemoteAuthClient when JWKS is insufficient,
# e.g. revocation check) — gated to internal callers only via shared secret.
POST   /auth/v1/_internal/resolve_ctx
       Body: { token, requested_org? }
       → 200 { claims, active_org, permissions }
```

### 5.3 Headers

| Header | Meaning |
|---|---|
| `Authorization: Bearer <jwt>` | Standard. |
| `X-Reactor-Org: <org_id>` | Active org for this request. Server verifies membership. Falls back to `default_org` from JWT. |
| `X-Reactor-Idempotency-Key: <uuid>` | Required on mutations once we add the global idempotency layer; tolerated as no-op until then. |

### 5.4 Error envelope

Single shape across all routes:

```json
{
  "error": {
    "code": "invalid_credentials",
    "message": "Email or password is incorrect.",
    "status": 401,
    "request_id": "req_01HZ..."
  }
}
```

Error codes are stable strings (snake_case), enumerated in `reactor-core::auth::error`. Never use HTTP status alone to discriminate — clients switch on `code`.

---

## 6. Database schema (`reactor_auth` schema in shared Postgres)

```sql
create schema if not exists reactor_auth;

-- 6.1 Users -------------------------------------------------------------
create table reactor_auth.users (
  id              uuid primary key,                 -- UUIDv7
  email           citext unique not null,
  email_verified  boolean not null default false,
  password_hash   text,                             -- nullable: oauth-only users
  metadata        jsonb not null default '{}'::jsonb,
  default_org_id  uuid,                             -- fk added after orgs table exists
  disabled_at     timestamptz,
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now()
);

-- 6.2 Identities (oauth provider linkages) ------------------------------
create table reactor_auth.identities (
  id              uuid primary key,
  user_id         uuid not null references reactor_auth.users(id) on delete cascade,
  provider        text not null,                    -- 'google' | 'github' | 'email'
  provider_uid    text not null,                    -- subject from IdP
  metadata        jsonb not null default '{}'::jsonb,
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now(),
  unique (provider, provider_uid)
);

-- 6.3 Orgs (tenancy boundary) -------------------------------------------
create table reactor_auth.orgs (
  id              uuid primary key,
  slug            citext unique not null,           -- url-safe, user-chosen
  name            text not null,
  metadata        jsonb not null default '{}'::jsonb,
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now()
);

alter table reactor_auth.users
  add constraint users_default_org_fk
  foreign key (default_org_id) references reactor_auth.orgs(id) on delete set null;

-- 6.4 Roles (per-org, project-defined) ----------------------------------
create table reactor_auth.roles (
  id              uuid primary key,
  org_id          uuid not null references reactor_auth.orgs(id) on delete cascade,
  name            text not null,                    -- 'owner' | 'admin' | 'member' | custom
  description     text,
  is_system       boolean not null default false,   -- 'owner' role is system
  created_at      timestamptz not null default now(),
  unique (org_id, name)
);

-- 6.5 Permissions assigned to roles -------------------------------------
create table reactor_auth.role_permissions (
  role_id         uuid not null references reactor_auth.roles(id) on delete cascade,
  permission      text not null,                    -- 'data:todos:read', '*'
  primary key (role_id, permission)
);

-- 6.6 Memberships -------------------------------------------------------
create table reactor_auth.memberships (
  user_id         uuid not null references reactor_auth.users(id) on delete cascade,
  org_id          uuid not null references reactor_auth.orgs(id) on delete cascade,
  role_id         uuid not null references reactor_auth.roles(id) on delete restrict,
  joined_at       timestamptz not null default now(),
  primary key (user_id, org_id)
);

-- 6.7 Sessions (one row per active session) -----------------------------
create table reactor_auth.sessions (
  id              uuid primary key,
  user_id         uuid not null references reactor_auth.users(id) on delete cascade,
  amr             text[] not null default '{}',     -- auth methods used
  ip              inet,
  user_agent      text,
  created_at      timestamptz not null default now(),
  last_seen_at    timestamptz not null default now(),
  revoked_at      timestamptz
);

-- 6.8 Refresh tokens (rotating, single-use) -----------------------------
create table reactor_auth.refresh_tokens (
  id              uuid primary key,
  session_id      uuid not null references reactor_auth.sessions(id) on delete cascade,
  token_hash      bytea not null unique,            -- sha256 of refresh token
  issued_at       timestamptz not null default now(),
  expires_at      timestamptz not null,
  used_at         timestamptz,                      -- non-null => burned
  replaced_by     uuid                              -- next token after rotation
);
create index on reactor_auth.refresh_tokens (session_id) where used_at is null;

-- 6.9 OAuth state (PKCE) ------------------------------------------------
create table reactor_auth.oauth_states (
  state           text primary key,                 -- random url-safe
  provider        text not null,
  code_verifier   text not null,
  redirect_to     text,
  created_at      timestamptz not null default now(),
  expires_at      timestamptz not null              -- 10 min
);

-- 6.10 MFA factors ------------------------------------------------------
create table reactor_auth.mfa_factors (
  id              uuid primary key,
  user_id         uuid not null references reactor_auth.users(id) on delete cascade,
  factor_type     text not null,                    -- 'totp'
  secret          bytea not null,                   -- encrypted at app layer
  verified_at     timestamptz,
  created_at      timestamptz not null default now()
);

-- 6.11 Email tokens (verify, recover, invite) ---------------------------
create table reactor_auth.email_tokens (
  token_hash      bytea primary key,                -- sha256(token)
  purpose         text not null,                    -- 'signup'|'recovery'|'email_change'|'invite'
  user_id         uuid references reactor_auth.users(id) on delete cascade,
  org_id          uuid references reactor_auth.orgs(id) on delete cascade,  -- for invites
  payload         jsonb not null default '{}'::jsonb,
  created_at      timestamptz not null default now(),
  expires_at      timestamptz not null,
  used_at         timestamptz
);

-- 6.12 JWT signing keys (rotation) --------------------------------------
create table reactor_auth.signing_keys (
  kid             text primary key,                 -- e.g. 'k_01HZ...'
  algorithm       text not null,                    -- 'RS256'
  private_key_pem text not null,                    -- encrypted at app layer
  public_key_pem  text not null,
  created_at      timestamptz not null default now(),
  activated_at    timestamptz not null,
  rotated_at      timestamptz,                      -- when superseded
  retired_at      timestamptz                       -- when no longer in JWKS
);

-- 6.13 Audit events (write-only) ----------------------------------------
create table reactor_auth.audit_events (
  id              uuid primary key,
  ts              timestamptz not null default now(),
  actor_user_id   uuid,
  actor_apikey_id uuid,
  org_id          uuid,
  event_type      text not null,                    -- 'user.signup', 'session.created'...
  resource        text,
  ip              inet,
  user_agent      text,
  details         jsonb not null default '{}'::jsonb
);
create index on reactor_auth.audit_events (org_id, ts desc);
create index on reactor_auth.audit_events (actor_user_id, ts desc);
```

Notes:
- `citext` for case-insensitive email + slug uniqueness.
- `password_hash`, `secret`, `private_key_pem` are application-layer encrypted with a key from `REACTOR_AUTH_DATA_KEY` (AES-256-GCM). Postgres-at-rest encryption is not enough; we want defence-in-depth.
- All UUIDs are v7 generated app-side. No `gen_random_uuid()` (we want time-sortable IDs without depending on `pgcrypto`).
- Audit events stream to a write-only table now; we'll fan them out to S3/Loki later.

---

## 7. Token lifecycle

### 7.1 Access tokens (JWT, RS256)

- TTL: **3600 s** (1 h)
- Claims: §4 above
- Signing key rotation: §7.3
- Verification: pure cryptographic (no DB roundtrip on happy path)

### 7.2 Refresh tokens

- Format: 256 bits of random, base64url, prefix `rrf_` (e.g. `rrf_AbCd...`). Not a JWT. Opaque to clients.
- Stored hashed (sha256) in `refresh_tokens.token_hash`.
- TTL: **30 days**, sliding (each rotation resets expiry).
- **Single-use**: refresh exchange atomically marks the old token `used_at` and inserts a replacement. Re-use of a used token revokes the entire session (token-theft detection).

### 7.3 Signing-key rotation

- Two active states per key: `active` (currently signing) and `previous` (verify-only).
- Rotation procedure (`reactor-auth-server rotate-keys`):
  1. Generate new RSA-2048 keypair, store in `signing_keys` with `activated_at = now()`.
  2. The new key becomes `active`; the prior `active` becomes `previous` (`rotated_at = now()`).
  3. After 7 days, `previous` keys get `retired_at` set and drop from JWKS output.
- JWKS endpoint includes all non-retired keys.
- Activation latency: caches in `RemoteAuthClient` refresh every 5 min; rotation is safe to do at any time.

### 7.4 Revocation

- `POST /auth/v1/logout` → `sessions.revoked_at = now()` and invalidate refresh tokens.
- Active access tokens stay valid until `exp` (max 1 h). Acceptable trade-off for stateless verify.
- Hard revoke (security incident): `reactor-auth-server revoke-session <id>` plus key rotation to invalidate all tokens.

---

## 8. Permissions

### 8.1 String form

Permissions are dotted, colon-separated lower-snake strings:

```
data:todos:read           -- read rows from the 'todos' table
data:todos:write          -- insert/update/delete
data:*:read               -- read any table
storage:avatars:put
functions:checkout:invoke
*                         -- god mode (system 'owner' role only)
```

Resolution:
- Exact match always wins.
- `*` at any segment is a wildcard for that segment.
- The trailing `*` permission grants everything within an org.

### 8.2 Built-in roles per new org

| Role | Permissions |
|---|---|
| `owner` | `*` (system, single user per org initially) |
| `admin` | `data:*:*`, `storage:*:*`, `functions:*:*`, `jobs:*:*`, `auth:members:*`, `auth:roles:*` |
| `member` | `data:*:read`, `storage:*:read`, `functions:*:invoke` |

Customers can define additional roles per org. `owner` cannot be deleted; transferring requires a special endpoint.

### 8.3 DSL (deferred)

The future SQL-policy DSL referenced in spec §4.2 will call `auth.has_permission('todos:read')`. That function:
- On Postgres: a SQL function in the `reactor_auth` schema reading from `memberships`/`role_permissions`.
- On the in-process side: `AuthClient::check_permission(ctx, perm)`.

Same semantics, two implementations. Defined in this doc, *implemented when `reactor-data` arrives.*

---

## 9. Client trait (the orchestration commitment)

```rust
// reactor-core/src/auth/client.rs
#[async_trait]
pub trait AuthClient: Send + Sync + 'static {
    async fn verify_token(&self, token: &str) -> Result<Claims, AuthError>;

    /// Full request context: verify + resolve active org + load permissions.
    /// `requested_org` comes from the X-Reactor-Org header.
    async fn resolve_ctx(
        &self,
        token: &str,
        requested_org: Option<&OrgId>,
    ) -> Result<AuthCtx, AuthError>;

    async fn get_user(&self, id: &UserId) -> Result<User, AuthError>;

    async fn check_permission(
        &self,
        ctx: &AuthCtx,
        permission: &str,
    ) -> Result<bool, AuthError>;

    async fn jwks(&self) -> Result<Jwks, AuthError>;
}
```

### 9.1 `InProcessAuthClient`

```rust
pub struct InProcessAuthClient {
    service: Arc<AuthService>,
}
```

Direct function calls. Zero serialization. Used in G1/G2.

### 9.2 `RemoteAuthClient`

```rust
pub struct RemoteAuthClient {
    base_url:       Url,
    http:           reqwest::Client,
    jwks_cache:     Arc<RwLock<JwksCache>>,   // refreshed every 5 min
    internal_secret: Option<String>,          // for /_internal/* endpoints
}
```

- `verify_token`: pure-crypto via cached JWKS. **No network call on happy path.**
- `resolve_ctx`: hits `POST /auth/v1/_internal/resolve_ctx` (one call per request when remote).
- `jwks`: 5-min cached; background refresh.

### 9.3 Wiring at startup

```rust
let auth: Arc<dyn AuthClient> = match config.deployment {
    Deployment::Monolith => Arc::new(InProcessAuthClient::new(auth_service.clone())),
    Deployment::Microservices => Arc::new(
        RemoteAuthClient::builder()
            .base_url(config.auth_url.clone())
            .internal_secret(config.internal_secret.clone())
            .build()?
    ),
};

let data_state  = DataState::new(auth.clone(), db.clone());
let store_state = StorageState::new(auth.clone(), blob.clone());
```

Consumer code never branches on topology.

---

## 10. Configuration

`reactor-auth-server` reads from env (12-factor) and optionally `reactor.toml [identity]`. Env wins.

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_AUTH_DATABASE_URL` | yes | — | Postgres connection string |
| `REACTOR_AUTH_BIND` | no | `0.0.0.0:8001` | HTTP bind address |
| `REACTOR_AUTH_DATA_KEY` | yes | — | base64 32-byte AES-256-GCM key for column encryption |
| `REACTOR_AUTH_JWT_ISSUER` | no | `reactor-auth` | `iss` claim |
| `REACTOR_AUTH_JWT_AUDIENCE` | no | `reactor` | `aud` claim |
| `REACTOR_AUTH_ACCESS_TTL_SECS` | no | `3600` | |
| `REACTOR_AUTH_REFRESH_TTL_SECS` | no | `2592000` | 30 days |
| `REACTOR_AUTH_INTERNAL_SECRET` | yes (microservice) | — | for `/_internal/*` |
| `REACTOR_AUTH_SMTP_HOST` | no | — | SMTP server hostname (e.g. `smtp.resend.com`, `smtp.postmarkapp.com`) |
| `REACTOR_AUTH_SMTP_PORT` | no | `587` | SMTP port |
| `REACTOR_AUTH_SMTP_USER` | no | — | SMTP username (often `apikey` for Resend/Postmark) |
| `REACTOR_AUTH_SMTP_PASSWORD` | no | — | SMTP password / API key |
| `REACTOR_AUTH_SMTP_FROM` | no | — | From address for outgoing emails |
| `REACTOR_AUTH_SMTP_TLS` | no | `starttls` | TLS mode: `starttls`, `tls`, or `none` |
| `REACTOR_AUTH_PUBLIC_URL` | yes | — | base URL used in email links and signed invite URLs |
| `REACTOR_AUTH_GOOGLE_CLIENT_ID` / `_SECRET` | no | — | OAuth (v0.2) |
| `REACTOR_AUTH_GITHUB_CLIENT_ID` / `_SECRET` | no | — | OAuth (v0.2) |
| `REACTOR_LOG` | no | `info` | tracing filter |

Boot fails fast on missing required vars. `doctor` subcommand prints diagnostics.

---

## 11. Tracing, metrics, audit

- **Tracing**: `tracing` + `tracing-subscriber` JSON output; every request has a `request_id` span field; `X-Request-Id` header echoed.
- **Metrics**: Prometheus endpoint at `/metrics` (basic — request counters, latency histograms, db pool stats). Off by default; gated by `REACTOR_AUTH_METRICS=1`.
- **Audit**: every state-changing operation writes a row to `reactor_auth.audit_events` *in the same transaction* as the state change. No fire-and-forget; if the audit fails, the operation fails.

---

## 12. Crypto rules (lock these now)

| Concern | Choice |
|---|---|
| Password hash | `argon2id`, m=64 MiB, t=3, p=1 (calibrated per deploy on first boot) |
| JWT signing | RS256, 2048-bit RSA keys |
| Refresh token | 32 bytes from `rand::rngs::OsRng`, base64url, prefix `rrf_` |
| Email token | 32 bytes from `OsRng`, base64url, prefix per-purpose (`rsv_`, `rrc_`, `rin_`) |
| TOTP | RFC 6238, SHA-1, 6 digits, 30 s, ±1 window |
| OAuth | Authorization Code + PKCE (S256) mandatory; state random ≥ 16 bytes |
| Column encryption | AES-256-GCM with `REACTOR_AUTH_DATA_KEY`, nonce per record |
| Constant-time compare | `subtle::ConstantTimeEq` everywhere a token/secret is compared |

---

## 13. Test surface

- **Unit**: pure functions (password hashing roundtrip, JWT issue/verify, permission resolver).
- **Integration**: spin up Postgres via `testcontainers-rs`; full HTTP harness against `reactor-auth-server` using `axum::Router::into_make_service` + `tower::ServiceExt::oneshot` (no actual sockets needed for most tests).
- **Conformance**: a `reactor-auth-conformance` test suite that runs against *both* `InProcessAuthClient` and `RemoteAuthClient` over the same scenarios — guarantees the two impls stay behaviourally identical.
- **Property**: `proptest` for the permission resolver and refresh-rotation state machine.

---

## 14. Cargo workspace

Root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members  = [
  "crates/reactor-core",
  "crates/reactor-auth",
  "crates/reactor-auth-server",
]

[workspace.package]
edition    = "2021"
rust-version = "1.78"
license    = "Apache-2.0 OR MIT"
repository = "https://github.com/.../reactor"

[workspace.dependencies]
tokio          = { version = "1", features = ["full"] }
axum           = { version = "0.7", features = ["macros", "tracing"] }
tower          = "0.5"
tower-http     = { version = "0.5", features = ["trace", "cors", "request-id"] }
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
sqlx           = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json", "macros"] }
uuid           = { version = "1", features = ["v7", "serde"] }
chrono         = { version = "0.4", features = ["serde"] }
thiserror      = "1"
anyhow         = "1"
tracing        = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
jsonwebtoken   = "9"
argon2         = "0.5"
rand           = "0.8"
rand_core      = { version = "0.6", features = ["std"] }
subtle         = "2"
aes-gcm        = "0.10"
base64         = "0.22"
sha2           = "0.10"
url            = { version = "2", features = ["serde"] }
async-trait    = "0.1"
reqwest        = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
totp-rs        = "5"
oauth2         = "4"
validator      = { version = "0.18", features = ["derive"] }
```

Pinning exact versions later; `workspace.dependencies` keeps the whole tree consistent.

---

## 15. Build order (v0 slice)

Each task is a self-contained PR. Doc lands first; checkpoints below.

| # | Task | Outcome |
|---|---|---|
| 0 | Workspace skeleton (root `Cargo.toml`, empty crates) | `cargo check` passes |
| 1 | `reactor-core`: `ReactorId`, `Claims`, `AuthCtx`, `AuthClient` trait, `AuthError` | published contract |
| 2 | `reactor-auth` skeleton: `AuthConfig`, `AuthState`, `Router::new(state)`, health route | binary boots, `/auth/v1/health` returns 200 |
| 3 | Migrations 001–003 (users, orgs, memberships, sessions, refresh_tokens, signing_keys) | `sqlx migrate run` clean |
| 4 | `signing_keys` + JWKS endpoint + key rotation command | `/auth/v1/keys` returns rotating JWKS |
| 5 | Password hash + signup + password-grant token + refresh rotation | end-to-end signup → login → refresh works |
| 6 | `/auth/v1/user` GET/PATCH/DELETE + logout | session lifecycle complete |
| 7 | Orgs + memberships + roles (system roles seeded on org create) | tenancy spine done |
| 8 | `resolve_ctx` + permission check + `X-Reactor-Org` resolution + `/auth/v1/permissions` | consumers can ask "who is this, in which org, with what powers" |
| 9 | `InProcessAuthClient` + `RemoteAuthClient` + conformance suite | both topologies pass identical tests |
| 10 | Audit events on every mutation | `audit_events` populated |
| 11 | `reactor-auth-server` binary polish: tracing, graceful shutdown, `doctor` cmd | shippable |

**v0 exit criteria**: a consumer crate can `use reactor_core::auth::AuthClient` and pick monolith or remote at startup, end-to-end identity flows work, both clients pass the conformance suite.

v0.2 adds: email flows (verify/recover/invite), OAuth (Google + GitHub), TOTP MFA, API keys.

---

## 16. Decision log

Decisions locked during v0 planning (May 2026):

| Question | Decision | Rationale |
|---|---|---|
| **API-key shape** | Opaque tokens, validated via DB lookup | Easier to revoke; cached in `RemoteAuthClient`. JWTs deferred to v0.2. |
| **Invitations** | Both signed shareable link (always) and SMTP email (when configured) | Works without SMTP for local dev; production gets email delivery via any SMTP relay (Resend, Postmark, SES, Mailgun). |
| **Org URL refs** | Accept both UUID and slug in path params (`{ref}`) | UUID for programmatic access, slug for human-friendly URLs. Server tries UUID parse first, falls back to slug lookup. |
| **Session model** | Pure bearer (no device binding) | Simpler for v0. Device fingerprinting and IP-binding is a v2 hardening pass. |
| **MFA enforcement** | Per-org policy with per-user opt-in | Org admins can require MFA for specific roles (e.g. "MFA required for `admin` role"). Implementation deferred to v0.2. |
| **Email transport** | Generic SMTP via `lettre` crate | Works with any SMTP relay. No vendor lock-in. Configured via `REACTOR_AUTH_SMTP_*` env vars. |

---

## 17. Open questions (deferred)

1. **API-key JWT variant.** Once opaque keys ship, should we also support JWT-based API keys for use cases that need stateless verify? Evaluate based on customer feedback.
2. **Invite link expiry.** Currently 7 days. Should this be configurable per-org?
3. **Rate limiting.** Not in v0. Need to decide: in-process token bucket, Redis-backed, or delegate to reverse proxy?

---

*End of design doc. Land code against checklist §15 in order, one PR per row, this doc updated as decisions change.*
