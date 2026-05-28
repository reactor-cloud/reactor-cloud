# `reactor-storage` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Third crate of the Reactor.cloud BaaS. Owns the Storage capability per `docs/ReactorCloud_spec.md` §2/§3/§9/§10-E.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, schema, policy integration, signed-URL format, multipart protocol — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Expose an **S3-shaped HTTP surface** for blob storage, modelled on Supabase Storage so existing client patterns are immediately usable.
2. Be **backend-portable**: support both local filesystem (G1/G2) and S3-compatible object storage (G3a/G3b/G3c) behind a single `BlobStore` trait. Both adapters ship at v0.
3. **Reuse the shared `reactor-policy` engine** for per-object authorization. Policies are the same DSL as reactor-data but evaluated against object metadata instead of SQL rows.
4. Be the **second real consumer of `reactor_core::auth::AuthClient`** — exercises both `InProcessAuthClient` and `RemoteAuthClient` topologies alongside reactor-data.
5. Be the only crate allowed to touch the `_reactor_storage.*` Postgres metadata schema.

## 2. Non-goals (v0)

- **TUS resumable upload protocol** — deferred to v0.2.
- **Lifecycle policies** (TTL, automatic cleanup) — deferred to v0.2.
- **Per-object explicit ACLs** — the policy DSL covers the same use cases.
- **Image transformations** (resize, format conversion) — likely a Function, not core storage.
- **Webhook events** on bucket/object lifecycle — deferred to v0.2.
- **Soft-delete + versioning** — deferred to v0.2.
- **Multipart cleanup sweeper** for abandoned uploads — deferred to v0.2.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, IDs, AuthClient trait
│
├── reactor-policy/                # NEW — extracted from reactor-data
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                 # public re-exports
│       ├── grammar.rs             # policy DSL parser
│       ├── ast.rs                 # PolicyExpr (Bool tree)
│       ├── compile.rs             # PolicyExpr × Context → Decision
│       ├── eval.rs                # Runtime evaluation for non-SQL contexts
│       ├── builtins.rs            # auth.* + extensible domain builtins
│       └── store.rs               # storage-specific builtins (object.*, bucket.*)
│
├── reactor-data/                  # (existing) — depends on reactor-policy
│
├── reactor-storage/               # the storage library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_storage.*
│   │   ├── 001_metadata.sql
│   │   ├── 002_policies.sql
│   │   ├── 003_multipart.sql
│   │   └── 004_audit.sql
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # StorageConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # StorageState, StorageCtx
│       ├── error.rs               # StorageError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs
│       │   ├── buckets.rs         # CRUD for buckets
│       │   ├── objects.rs         # PUT/GET/HEAD/DELETE objects
│       │   ├── multipart.rs       # S3-style multipart upload
│       │   └── sign.rs            # signed URL generation
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth.rs            # bearer + X-Reactor-Org → StorageCtx
│       │
│       ├── service/
│       │   ├── mod.rs
│       │   ├── buckets.rs         # bucket business logic
│       │   ├── objects.rs         # object business logic
│       │   ├── multipart.rs       # multipart orchestration
│       │   └── policy.rs          # policy enforcement for objects
│       │
│       ├── store/
│       │   ├── mod.rs             # MetadataStore + BlobStore traits
│       │   ├── metadata_postgres.rs  # PgMetadataStore
│       │   └── blob/
│       │       ├── mod.rs
│       │       ├── fs.rs          # FsBlobStore (local filesystem)
│       │       └── s3.rs          # S3BlobStore (aws-sdk-s3)
│       │
│       ├── signing.rs             # HMAC URL signing for FS backend
│       └── audit.rs               # mutation audit-event writer
│
└── reactor-storage-server/        # standalone bin
    ├── Cargo.toml
    └── src/
        ├── main.rs                # axum bind + tracing + migrate + serve
        └── cli/
            ├── mod.rs
            └── doctor.rs          # connectivity diagnostics
