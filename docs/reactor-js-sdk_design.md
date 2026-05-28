# Reactor JavaScript/TypeScript SDK Design

> Design document for `@reactor/*` — the official TypeScript SDK for Reactor

## Status: Draft
## Author: AI Assistant
## Date: 2026-05-17

---

## 1. Overview

### Problem Statement

Today, applications integrating with Reactor must hand-roll HTTP calls against the raw REST API:

```ts
const res = await fetch('/auth/v1/login', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ email, password }),
});
const data = await res.json();
localStorage.setItem('access_token', data.access_token);
```

This is brittle, untyped, lacks token refresh, has no session persistence, no error normalization, no retries, and no consistent envelope handling across capabilities. Developers building on Reactor (including our own `reactor.cloud` Astro site) need a Supabase-class developer experience.

### Goals

1. **Single import, fluent API**: `const reactor = createClient(url, key); reactor.auth.signIn(...); reactor.from('posts').select(...)`
2. **Full coverage**: auth, data, storage, functions, jobs, sites — every capability is reachable
3. **Type-safe**: TypeScript-first, with generated DB types via `reactor types generate`
4. **Isomorphic**: works in browsers, Node 20+, Bun, Deno, edge runtimes (Cloudflare Workers, Vercel Edge)
5. **Auto-managed sessions**: silent token refresh, persistence adapters, multi-tab sync
6. **Tree-shakable**: bundlers can drop capabilities you don't use
7. **Zero runtime deps where possible** (fetch is global; only optional `ws` for realtime)
8. **Generated low-level + hand-written ergonomic layer** — OpenAPI as the contract

### Non-Goals (v0)

- React/Vue/Svelte hooks packages (separate `@reactor/react` etc. — phase 2)
- Realtime subscriptions over WebSocket (phase 2; reserve API surface now)
- Admin SDK for managing Reactor itself (use `reactor-cli` or `reactor-client` Rust crate)
- Code generation for non-DB schemas (functions/jobs types come later)

---

## 2. Package Architecture

### Monorepo Layout

The SDK lives in the existing Reactor monorepo under `sdks/js/`:

```
sdks/js/
├── package.json                     # pnpm workspace root
├── pnpm-workspace.yaml
├── tsconfig.base.json
├── turbo.json                       # build/test orchestration
├── packages/
│   ├── reactor-js/                  # @reactor/client — umbrella, what users install
│   ├── auth-js/                     # @reactor/auth
│   ├── data-js/                     # @reactor/data
│   ├── storage-js/                  # @reactor/storage
│   ├── functions-js/                # @reactor/functions
│   ├── jobs-js/                     # @reactor/jobs
│   ├── sites-js/                    # @reactor/sites (admin-only, niche)
│   ├── shared/                      # @reactor/shared — fetch helpers, types, errors
│   └── realtime-js/                 # @reactor/realtime (stub for phase 2)
├── examples/
│   ├── astro-reactor-cloud/         # the existing reactor.cloud site, migrated
│   ├── next-app/                    # Next.js example
│   ├── node-script/                 # server-side example
│   └── cf-worker/                   # edge runtime example
└── tests/
    ├── unit/                        # vitest, mocked fetch
    └── integration/                 # against real reactor-server in CI
```

### Package Boundaries

| Package | Purpose | Depends on |
|---|---|---|
| `@reactor/shared` | Fetch wrapper, error types, envelope parsers, JWT decode, query string helpers | nothing |
| `@reactor/auth` | Sign up/in/out, session, tokens, refresh, orgs, invites, verify, MFA (future) | `shared` |
| `@reactor/data` | PostgREST-style query builder, CRUD, RPC | `shared` |
| `@reactor/storage` | Buckets, upload (multipart, resumable, signed), download, signed URLs | `shared` |
| `@reactor/functions` | Invoke, stream (SSE), logs (tail), env (admin) | `shared` |
| `@reactor/jobs` | Trigger, status, list runs, dlq | `shared` |
| `@reactor/sites` | Admin deploys, rollbacks, domains (only useful for build pipelines) | `shared` |
| `@reactor/realtime` | (phase 2) WebSocket channels, postgres changes, presence | `shared`, `auth` |
| `@reactor/client` | Composes the above; the package end users install | all of the above |

Each sub-package is independently published; users can `import { AuthClient } from '@reactor/auth'` if they want a minimal install.