```

Conventions:
- `reactor-storage` depends on `reactor-core` (for `ReactorId`, `AuthClient`, `AuthCtx`) and `reactor-policy` (for the shared policy engine).
- `reactor-storage` **never** depends on `reactor-auth`. Auth is consumed through the `AuthClient` trait, full stop.
- Both `FsBlobStore` and `S3BlobStore` ship at v0, selected via Cargo features (`fs`, `s3`) and runtime config.

---

## 4. Core types

### 4.1 ID & types reuse

All IDs are `ReactorId` (UUIDv7) from `reactor-core`. Storage-specific types:

| Type | Rust | Notes |
|---|---|---|
| `BucketId` | `ReactorId` | Primary key for buckets |
| `ObjectId` | `ReactorId` | Primary key for objects |
| `UploadId` | `String` | S3-compatible multipart upload ID |

### 4.2 `StorageCtx` (request-local)

Constructed by middleware once per request from `AuthCtx`:

```rust
// reactor-storage/src/state.rs
#[derive(Debug, Clone)]
pub struct StorageCtx {
    pub auth:       Option<AuthCtx>,   // None for anonymous/signed-URL access
    pub request_id: String,
    pub org_id:     Option<OrgId>,     // resolved active org
}

impl StorageCtx {
    pub fn user_id(&self) -> Option<&UserId> { /* ... */ }
    pub fn active_org(&self) -> Option<&OrgId> { self.org_id.as_ref() }
    pub fn has_permission(&self, perm: &str) -> bool {
        self.auth.as_ref().map_or(false, |a| a.has_permission(perm))
    }
    pub fn is_anonymous(&self) -> bool { self.auth.is_none() }
}
```

Anonymous contexts are allowed for:
- Public bucket reads (bucket `public = true`)
- Signed URL access (verified separately)

### 4.3 `BlobStore` trait

```rust
// reactor-storage/src/store/blob/mod.rs
#[async_trait]
pub trait BlobStore: Send + Sync + 'static {
    /// Store an object
    async fn put(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        body: ByteStream,
        content_type: Option<&str>,
    ) -> Result<PutOutcome, StorageError>;

    /// Retrieve an object
    async fn get(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        range: Option<ByteRange>,
    ) -> Result<GetOutcome, StorageError>;

    /// Get object metadata without body
    async fn head(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
    ) -> Result<HeadOutcome, StorageError>;

    /// Delete an object
    async fn delete(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
    ) -> Result<(), StorageError>;

    /// Generate a presigned URL for GET
    async fn presign_get(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        ttl: Duration,
    ) -> Result<SignedUrl, StorageError>;

    // Multipart operations
    async fn create_multipart(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        content_type: Option<&str>,
    ) -> Result<UploadId, StorageError>;

    async fn upload_part(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: u32,
        body: ByteStream,
    ) -> Result<PartEtag, StorageError>;

    async fn complete_multipart(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<PartRef>,
    ) -> Result<PutOutcome, StorageError>;

    async fn abort_multipart(
        &self,
        org_id: &OrgId,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<(), StorageError>;
}

#[derive(Debug)]
pub struct PutOutcome {
    pub etag: String,
    pub size: u64,
}

#[derive(Debug)]
pub struct GetOutcome {
    pub body: ByteStream,
    pub content_type: Option<String>,
    pub content_length: u64,
    pub etag: String,
    pub range: Option<ContentRange>,
}