### Build Output

- **ESM-first**, with CJS fallback via package.json `exports` conditionals
- Bundle target: ES2022, `module: "esnext"`, `moduleResolution: "bundler"`
- Built with `tsup` (single command, emits `.js`, `.cjs`, `.d.ts`, sourcemaps)
- Each package declares `"sideEffects": false` for tree-shaking

---

## 3. The Public API

### Construction

```ts
import { createClient } from '@reactor/client';

const reactor = createClient('https://reactor.cloud', {
  // Project key (anon key) — safe to ship in browser bundles
  key: 'rk_pub_...',
  // Optional: which org context to default to
  org: 'acme',
  // Optional: override fetch (for Node 18, edge runtimes, retries, tracing)
  fetch: customFetch,
  // Session persistence (default: localStorage in browser, in-memory elsewhere)
  auth: {
    storage: customStorageAdapter,
    storageKey: 'reactor.session',     // default
    autoRefresh: true,                  // default true
    persistSession: true,               // default true
    detectSessionInUrl: true,           // default true (for OAuth callbacks)
  },
  // Global headers (e.g. for tracing)
  headers: { 'x-app-version': '1.2.3' },
  // Optional: structured logger
  logger: console,
});
```

### Auth

```ts
// Sign up — emails verification link automatically
const { user, session } = await reactor.auth.signUp({
  email: 'jane@acme.com',
  password: '...',
  metadata: { name: 'Jane' },
});

// Sign in
const { user, session } = await reactor.auth.signIn({ email, password });

// Sign out (revokes refresh token server-side)
await reactor.auth.signOut();

// Current session (cached, refreshes silently if near expiry)
const session = await reactor.auth.getSession();
const user = await reactor.auth.getUser();

// Listen for auth state changes (login, logout, token refresh)
const { unsubscribe } = reactor.auth.onAuthStateChange((event, session) => {
  // event: 'INITIAL_SESSION' | 'SIGNED_IN' | 'SIGNED_OUT' | 'TOKEN_REFRESHED' | 'USER_UPDATED'
});

// Email verification
await reactor.auth.verifyEmail(token);
await reactor.auth.resendVerification({ email });

// Password reset
await reactor.auth.requestPasswordReset({ email });
await reactor.auth.confirmPasswordReset({ token, newPassword });

// Update profile
await reactor.auth.updateUser({ email, password, metadata });

// Orgs & membership
const orgs = await reactor.auth.orgs.list();
const org = await reactor.auth.orgs.create({ slug: 'acme', name: 'Acme Inc' });
const members = await reactor.auth.orgs.members(orgId).list();
await reactor.auth.orgs.members(orgId).invite({ email, roleId });
await reactor.auth.orgs.invitations.accept({ token });

// Permissions
const perms = await reactor.auth.permissions.get({ org: 'acme' });
const allowed = await reactor.auth.permissions.check(['data:read'], { org: 'acme' });

// API keys (for server-to-server use)
const key = await reactor.auth.apiKeys.create({ name: 'ci-key', scopes: ['data:*'] });
await reactor.auth.apiKeys.revoke(keyId);
```

### Data (PostgREST-style query builder)

The query builder mirrors Supabase's API but with Reactor's filter dialect. URL encoding is handled internally to match `reactor-data/src/query/`.