#[derive(Debug)]
pub struct HeadOutcome {
    pub content_type: Option<String>,
    pub content_length: u64,
    pub etag: String,
}
```

### 4.4 `MetadataStore` trait

```rust
// reactor-storage/src/store/mod.rs
#[async_trait]
pub trait MetadataStore: Send + Sync + 'static {
    type Tx<'a>: MetadataTx where Self: 'a;

    async fn begin(&self) -> Result<Self::Tx<'_>, StorageError>;

    // Bucket operations
    async fn create_bucket(&self, bucket: &NewBucket) -> Result<Bucket, StorageError>;
    async fn get_bucket(&self, org_id: &OrgId, ref_: &BucketRef) -> Result<Option<Bucket>, StorageError>;
    async fn list_buckets(&self, org_id: &OrgId) -> Result<Vec<Bucket>, StorageError>;
    async fn update_bucket(&self, id: &BucketId, update: &BucketUpdate) -> Result<Bucket, StorageError>;
    async fn delete_bucket(&self, id: &BucketId, cascade: bool) -> Result<(), StorageError>;

    // Object metadata operations
    async fn upsert_object(&self, obj: &NewObject) -> Result<Object, StorageError>;
    async fn get_object(&self, bucket_id: &BucketId, key: &str) -> Result<Option<Object>, StorageError>;
    async fn list_objects(&self, bucket_id: &BucketId, prefix: Option<&str>, limit: u32, offset: u32) -> Result<Vec<Object>, StorageError>;
    async fn delete_object(&self, bucket_id: &BucketId, key: &str) -> Result<(), StorageError>;

    // Policy operations
    async fn get_policies(&self, bucket_id: &BucketId, scope: PolicyScope) -> Result<Vec<StoragePolicy>, StorageError>;
    async fn upsert_policy(&self, policy: &NewStoragePolicy) -> Result<StoragePolicy, StorageError>;

    // Multipart tracking
    async fn create_multipart_upload(&self, upload: &NewMultipartUpload) -> Result<MultipartUpload, StorageError>;
    async fn get_multipart_upload(&self, upload_id: &str) -> Result<Option<MultipartUpload>, StorageError>;
    async fn complete_multipart_upload(&self, upload_id: &str) -> Result<(), StorageError>;
    async fn abort_multipart_upload(&self, upload_id: &str) -> Result<(), StorageError>;

    // Audit
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<(), StorageError>;
}
```

---

## 5. HTTP surface (v0)

### 5.1 Health

```
GET    /storage/v1/health
       → 200 { "status": "ok", "version": "0.1.0" }
```

### 5.2 Buckets

```
POST   /storage/v1/buckets
       Body: { "name": "avatars", "public": false }
       → 201 { bucket }
       Requires: storage:bucket:create

GET    /storage/v1/buckets
       → 200 [ bucket, ... ]
       Lists buckets for active org

GET    /storage/v1/buckets/{ref}
       → 200 { bucket }
       {ref} accepts bucket name (slug) OR UUID

PATCH  /storage/v1/buckets/{ref}
       Body: { "public": true }
       → 200 { bucket }
       Requires: storage:{bucket}:admin

DELETE /storage/v1/buckets/{ref}
       Query: ?cascade=true (optional, deletes all objects)
       → 204
       Requires: storage:{bucket}:admin
       Fails with 409 if objects exist and cascade=false
```

Bucket name constraints: `^[a-z0-9][a-z0-9-]{1,62}$` (lowercase, hyphens allowed, 3–63 chars).

### 5.3 Objects (simple upload/download)

```
PUT    /storage/v1/buckets/{bucket}/objects/{*key}
       Body: raw bytes
       Headers: Content-Type (optional)
       → 200 { "key": "...", "etag": "...", "size": 12345 }
       Requires: storage:{bucket}:write + policy evaluation

GET    /storage/v1/buckets/{bucket}/objects/{*key}
       Headers: Range (optional)
       → 200 (body streamed)
       → 206 Partial Content (if Range)
       Response headers: ETag, Content-Length, Content-Type, Accept-Ranges: bytes
       Requires: storage:{bucket}:read OR bucket.public=true OR valid signed URL

HEAD   /storage/v1/buckets/{bucket}/objects/{*key}
       → 200 (no body)
       Response headers: ETag, Content-Length, Content-Type
       Same auth as GET

DELETE /storage/v1/buckets/{bucket}/objects/{*key}
       → 204
       Requires: storage:{bucket}:delete + policy evaluation
```

`{*key}` is a wildcard segment — keys may contain `/` for directory-like structures. Keys are sanitised (no `..`, no null bytes, max 1024 chars).

### 5.4 Objects (listing)

```
GET    /storage/v1/buckets/{bucket}/objects
       Query: ?prefix=uploads/&limit=100&offset=0
       → 200 { "objects": [...], "has_more": true }
       Requires: storage:{bucket}:read
```

### 5.5 Multipart upload (S3-compatible)

```
POST   /storage/v1/buckets/{bucket}/objects/{*key}?uploads
       → 200 { "upload_id": "..." }
       Initiates multipart upload
       Requires: storage:{bucket}:write