```ts
// SELECT
const { data, error } = await reactor
  .from<Post>('posts')
  .select('id, title, author:users(name, avatar)')
  .eq('published', true)
  .gte('views', 100)
  .order('created_at', { ascending: false })
  .limit(20)
  .range(0, 19);

// SELECT single
const { data } = await reactor.from('posts').select('*').eq('id', id).single();
const { data } = await reactor.from('posts').select('*').eq('id', id).maybeSingle();

// INSERT
const { data } = await reactor.from('posts').insert({ title, body }).select().single();
const { data } = await reactor.from('posts').insert([row1, row2]).select();

// UPSERT
await reactor.from('posts').upsert({ id, title }, { onConflict: 'id' });

// UPDATE
await reactor.from('posts').update({ published: true }).eq('id', id);

// DELETE
await reactor.from('posts').delete().eq('id', id);

// Filter operators (mirror reactor-data dialect)
.eq(col, val) .neq() .gt() .gte() .lt() .lte()
.like() .ilike() .is() .in() .contains() .containedBy()
.rangeGt() .rangeGte() .rangeLt() .rangeLte() .rangeAdjacent() .overlaps()
.textSearch(col, query, { type: 'plain' | 'phrase' | 'websearch', config })
.match(obj)         // shorthand for AND of .eq()s
.not(col, op, val)
.or('age.gte.10,name.eq.Jane')
.filter(col, op, val)  // escape hatch

// Modifiers
.select('id,title,user:users(name)')
.order(col, { ascending, nullsFirst })
.limit(n)
.range(from, to)
.abortSignal(controller.signal)
.returns<MyType>()    // type override
.csv()                // returns text/csv
.explain({ analyze: true })  // returns query plan

// Embedded resources (joins)
.select('*, comments(*)')
.select('*, comments!inner(*)')        // inner join
.select('*, comments(*, author:users(*))')  // nested

// Counting
.select('*', { count: 'exact' | 'planned' | 'estimated' })

// RPC (call a function exposed via reactor-data)
const { data } = await reactor.rpc<MyArgs, MyResult>('search', { q: 'rust' });
```

### Storage

```ts
const bucket = reactor.storage.from('avatars');

// Upload
const { data, error } = await bucket.upload('users/me.jpg', file, {
  contentType: 'image/jpeg',
  cacheControl: '3600',
  upsert: false,
  metadata: { uploadedBy: userId },
});

// Resumable upload (for large files, uses tus protocol or multipart)
const upload = bucket.uploadResumable('big-video.mp4', file);
upload.on('progress', (p) => console.log(p.loaded / p.total));
upload.on('error', (e) => ...);
await upload.start();

// Download
const blob = await bucket.download('users/me.jpg');
const stream = await bucket.downloadStream('big-video.mp4');

// Signed URLs (for private buckets)
const { url } = await bucket.createSignedUrl('users/me.jpg', 3600);
const urls = await bucket.createSignedUrls(paths, 3600);

// Public URL (for public buckets — no signing)
const url = bucket.getPublicUrl('users/me.jpg');

// List
const { data } = await bucket.list('users/', { limit: 100, offset: 0, sortBy: { column: 'name', order: 'asc' } });

// Delete
await bucket.remove(['users/me.jpg', 'users/old.jpg']);

// Move/copy
await bucket.move('users/me.jpg', 'archived/me.jpg');
await bucket.copy('users/me.jpg', 'backup/me.jpg');

// Bucket admin (requires admin auth)
await reactor.storage.createBucket('public-assets', { public: true });
await reactor.storage.deleteBucket('old-bucket');
await reactor.storage.listBuckets();
```

### Functions

```ts
// Invoke (returns JSON by default)
const { data, error } = await reactor.functions.invoke('send-email', {
  body: { to, subject, html },
  headers: { 'x-trace-id': '...' },
});

// Streaming (SSE — function streams chunks back)
const stream = await reactor.functions.invokeStream('generate-report', { body: { id } });
for await (const chunk of stream) {
  console.log(chunk);
}

// Raw response (for non-JSON return types)
const response = await reactor.functions.invokeRaw('image-process', { body: file });
const blob = await response.blob();

// Admin (deploy, manage envs) — gated by admin auth
await reactor.functions.deploy('send-email', bundleBytes, { version: '1.2.0' });
await reactor.functions.env.set('send-email', { RESEND_KEY: '...' });
await reactor.functions.env.list('send-email');
await reactor.functions.logs.tail('send-email', { since: '5m', onLog: (l) => ... });
await reactor.functions.versions.list('send-email');
await reactor.functions.versions.rollback('send-email', 'v1.1.0');
```

### Jobs

```ts
// Trigger a job
const { runId } = await reactor.jobs.trigger('reindex-search', {
  payload: { docs: [...] },
  idempotencyKey: 'reindex-2026-05-17',
});

// Check status
const run = await reactor.jobs.runs.get(runId);
// run: { id, jobName, status: 'pending'|'running'|'succeeded'|'failed', startedAt, ... }

// List runs
const { data } = await reactor.jobs.runs.list({
  jobName: 'reindex-search',
  status: 'failed',
  limit: 50,
});

// Cancel
await reactor.jobs.runs.cancel(runId);

// Wait for completion (polls; uses backoff)
const finalRun = await reactor.jobs.runs.wait(runId, { timeoutMs: 60_000 });

// DLQ
await reactor.jobs.dlq.list();
await reactor.jobs.dlq.retry(runId);

// Triggers (admin)
await reactor.jobs.triggers.create('reindex-search', { cron: '0 * * * *' });
await reactor.jobs.triggers.list('reindex-search');
```