PUT    /storage/v1/buckets/{bucket}/objects/{*key}?partNumber=N&uploadId=U
       Body: raw bytes (part data)
       → 200 { "etag": "..." }
       Uploads a part (N is 1-indexed)

POST   /storage/v1/buckets/{bucket}/objects/{*key}?uploadId=U
       Body: { "parts": [{ "part_number": 1, "etag": "..." }, ...] }
       → 200 { "key": "...", "etag": "...", "size": 12345 }
       Completes multipart upload

DELETE /storage/v1/buckets/{bucket}/objects/{*key}?uploadId=U
       → 204
       Aborts multipart upload, cleans up parts
```

Part size constraints:
- Minimum part size: 5 MiB (except last part)
- Maximum parts: 10,000
- Maximum object size: 5 TiB

### 5.6 Signed URLs

```
POST   /storage/v1/buckets/{bucket}/objects/{*key}/sign
       Body: { "ttl_secs": 3600, "action": "read" }
       → 200 { "url": "...", "expires_at": "..." }
       Requires: storage:{bucket}:read (for action=read) or :write (for action=write)
```

**FS backend**: URL is `https://{server}/storage/v1/buckets/{b}/objects/{k}?sig={hmac}&exp={ts}&kid={key_id}`. Verification middleware checks HMAC signature.

**S3 backend**: URL is a native S3 presigned URL. No reactor-storage-server roundtrip on use.

### 5.7 Headers

| Header | Meaning |
|---|---|
| `Authorization: Bearer <jwt>` | Required unless public bucket read or valid signed URL |
| `X-Reactor-Org: <ref>` | Active org override. Accepts UUID or slug. |
| `Content-Type` | MIME type for uploads |
| `Content-Length` | Required for PUT |
| `Range` | Byte range for GET (e.g., `bytes=0-1023`) |
| `If-None-Match` | ETag-based conditional GET |
| `X-Request-Id` | Optional; echoed in response |

### 5.8 Error envelope

Same shape as reactor-auth and reactor-data:

```json
{
  "error": {
    "code": "policy_denied",
    "message": "Object access denied by policy 'reports_tenant'.",
    "status": 403,
    "request_id": "req_01HZ...",
    "details": {
      "bucket": "reports",
      "key": "2024/q1.pdf",
      "policy": "reports_tenant",
      "scope": "read"
    }
  }
}
```

Error codes (snake_case): `bucket_not_found`, `object_not_found`, `bucket_not_empty`, `invalid_bucket_name`, `object_too_large`, `policy_denied`, `signature_invalid`, `signature_expired`, `multipart_not_found`, `part_too_small`.

---

## 6. Database schema (`_reactor_storage`)