### Sites (admin-only)

Mostly used by `reactor-cli` and CI pipelines; included for completeness.

```ts
await reactor.sites.deploy('marketing', bundleBytes, { framework: 'astro' });
await reactor.sites.domains.add('marketing', 'reactor.cloud');
await reactor.sites.deployments.list('marketing');
await reactor.sites.deployments.rollback('marketing', deploymentId);
```

### Error Handling

Every method returns `{ data, error }` Supabase-style, OR throws if you prefer. Both are first-class:

```ts
// Result style (default)
const { data, error } = await reactor.from('posts').select('*');
if (error) {
  console.error(error.code, error.message, error.hint, error.statusCode);
  return;
}

// Throw style (opt-in per call)
try {
  const data = await reactor.from('posts').select('*').throwOnError();
} catch (e) {
  if (e instanceof ReactorError) ...
}
```

Error class hierarchy:

```ts
class ReactorError extends Error {
  code: string;          // e.g. 'invalid_credentials'
  statusCode: number;    // HTTP status
  hint?: string;
  cause?: Error;         // network errors, abort, etc.
}

class AuthError extends ReactorError {}     // 401, 403
class ValidationError extends ReactorError {}  // 400, 422
class NotFoundError extends ReactorError {}    // 404
class ConflictError extends ReactorError {}    // 409
class RateLimitError extends ReactorError {    // 429
  retryAfter?: number;
}
class ServerError extends ReactorError {}      // 5xx
class NetworkError extends ReactorError {}     // fetch failed, aborted
```

Errors are parsed from the standard envelope returned by all Reactor capabilities (`{ error: { code, message, status, hint? } }`).

---

## 4. Type Safety: Generated DB Types

Following Supabase's pattern, types for the data layer are generated from the project schema:

```bash
# Via reactor-cli
reactor types generate --output src/database.types.ts

# Or via SDK CLI
npx reactor-types generate --url https://reactor.cloud --key rk_pub_... > types.ts
```

Then:

```ts
import type { Database } from './database.types';

const reactor = createClient<Database>(url, { key });

const { data } = await reactor.from('posts').select('id, title');
// data is typed as { id: string; title: string }[] | null
```

Implementation: the dialect AST in `crates/reactor-data/src/dialect/ast.rs` already knows table column types. We add a `reactor-data` admin endpoint `GET /data/v1/_admin/types/typescript` that emits TS interfaces. The CLI just fetches and writes.

Type shape mirrors Supabase's:

```ts
export interface Database {
  public: {
    Tables: {
      posts: {
        Row: { id: string; title: string; body: string; author_id: string; created_at: string };
        Insert: { id?: string; title: string; body: string; author_id: string; created_at?: string };
        Update: Partial<{ id: string; title: string; body: string; author_id: string; created_at: string }>;
        Relationships: [{ foreignKeyName: 'posts_author_id_fkey'; columns: ['author_id']; referencedRelation: 'users'; referencedColumns: ['id']; }];
      };
      // ...
    };
    Views: { ... };
    Functions: { search: { Args: { q: string }; Returns: PostRow[] } };
    Enums: { ... };
  };
}
```

---

## 5. Sessions & Auth Internals

### Storage adapter

```ts
interface StorageAdapter {
  getItem(key: string): string | null | Promise<string | null>;
  setItem(key: string, value: string): void | Promise<void>;
  removeItem(key: string): void | Promise<void>;
}
```

Defaults:
- Browser: `localStorage`
- Browser (when `localStorage` unavailable, e.g. private mode): `sessionStorage` then in-memory
- Node/Bun/Deno: in-memory (apps set their own)
- React Native: pass `AsyncStorage` explicitly

### Token refresh

- Access token TTL: 1h (configurable server-side; SDK reads `exp` from JWT)
- Refresh token TTL: 30d
- SDK schedules a refresh at `exp - 60s`
- Concurrent refresh requests are deduplicated via an in-flight promise
- On refresh failure → emit `SIGNED_OUT`, clear storage

### Multi-tab sync

Browsers: listen to `storage` events. When tab A refreshes the token, tab B picks it up automatically and updates its in-memory cache.

### Session detection from URL

For email verification, password reset, OAuth callbacks — the SDK auto-parses `?token=...` or hash fragment `#access_token=...&refresh_token=...` on page load, persists, then cleans the URL.

---

## 6. The Wire Protocol

### Project key vs JWT vs Admin token

Reactor currently has two tiers:
- `REACTOR_ADMIN_TOKEN` — full system access, server-side only
- User JWT — per-user, scoped by org/permissions

The SDK introduces a **third tier**:

- **Project (anon) key** (`rk_pub_*`) — public, safe to ship in browser bundles. Acts as the "project identity" so the server knows which Reactor project this request belongs to. Required for all SDK requests. Authorization is enforced by RLS policies in `reactor-data`, not by the key itself.

Header convention:
```
Authorization: Bearer <user-jwt>           (if signed in)
X-Reactor-Project-Key: rk_pub_...           (always)
```

For server-side use, the SDK also accepts a service role key (`rk_srv_*`) that bypasses RLS — same as Supabase's service role.

### Request shape

All requests:
- `Content-Type: application/json` for JSON bodies
- `Accept: application/json`
- `X-Reactor-Project-Key: <key>` always
- `Authorization: Bearer <jwt>` when authed
- `X-Reactor-Client: js/<version>` for analytics

All responses follow Reactor's standard envelope. The SDK unwraps it; users get the inner `data` directly.

---

## 7. OpenAPI Generation

To keep types in sync, we generate the low-level client from server-emitted OpenAPI specs.

### Server side

Add `utoipa` to each capability. Existing Axum routes + serde structs already have most of what's needed — annotations just mark the routes:

```rust
#[utoipa::path(
    post,
    path = "/auth/v1/signup",
    request_body = SignupRequest,
    responses(
        (status = 200, body = SignupResponse),
        (status = 409, body = ErrorResponse, description = "email exists"),
    ),
    tag = "auth"
)]
pub async fn signup(...) { ... }
```

Each `reactor-{cap}` crate exposes:
- `pub fn openapi() -> utoipa::openapi::OpenApi`
- A route at `/{cap}/v1/openapi.json`

The server (`reactor-server`) merges them at `/_api/openapi.json`.

### Client side

`sdks/js/scripts/generate.ts`:
1. Fetch `http://localhost:8000/_api/openapi.json` (or read a checked-in file at `sdks/js/openapi/spec.json`)
2. Run `openapi-typescript` → emits `sdks/js/packages/shared/src/generated/api.d.ts`
3. The hand-written packages import these types but never expose the raw shape

The generated types are checked into the repo for reproducible builds. CI verifies regeneration produces no diff.

---

## 8. Examples — Migrating reactor.cloud

The Astro site at `apps/reactor-cloud/sites/reactor-cloud/` currently has raw fetch calls in `login.astro`. After SDK rollout:

```astro
<!-- login.astro -->
<script>
  import { reactor } from '@/lib/reactor';

  const form = document.getElementById('login-form') as HTMLFormElement;
  form?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const data = new FormData(form);
    const { error } = await reactor.auth.signIn({
      email: String(data.get('email')),
      password: String(data.get('password')),
    });
    if (error) {
      alert(error.message);
      return;
    }
    window.location.href = '/app';
  });
</script>
```

```ts
// src/lib/reactor.ts
import { createClient } from '@reactor/client';
import type { Database } from './database.types';

export const reactor = createClient<Database>(
  import.meta.env.PUBLIC_REACTOR_URL,
  { key: import.meta.env.PUBLIC_REACTOR_KEY }
);
```

This becomes the canonical SDK usage example, and we keep `apps/reactor-cloud/` as the smoke test for every SDK release.

---

## 9. Versioning & Releases

- **Semver per package**, with synchronized major versions across the family
- `@reactor/client` is the umbrella; pinning it pins everything
- Releases via Changesets (`pnpm changeset` workflow)
- Published to npm under `@reactor` scope
- Each release tags the OpenAPI spec version it was generated against
- A compat matrix in the README maps SDK version → server version

---

## 10. Testing Strategy