```sql
create schema if not exists _reactor_storage;

-- 6.1 Buckets
create table _reactor_storage.buckets (
  id              uuid primary key,               -- ReactorId
  org_id          uuid not null,                  -- FK to reactor_auth.orgs conceptually
  name            citext not null,                -- e.g. 'avatars', 'documents'
  public          boolean not null default false, -- allow anonymous reads
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now(),
  unique (org_id, name)
);
create index on _reactor_storage.buckets (org_id);

-- 6.2 Objects (metadata only; blobs live in backend)
create table _reactor_storage.objects (
  id              uuid primary key,
  bucket_id       uuid not null references _reactor_storage.buckets(id) on delete cascade,
  key             text not null,                  -- path/to/file.pdf
  size            bigint not null,
  content_type    text,
  etag            text not null,                  -- usually sha256 or backend-computed
  metadata        jsonb not null default '{}'::jsonb,  -- user-defined key-values
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now(),
  unique (bucket_id, key)
);
create index on _reactor_storage.objects (bucket_id);
create index on _reactor_storage.objects (bucket_id, key text_pattern_ops);  -- prefix queries

-- 6.3 Policies (per-bucket, per-scope)
create table _reactor_storage.policies (
  id              uuid primary key,
  bucket_id       uuid not null references _reactor_storage.buckets(id) on delete cascade,
  name            text not null,                  -- e.g. 'tenant_isolation'
  scope           text not null,                  -- 'read' | 'write' | 'delete'
  using_expr_json jsonb,                          -- PolicyExpr for reads/deletes
  check_expr_json jsonb,                          -- PolicyExpr for writes
  raw_text        text not null,                  -- original DSL source
  sha256          bytea not null,
  created_at      timestamptz not null default now(),
  unique (bucket_id, name, scope)
);
create index on _reactor_storage.policies (bucket_id, scope);

-- 6.4 Multipart uploads (tracking in-progress uploads)
create table _reactor_storage.multipart_uploads (
  id              uuid primary key,
  bucket_id       uuid not null references _reactor_storage.buckets(id) on delete cascade,
  key             text not null,
  upload_id       text unique not null,           -- exposed to client
  initiated_by    uuid,                           -- user_id
  content_type    text,
  parts           jsonb not null default '[]'::jsonb,  -- [{ part_number, etag, size }]
  created_at      timestamptz not null default now(),
  completed_at    timestamptz,
  aborted_at      timestamptz
);
create index on _reactor_storage.multipart_uploads (bucket_id, key);
create index on _reactor_storage.multipart_uploads (created_at) where completed_at is null and aborted_at is null;

-- 6.5 Audit events
create table _reactor_storage.audit_events (
  id              uuid primary key,
  ts              timestamptz not null default now(),
  actor_user_id   uuid,
  actor_apikey_id uuid,
  org_id          uuid,
  bucket_id       uuid,
  object_key      text,
  event_type      text not null,                  -- 'bucket.create', 'object.put', 'multipart.complete', etc.
  details         jsonb not null default '{}'::jsonb,
  request_id      text not null
);
create index on _reactor_storage.audit_events (org_id, ts desc);
create index on _reactor_storage.audit_events (bucket_id, ts desc);
create index on _reactor_storage.audit_events (actor_user_id, ts desc);
```

### 6.6 Role grants

`_reactor_storage` is **not** readable by the user application role. reactor-storage-server connects with a dedicated role that has:
- `USAGE` on `_reactor_storage` schema
- Full DML on all tables in `_reactor_storage`
- No access to user data schemas (that's reactor-data's domain)

---

## 7. Policy engine integration

### 7.1 Shared policy engine (`reactor-policy`)

The policy DSL is extracted from reactor-data into `reactor-policy`, shared by both crates. The core grammar:

```ebnf
policy_stmt    := "policy" ident "on" target "for" scope_list
                  ( "using" "(" expr ")" )?
                  ( "check" "(" expr ")" )?
                  ";"
target         := table_ref                       -- for reactor-data
               |  "bucket" string_literal         -- for reactor-storage
scope_list     := scope ( "," scope )*
scope          := "read" | "write" | "delete"
```

### 7.2 Storage-specific builtins

In addition to `auth.*` builtins, storage policies can reference:

| Builtin | Type | Description |
|---|---|---|
| `object.key` | `text` | Full object key |
| `object.size` | `bigint` | Object size in bytes |
| `object.content_type` | `text` | MIME type |
| `object.metadata` | `jsonb` | User-defined metadata |
| `bucket.name` | `text` | Bucket name |
| `bucket.public` | `bool` | Whether bucket is public |

### 7.3 Policy evaluation flow

For object operations:

1. Load policies for `(bucket_id, scope)` from `_reactor_storage.policies`.
2. Build `ObjectFacts` from the request:
   - For reads/deletes: facts come from existing object metadata
   - For writes: facts come from the incoming request (proposed object)
3. Evaluate each policy expression against `ObjectFacts` + `StorageCtx`.
4. Decision:
   - `AlwaysDeny` → 403 immediately
   - `AlwaysAllow` → proceed
   - `Conditional` → evaluate in Rust against facts, deny if false

### 7.4 Example policies

```sql
-- All objects in 'avatars' bucket must have org-matching metadata
policy tenant_isolation on bucket "avatars"
  for read, write, delete
  using (object.metadata->>'org_id' = auth.org_id()::text);

-- Only users with specific permission can write large files
policy large_file_gate on bucket "uploads"
  for write
  check (object.size < 10485760 or auth.has_permission('storage:uploads:large'));

-- Restrict by file extension
policy pdf_only on bucket "documents"
  for write
  check (object.key like '%.pdf');
```

---

## 8. Backend adapters

### 8.1 `FsBlobStore` (local filesystem)

- **On-disk layout**: `{FS_ROOT}/{org_id}/{bucket_name}/{key}`
- **Atomicity**: writes go to a temp file, then `rename()` for atomicity
- **ETag**: sha256 of content (computed during upload)
- **Content-Type**: stored in `_reactor_storage.objects` (not on disk)
- **Range requests**: supported via `AsyncSeekExt`
- **Key sanitisation**: reject `..`, null bytes, control characters
- **Multipart**: parts stored in `{FS_ROOT}/.multipart/{upload_id}/{part_number}`; complete concatenates and moves to final location

### 8.2 `S3BlobStore` (S3-compatible)

- **Backend support**: AWS S3, R2, Tigris, MinIO
- **Layout**: single physical bucket with key prefix `{org_id}/{bucket_name}/{key}`
  - Avoids S3's 100-bucket-per-account soft limit
  - Configurable via `_S3_LAYOUT=single_bucket|multi_bucket`
- **ETag**: passthrough from S3 response
- **Content-Type**: passed to S3 on upload
- **Range requests**: passthrough to S3
- **Presigned URLs**: native S3 presigner (no roundtrip through reactor-storage)
- **Multipart**: passthrough to S3's native multipart API

### 8.3 Feature gates

```toml
[features]
default = ["fs"]
fs = []
s3 = ["dep:aws-sdk-s3", "dep:aws-config"]
```

`reactor-storage-server` enables both features by default. Compile with `--no-default-features --features s3` for S3-only deployments.

---

## 9. Signed URLs

### 9.1 FS backend (HMAC)

Format: `https://{host}/storage/v1/buckets/{bucket}/objects/{key}?sig={signature}&exp={expiry}&kid={key_id}`

Components:
- `exp`: Unix timestamp (seconds)
- `kid`: `current` or `previous` (for key rotation)
- `sig`: HMAC-SHA256 over `{bucket}|{key}|{action}|{exp}` using `REACTOR_STORAGE_SIGNED_URL_HMAC_KEY`

Verification middleware:
1. Parse query params
2. Check `exp` against current time (reject if expired)
3. Recompute signature, compare with constant-time equality
4. If valid, allow request without bearer auth

### 9.2 S3 backend (native presign)

Uses `aws-sdk-s3`'s presigning capabilities. URL goes directly to S3; reactor-storage-server is not in the request path.

### 9.3 Key rotation

Support two keys:
- `REACTOR_STORAGE_SIGNED_URL_HMAC_KEY` (current, `kid=current`)
- `REACTOR_STORAGE_SIGNED_URL_HMAC_KEY_PREVIOUS` (previous, `kid=previous`)

Signing always uses current; verification accepts either. Rotate by:
1. Move current to previous
2. Generate new current
3. After TTL window passes, remove previous

---

## 10. Auth integration

### 10.1 Middleware

```rust
async fn auth_middleware<B>(
    State(state): State<StorageState>,
    headers: HeaderMap,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StorageError> {
    let token = headers.get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    let requested_org: Option<OrgRef> = headers.get("x-reactor-org")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.parse().unwrap());

    let ctx = if let Some(token) = token {
        let auth_ctx = state.auth.resolve_ctx(token, requested_org.as_ref()).await?;
        StorageCtx::authenticated(auth_ctx, request_id)
    } else {
        // Anonymous context — only allowed for public buckets or signed URLs
        StorageCtx::anonymous(request_id)
    };

    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}
```

### 10.2 Permission scheme

| Permission | Scope |
|---|---|
| `storage:bucket:create` | Create new buckets in the org |
| `storage:{bucket}:read` | Read objects from a specific bucket |
| `storage:{bucket}:write` | Write objects to a specific bucket |
| `storage:{bucket}:delete` | Delete objects from a specific bucket |
| `storage:{bucket}:admin` | Update/delete bucket itself |
| `storage:*:read` | Read from any bucket |
| `storage:*:*` | Full storage access |

### 10.3 Topology wiring

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