| Layer | Tooling | Scope |
|---|---|---|
| Unit | Vitest + MSW (mocked fetch) | Query builder URL generation, error parsing, refresh logic, storage adapters |
| Integration | Vitest against `reactor-server` in Docker | End-to-end flows for each capability |
| Type tests | `tsd` or `expect-type` | Generic inference, especially for `from<T>()` and `rpc<A,R>()` |
| Examples | Playwright | The Astro example + Next.js example actually log in |
| Bundle size | `size-limit` | Tracked per-package; CI fails on regression |
| Browser matrix | Playwright (Chromium, WebKit, Firefox) | Auth flows, multi-tab sync |

CI runs all of these on every PR via Turbo + GitHub Actions.

---

## 11. Implementation Phases

### Phase 0 — Foundation (1 week)
- Set up `sdks/js/` pnpm workspace, tsup, Turbo, Changesets, Vitest
- Implement `@reactor/shared`: fetch wrapper, error parsing, JWT decode, envelope handling
- Add `utoipa` to `reactor-auth` (one capability proves the pattern)
- Generate `api.d.ts` from `/auth/v1/openapi.json`

### Phase 1 — Auth (1 week)
- Build `@reactor/auth`: signUp, signIn, signOut, getSession, getUser, onAuthStateChange
- Storage adapter, token refresh, multi-tab sync
- Email verification, password reset
- Migrate `login.astro` and `signup.astro` to use the SDK
- Ship `@reactor/auth@0.1.0`

### Phase 2 — Data (1.5 weeks)
- Build `@reactor/data`: query builder mirroring PostgREST conventions
- All filter operators, ordering, pagination, embedded selects, counting
- RPC support
- TS type generation via `reactor types generate`
- Ship `@reactor/data@0.1.0`

### Phase 3 — Storage + Functions + Jobs (1.5 weeks)
- `@reactor/storage`: upload (simple + resumable), download, signed URLs, list, delete
- `@reactor/functions`: invoke, invokeStream, raw
- `@reactor/jobs`: trigger, runs.get/list/cancel/wait
- Ship 0.1.0 for each

### Phase 4 — Umbrella + Docs (1 week)
- `@reactor/client`: compose everything
- Sites admin (minimal), org & invitation flows on auth
- Reference docs (TypeDoc + hand-written guides)
- Example apps (Next.js, Bun, CF Worker)
- Ship `@reactor/client@0.1.0` publicly

### Phase 5 — Polish & Realtime (parallel, ongoing)
- `@reactor/realtime`: WebSocket protocol design + implementation
- Framework adapters: `@reactor/react`, `@reactor/svelte`
- React Native compatibility verified
- 1.0.0 stabilization

Total: ~5–6 weeks of focused work for a complete v0.1 of the JS SDK family.

---

## 12. Open Questions

1. **Anon key issuance**: who/where mints `rk_pub_*` and `rk_srv_*`? Likely `reactor-cloud-api` (the control plane from the previous design), per-project. Self-hosted users get them via `reactor-cli`.
2. **RLS terminology**: do we call it RLS (Postgres native) or "Reactor policies" (since policies live in `reactor-data/src/policy/`)? Affects docs everywhere.
3. **Realtime protocol**: roll our own WS protocol over `pg_notify`, or adopt Phoenix Channels / Supabase Realtime wire format for ecosystem compatibility?
4. **Resumable uploads**: tus.io protocol (battle-tested) vs custom multipart with checkpoint? `reactor-storage` doesn't support resumable yet — needs a server-side decision first.
5. **Streaming functions**: SSE (already in use) vs WebSockets vs Web Streams over fetch. SSE is simplest and works through proxies; recommend sticking with it.
6. **Service role key in browsers**: hard-block at the SDK level if `rk_srv_*` is detected in a browser context? Likely yes — print a loud warning, refuse to send.
7. **Bundle size targets**: `@reactor/client` minified+gzipped target — propose <15kb without realtime, <25kb with.
8. **Deno publishing**: also publish to JSR? Deno can consume npm packages directly, but JSR is the Deno-native registry. Probably worth doing for visibility.

---

## 13. Out of Scope (Future Work)

- React/Vue/Svelte hook packages (`@reactor/react`, etc.) — separate effort once core is stable
- Admin SDK for project/infra management — covered by Rust `reactor-client` + CLI
- GraphQL gateway over data — interesting but big; defer
- Local dev mock server — for offline development; defer
- CLI codegen for functions/jobs payload types — once their schemas stabilize