let state = StorageState::new(meta_store, blob_store, auth, policy_engine, config);
let app = reactor_storage::router(state);
```

---

## 11. Configuration

`reactor-storage-server` reads from env (12-factor).

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_STORAGE_DATABASE_URL` | yes | — | Postgres connection string |
| `REACTOR_STORAGE_BIND` | no | `0.0.0.0:8003` | HTTP bind address |
| `REACTOR_STORAGE_BACKEND` | no | `fs` | `fs` or `s3` |
| `REACTOR_STORAGE_FS_ROOT` | yes (fs) | — | Root directory for file storage |
| `REACTOR_STORAGE_S3_ENDPOINT` | no | — | S3 endpoint URL (for R2/MinIO) |
| `REACTOR_STORAGE_S3_REGION` | no | `us-east-1` | AWS region |
| `REACTOR_STORAGE_S3_BUCKET` | yes (s3) | — | Physical S3 bucket name |
| `REACTOR_STORAGE_S3_ACCESS_KEY_ID` | yes (s3) | — | AWS access key |
| `REACTOR_STORAGE_S3_SECRET_ACCESS_KEY` | yes (s3) | — | AWS secret key |
| `REACTOR_STORAGE_S3_LAYOUT` | no | `single_bucket` | `single_bucket` or `multi_bucket` |
| `REACTOR_STORAGE_SIGNED_URL_TTL_SECS` | no | `3600` | Default TTL for signed URLs |
| `REACTOR_STORAGE_SIGNED_URL_HMAC_KEY` | yes (fs) | — | Base64 32-byte key for HMAC signing |
| `REACTOR_STORAGE_SIGNED_URL_HMAC_KEY_PREVIOUS` | no | — | Previous key for rotation |
| `REACTOR_STORAGE_MAX_OBJECT_SIZE` | no | `5368709120` | 5 GiB default |
| `REACTOR_STORAGE_DEPLOYMENT` | no | `monolith` | `monolith` or `microservices` |
| `REACTOR_STORAGE_AUTH_URL` | yes (microservices) | — | URL of reactor-auth-server |
| `REACTOR_STORAGE_INTERNAL_SECRET` | yes (microservices) | — | Shared secret for internal endpoints |
| `REACTOR_STORAGE_AUTH_DATABASE_URL` | yes (monolith) | — | Postgres URL for auth schema |
| `REACTOR_STORAGE_AUTH_DATA_KEY` | yes (monolith) | — | Column encryption key for auth |
| `REACTOR_STORAGE_METRICS` | no | `0` | Set to `1` to enable Prometheus `/metrics` |
| `REACTOR_LOG` | no | `info` | Tracing filter |

---

## 12. Tracing, metrics, audit

- **Tracing**: `tracing` + JSON subscriber; every request has a `request_id` span; fields include `bucket`, `key`, `verb`, `backend`, `bytes_in`, `bytes_out`, `policy_decision`.
- **Metrics**: Prometheus `/metrics` (gated by `REACTOR_STORAGE_METRICS=1`):
  - `storage_requests_total{bucket, verb, status}`
  - `storage_request_duration_seconds{bucket, verb}`
  - `storage_bytes_in_total{bucket}`
  - `storage_bytes_out_total{bucket}`
  - `storage_objects_total{bucket}` (gauge)
  - `storage_policy_denied_total{bucket, scope}`
- **Audit**: every mutation writes to `_reactor_storage.audit_events` in the same transaction. Event types:
  - `bucket.create`, `bucket.update`, `bucket.delete`
  - `object.put`, `object.delete`
  - `multipart.create`, `multipart.complete`, `multipart.abort`
  - `signed_url.issue`
  - `policy.bypass` (when `*` permission used)

---

## 13. Test surface

- **Unit**: signing round-trip, key sanitisation, policy expression evaluation, byte-range parsing.
- **Integration**: `testcontainers` Postgres + `tempdir` for FS backend; `testcontainers` MinIO for S3 backend.
- **Conformance**: `tests/blob_conformance.rs` runs identical scenarios against both `FsBlobStore` and `S3BlobStore`.
- **Cross-capability**: `tests/auth_integration.rs` runs the full matrix:
  - `{FS, S3} × {InProcessAuthClient, RemoteAuthClient}`
  - Scenarios: signup → org → bucket → upload → download → policy → multipart → signed URL → audit

---

## 14. Cargo workspace additions

Root `Cargo.toml` additions:

```toml
[workspace.dependencies]
aws-sdk-s3    = "1"
aws-config    = "1"
hmac          = "0.12"
bytes         = "1"
futures       = "0.3"
mime          = "0.3"
tempfile      = "3"
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
]
```

---

## 15. Build order (v0 slice)

| # | Task | Outcome |
|---|---|---|
| 0 | Land this design doc | Reviewed contract |
| 1 | Extract `reactor-policy` from reactor-data | Shared policy engine, reactor-data tests pass |
| 2 | Workspace skeleton: add `reactor-storage` + `reactor-storage-server` | `cargo check` clean with all feature combos |
| 3 | `reactor-storage` skeleton: config, state, router, health | Binary boots, `/storage/v1/health` returns 200 |
| 4 | Metadata migrations + `MetadataStore` + `BlobStore` traits | Schema applies, trait definitions complete |
| 5 | Auth middleware → `StorageCtx` | Bearer + X-Reactor-Org resolution works |
| 6 | Buckets CRUD | Create/read/update/delete buckets |
| 7 | `FsBlobStore` adapter | Local filesystem storage works |
| 8 | Simple object PUT/GET/HEAD/DELETE | Full object lifecycle (no policies yet) |
| 9 | Policy enforcement | Per-bucket policies evaluated on object ops |
| 10 | `S3BlobStore` adapter | S3-compatible storage works |
| 11 | S3-style multipart uploads | Large file uploads work |
| 12 | Signed URLs | HMAC (FS) and native presign (S3) work |
| 13 | Audit + observability | Tracing, metrics, audit events |
| 14 | `doctor` + README + conformance harness | 4-cell matrix passes |

### v0 exit checklist

- [ ] `reactor-storage-server` boots against empty Postgres → migrations apply, doctor green
- [ ] Bucket CRUD: create → list → update → delete (cascade)
- [ ] Object CRUD: PUT → GET → HEAD → DELETE with correct headers
- [ ] Range requests: `Range: bytes=0-1023` returns 206 with correct content
- [ ] Policy denial: returns 403 with `policy_denied` code
- [ ] Public bucket: anonymous GET works
- [ ] Multipart: 3-part upload completes, sha256 matches
- [ ] Signed URL: FS (HMAC) and S3 (native) both work
- [ ] `X-Reactor-Org` slug resolution works
- [ ] Audit: `_reactor_storage.audit_events` populated for all mutations
- [ ] Conformance: all scenarios pass on `{FS, S3} × {InProcess, Remote}` matrix

---

## 16. Decision log

| Question | Decision | Rationale |
|---|---|---|
| **v0 backends** | Both FS and S3 ship at v0 | FS needed for G1/G2, S3 for G3. Single trait proves portability. |
| **Bucket model** | Named buckets per org with `public` flag | S3/Supabase-familiar; public flag enables anonymous reads without tokens. |
| **Authorization model** | Two-layer: permissions + policies | Permissions gate bucket access; policies allow fine-grained object-level rules. |
| **Policy engine** | Extract to shared `reactor-policy` crate | Avoids duplication; storage and data use identical DSL. |
| **Upload protocol** | Simple PUT + S3-style multipart | TUS is v0.2; multipart is sufficient for large files. |
| **Signed URLs** | HMAC for FS, native presign for S3 | FS needs server in path; S3 doesn't. Same API surface. |
| **S3 layout** | Single physical bucket + key prefix | Avoids bucket-count limits; simpler IAM. |
| **Key sanitisation** | Reject `..`, null, control chars; max 1024 | Security + S3 compatibility. |

---

## 17. Open questions (deferred)

1. **Multipart cleanup**: Abandoned uploads accumulate. Need a background sweeper (v0.2).
2. **Versioning**: S3 supports object versioning; should we expose it? Evaluate based on usage.
3. **CDN integration**: For G3c, objects should be served through a CDN. Design the cache-invalidation story.

---

*End of design doc. Land code against checklist §15 in order, one PR per row, this doc updated as decisions change.*
