# `reactor-data` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** Second crate of the Reactor.cloud BaaS. Owns the Data capability per `docs/ReactorCloud_spec.md` §4.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, SQL dialect, policy DSL, internal types — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Expose a **PostgREST-shaped HTTP surface** over a relational store, modelled on Supabase Data so existing client patterns are immediately usable.
2. Be **dialect-portable from the source layer down**: every migration is written once in a Reactor SQL subset and compiles to native SQL for any backend. Postgres ships in v0; SQLite ships in v0.2; further backends are additive.
3. **Own the policy enforcement engine in Rust.** Native Postgres RLS is *not* used. A single Rust-side policy compiler and evaluator is the only enforcement point. This makes the policy DSL first-class, identical across backends, and able to call into `reactor-auth` predicates (`auth.has_permission(...)`) directly.
4. Be the **first real consumer of `reactor_core::auth::AuthClient`** — exercises both `InProcessAuthClient` and `RemoteAuthClient` end-to-end and proves the deployment-topology-as-runtime-choice claim.
5. Be the only crate allowed to touch the user's data schema *and* the `_reactor_data.*` metadata schema.

## 2. Non-goals (v0)

- **Realtime subscriptions** — SSE/WS streaming of table changes is v0.2.
- **SQLite adapter** — the dialect compiler and portability lint ship in v0 so migrations are authored portably from day one, but only the Postgres adapter is wired at runtime. SQLite adapter is v0.2.
- **Type generation** (`reactor db generate-types`) — schema metadata is captured in v0 so generation is mechanical later, but the codegen itself is v0.2.
- **Rust-defined RPC functions** — only SQL-defined functions (registered via migrations) in v0.
- **Full-text search operators** (`fts`, `plfts`, `phfts`, `wfts`) — defer to a later pass once we settle on a tsvector strategy.
- **CDC / logical replication.**
- **Per-row encryption / column masking.**
- **Visual schema editor / studio.** CLI + types are the v0 DX.

## 3. Crate layout

```
crates/
├── reactor-core/                  # (existing) shared types, IDs, AuthClient trait, etc.
│
├── reactor-data/                  # the data library
│   ├── Cargo.toml
│   ├── migrations/                # sqlx migrations against _reactor_data.* metadata schema
│   │   ├── 001_metadata.sql
│   │   └── 002_policies.sql
│   ├── DESIGN.md                  # (this file, mirrored)
│   └── src/
│       ├── lib.rs                 # crate root, re-exports
│       ├── config.rs              # DataConfig
│       ├── router.rs              # axum Router::new(state) factory
│       ├── state.rs               # DataState { store, auth: Arc<dyn AuthClient>, ... }
│       ├── service.rs             # DataService (orchestrates query → policy → exec)
│       ├── error.rs               # DataError
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── crud.rs            # GET/POST/PATCH/DELETE /data/v1/{table}
│       │   ├── rpc.rs             # POST /data/v1/rpc/{fn}
│       │   ├── health.rs
│       │   └── internal.rs        # internal introspection (gated by secret)
│       │
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth.rs            # bearer + X-Reactor-Org → AuthCtx
│       │
│       ├── store/
│       │   ├── mod.rs             # DataStore trait
│       │   └── postgres.rs        # PgDataStore impl (sqlx)
│       │
│       ├── dialect/               # portable SQL subset
│       │   ├── mod.rs
│       │   ├── ast.rs             # canonical Reactor SQL AST
│       │   ├── parser.rs          # sqlparser-rs wrapper + lint pass
│       │   ├── lint.rs            # rejects PG-only / SQLite-only constructs
│       │   ├── types.rs           # reactor_id ↔ uuid/text mapping etc.
│       │   └── emit_postgres.rs   # AST → PG DDL
│       │
│       ├── migrate/
│       │   ├── mod.rs
│       │   ├── runner.rs          # discover, parse, apply, record
│       │   └── source.rs          # in-memory or filesystem migration source
│       │
│       ├── policy/
│       │   ├── mod.rs
│       │   ├── grammar.rs         # `policy ... on ... using/check ...` parser
│       │   ├── ast.rs             # PolicyExpr (Bool tree referencing columns + auth.*)
│       │   ├── compile.rs         # PolicyExpr × AuthCtx → SQL fragment OR Decision
│       │   ├── eval.rs            # CHECK evaluator (insert/update payload)
│       │   └── builtins.rs        # auth.user_id() / auth.org_id() / auth.has_permission()
│       │
│       ├── query/                 # PostgREST-shaped HTTP query interpretation
│       │   ├── mod.rs
│       │   ├── filter.rs          # ?col=eq.value, and(...), or(...), in.(...), is.null, ...
│       │   ├── select.rs          # ?select=id,title,author(id,name) (embedded resources)
│       │   ├── order.rs           # ?order=col.desc.nullsfirst
│       │   ├── pagination.rs      # ?limit=, ?offset=, Range header
│       │   ├── prefer.rs          # Prefer: return=, count=, resolution=
│       │   └── plan.rs            # QueryPlan: tables, filters, projection, joins
│       │
│       ├── execute/
│       │   ├── mod.rs             # Planner: QueryPlan + policies → SqlAst → rows
│       │   └── format.rs          # rows → JSON, count headers, Range responses
│       │
│       └── audit.rs               # mutation audit-event writer (in-txn)
│
└── reactor-data-server/           # standalone bin: `reactor-data-server`
    ├── Cargo.toml
    └── src/main.rs                # axum bind + tracing + migrate + serve
```

Conventions:
- `reactor-data` depends on `reactor-core` (for `ReactorId`, `AuthClient`, `AuthCtx`, `Claims`, errors) and **never** on `reactor-auth`. Auth is consumed through the trait, full stop.
- The Postgres adapter is *inside* `reactor-data` as `store/postgres.rs` for v0 (a single backend doesn't justify a separate crate yet). When the SQLite adapter lands in v0.2, both move to `crates/reactor-data-postgres/` and `crates/reactor-data-sqlite/` and `reactor-data` becomes adapter-agnostic.
- All HTTP path/query parsing is in `query/`. All SQL emission is in `execute/`. They meet only at `QueryPlan`. This keeps the PostgREST layer and the SQL layer independently testable.

---

## 4. Core types

### 4.1 ID & types reuse

`reactor_id` (the SQL type used in migrations) maps to:

| SQL type | Postgres | SQLite (v0.2) | Rust |
|---|---|---|---|
| `reactor_id` | `uuid` (with check `version=7`) | `text` (36-char canonical) | `ReactorId` (from `reactor-core`) |
| `text` | `text` | `text` | `String` |
| `bool` | `boolean` | `integer` (0/1) | `bool` |
| `int` | `integer` | `integer` | `i32` |
| `bigint` | `bigint` | `integer` | `i64` |
| `float` | `double precision` | `real` | `f64` |
| `timestamptz` | `timestamptz` | `text` (ISO 8601 UTC) | `chrono::DateTime<Utc>` |
| `jsonb` | `jsonb` | `text` (JSON) | `serde_json::Value` |
| `bytea` | `bytea` | `blob` | `Vec<u8>` |

`reactor_id` is the only Reactor-specific type. It compiles to the natural backend type and `ReactorId` parses/serialises from both shapes.

### 4.2 `DataCtx` (request-local)

Constructed by middleware once per request from `AuthCtx`:

```rust
// reactor-data/src/state.rs
#[derive(Debug, Clone)]
pub struct DataCtx {
    pub auth:      AuthCtx,           // from reactor-core::auth
    pub request_id: String,
    pub table_ns:  String,            // "public" by default; future per-org schemas
}

impl DataCtx {
    pub fn user_id(&self) -> &UserId { /* ... */ }
    pub fn active_org(&self) -> Option<&OrgId> { self.auth.active_org.as_ref() }
    pub fn has_permission(&self, perm: &str) -> bool {
        // pure-local check against ctx.auth.permissions, no roundtrip
    }
}
```

### 4.3 `DataStore` trait

```rust
// reactor-data/src/store/mod.rs
#[async_trait]
pub trait DataStore: Send + Sync + 'static {
    type Tx<'a>: DataTx where Self: 'a;

    async fn begin(&self) -> Result<Self::Tx<'_>, DataError>;

    async fn introspect_schema(&self) -> Result<SchemaSnapshot, DataError>;
}

#[async_trait]
pub trait DataTx: Send {
    async fn execute_raw(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, DataError>;
    async fn fetch_rows  (&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>, DataError>;
    async fn commit(self) -> Result<(), DataError>;
    async fn rollback(self) -> Result<(), DataError>;
}
```

The trait deliberately does **not** expose typed CRUD. It is a low-level cursor — typed query construction is the planner's job, not the store's. This is what makes a SQLite adapter incremental: a `SqliteDataStore` just needs to implement the same two methods.

`SchemaSnapshot` captures tables, columns, primary keys, foreign keys, indexes, and compiled policies — populated from `_reactor_data.*` metadata + a backend probe.

### 4.4 No `DataClient` trait

Unlike `AuthClient`, **there is no `DataClient` trait** in `reactor-core`. Other capabilities (functions, jobs) consume reactor-data via its HTTP surface, the same way external clients do. If we ever need in-process embedding for performance, `reactor-data::router(state)` is the embeddable surface — pass through an in-memory `tower::Service` instead of a TCP socket. Wrapping arbitrary CRUD over arbitrary tables behind a Rust trait would be inventing a worse PostgREST.

---

## 5. HTTP surface (v0)

PostgREST-shaped. Diverges only where noted.

### 5.1 CRUD endpoints

```
GET    /data/v1/{table}                 -- select with filters, projection, order, paging
POST   /data/v1/{table}                 -- insert single row or array
PATCH  /data/v1/{table}?col=op.value    -- update rows matching filter
DELETE /data/v1/{table}?col=op.value    -- delete rows matching filter

POST   /data/v1/rpc/{function_name}     -- call a SQL-defined function
```

Read-only routes (`GET`, `POST /rpc` if function is marked `stable`/`immutable`) require at least one permission of `data:{table}:read`. Mutations require `data:{table}:write`. Wildcard segments (`data:*:*`) match.

### 5.2 Filter grammar

Querystring keys that match a column name are treated as filters; everything else is reserved (`select`, `order`, `limit`, `offset`, `and`, `or`).

| Operator | Example | Meaning |
|---|---|---|
| `eq` | `?id=eq.5` | `=` |
| `neq` | `?id=neq.5` | `<>` |
| `gt` `gte` `lt` `lte` | `?n=gt.10` | comparison |
| `like` | `?title=like.foo*` | SQL `LIKE` (`*` → `%`) |
| `ilike` | `?title=ilike.FOO*` | case-insensitive |
| `in` | `?id=in.(1,2,3)` | `IN (...)` |
| `is` | `?col=is.null`, `?col=is.true` | `IS NULL` / `IS TRUE` / `IS FALSE` |
| `not` | `?col=not.is.null`, `?col=not.eq.5` | negation prefix |
| `cs` `cd` | `?tags=cs.{a,b}` | array contains / contained-by |
| `ov` | `?range=ov.[1,5]` | range overlap (PG only; rejected by lint when SQLite enabled) |
| `and` | `?and=(id.eq.5,title.like.foo*)` | grouped conjunction |
| `or` | `?or=(id.eq.5,id.eq.7)` | grouped disjunction |

Identical to PostgREST/Supabase. Values are always treated as parameters — never interpolated.

### 5.3 Projection & embedded resources

```
GET /data/v1/posts?select=id,title,author(id,name),comments(id,body,author(name))
```

Embedded resources traverse declared foreign keys. The planner walks the FK graph from `posts.author_id → users.id` and `comments.post_id → posts.id`, then emits a single SQL statement using JSON aggregation (`jsonb_agg` / `jsonb_build_object` on PG; analogous on SQLite later). Default max depth: 5. Configurable via `REACTOR_DATA_MAX_EMBED_DEPTH`.

Embedded resources are policy-aware: each joined table's policies are applied to *that* JOIN's predicate, so a user without `data:comments:read` permission gets `[]` for the comments key, not an error.

### 5.4 Ordering, pagination, count

```
?order=created_at.desc,title.asc
?order=col.desc.nullsfirst
?limit=20&offset=40
```

Pagination also accepted via standard HTTP `Range`/`Range-Unit: items` headers (Supabase parity). Response includes `Content-Range: 40-59/237` when count is requested.

### 5.5 `Prefer` header semantics

| Prefer value | Effect |
|---|---|
| `return=representation` | Mutations respond with the affected rows (`200`/`201`) |
| `return=minimal` | Mutations respond `204` with empty body (default) |
| `count=exact` | Include exact total in `Content-Range` (extra `SELECT count(*)` runs) |
| `count=planned` | Best-effort estimate from query planner (PG only; SQLite falls back to `exact`) |
| `count=estimated` | Reads PG stats (`pg_class.reltuples`); SQLite falls back to `planned`/`exact` |
| `resolution=merge-duplicates` | INSERT does `ON CONFLICT DO UPDATE` keyed by primary key |
| `resolution=ignore-duplicates` | INSERT does `ON CONFLICT DO NOTHING` |

### 5.6 RPC

```
POST /data/v1/rpc/{function_name}
Body: { "arg1": 5, "arg2": "hello" }
→ Result of the SQL function as JSON (scalar, array, row, or rowset)
```

SQL functions are declared in migrations (§7.4). At call time, the planner:
1. Looks up the function signature from `_reactor_data.rpc_functions`.
2. Binds named JSON args to positional SQL parameters by name.
3. Wraps in `SELECT * FROM fn(...)` and executes.
4. Enforces `data:rpc:{name}:invoke` permission.

Functions can opt into row-level policy enforcement via `with security = invoker` (default: `security = definer`, which bypasses table policies inside the function body — same Postgres semantics).

### 5.7 Realtime (v0.2 placeholder)

```
GET /data/v1/{table}?subscribe=1   -- 426 Upgrade Required in v0
```

Reserved in the contract. v0 returns `426` with the documented v0.2 capability hint. v0.2 implementation: SSE first (`text/event-stream`, last-event-id resume); WS adapter on the same broadcast core when needed for Supabase JS client compatibility.

### 5.8 Headers

| Header | Meaning |
|---|---|
| `Authorization: Bearer <jwt>` | Required on every non-health request. Validated via `AuthClient::resolve_ctx`. |
| `X-Reactor-Org: <ref>` | Active org override. Forwarded verbatim to `AuthClient::resolve_ctx` as `requested_org`. **reactor-data does no slug/uuid resolution of its own** — it trusts `ctx.active_org`. |
| `Prefer: ...` | See §5.5. |
| `Range`, `Range-Unit: items` | Pagination, alternate to `?limit`/`?offset`. |
| `Content-Type: application/json` | Required on POST/PATCH bodies. CSV/upsert-from-CSV is v0.2. |

### 5.9 Error envelope

Identical shape to `reactor-auth`:

```json
{
  "error": {
    "code": "policy_denied",
    "message": "Row violates policy 'todos_tenant' on table 'todos'.",
    "status": 403,
    "request_id": "req_01HZ...",
    "details": {
      "table": "todos",
      "policy": "todos_tenant",
      "scope": "select"
    }
  }
}
```

Stable error codes (snake_case), enumerated in `reactor-data::error::DataErrorCode`. PostgREST-compatible codes (`PGRST...`) are *also* emitted in a `pgrst_code` field for clients that switch on them, but Reactor's own code is canonical.

---

## 6. Database layout

reactor-data manages two distinct concerns in the same database:

1. **User schema** — the project's tables, defined entirely in user migrations. Default schema name `public` (overridable per project).
2. **Reactor metadata** — internal bookkeeping in schema `_reactor_data`, managed by reactor-data's own embedded sqlx migrations.

### 6.1 Metadata schema (`_reactor_data`)

```sql
create schema if not exists _reactor_data;

-- 6.1.1 Migration history (Reactor's own portable migrations)
create table _reactor_data.migrations (
  version       text primary key,           -- '001_init'
  source_sha256 bytea not null,             -- sha256 of the raw migration text
  applied_at    timestamptz not null default now(),
  applied_by    text                         -- user/process that ran the migration
);

-- 6.1.2 Compiled policies (one row per policy declaration)
create table _reactor_data.policies (
  id            uuid primary key,            -- ReactorId
  table_schema  text not null,               -- 'public'
  table_name    text not null,
  name          text not null,               -- 'todos_tenant'
  scopes        text[] not null,             -- {'select','update','delete'}
  kind          text not null,               -- 'using' | 'check'
  raw_text      text not null,               -- original DSL source
  ast_json      jsonb not null,              -- serialized PolicyExpr
  created_at    timestamptz not null default now(),
  unique (table_schema, table_name, name, kind)
);
create index on _reactor_data.policies (table_schema, table_name);

-- 6.1.3 Table introspection cache (refreshed on migration apply)
create table _reactor_data.tables (
  table_schema  text not null,
  table_name    text not null,
  columns_json  jsonb not null,              -- [{ name, type, nullable, default }, ...]
  primary_key   text[] not null,
  foreign_keys  jsonb not null,              -- [{ name, columns, ref_table, ref_columns, on_delete }, ...]
  indexes       jsonb not null,
  primary key (table_schema, table_name)
);

-- 6.1.4 Registered RPC functions
create table _reactor_data.rpc_functions (
  name          text primary key,
  table_schema  text not null,
  signature     jsonb not null,              -- [{ name, sql_type, nullable }, ...]
  return_type   jsonb not null,              -- { kind: 'row'|'setof'|'scalar', schema?: ... }
  security      text not null,               -- 'definer' | 'invoker'
  raw_definition text not null
);

-- 6.1.5 Mutation audit (mirrors reactor_auth.audit_events shape)
create table _reactor_data.audit_events (
  id              uuid primary key,
  ts              timestamptz not null default now(),
  actor_user_id   uuid,
  actor_apikey_id uuid,
  org_id          uuid,
  request_id      text not null,
  event_type      text not null,             -- 'rows.insert' | 'rows.update' | 'rows.delete' | 'rpc.invoke'
  table_name      text,
  row_count       integer,
  details         jsonb not null default '{}'::jsonb
);
create index on _reactor_data.audit_events (org_id, ts desc);
create index on _reactor_data.audit_events (actor_user_id, ts desc);
```

### 6.2 User schema connection model

reactor-data connects to Postgres with a **single application role** (e.g. `reactor_data_app`) that owns and has full DML on the user schema. **No per-user `SET LOCAL` GUCs, no `SET ROLE`, no native RLS.** All access control happens in the Rust planner before SQL leaves the process.

The app role's grants in v0:
- `USAGE` on the user schema (default `public`).
- `SELECT, INSERT, UPDATE, DELETE` on all current and future tables in the user schema.
- `USAGE, SELECT` on sequences.
- `EXECUTE` on `_reactor_data` functions only.
- Membership in a least-privilege role; no superuser.

This is the lever that makes the design choice in §1.3 viable: the only path to the user's data is through reactor-data, so the Rust-side policy engine is the only enforcement point that needs to exist.

---

## 7. Portable SQL dialect

### 7.1 Goals & scope

The dialect is a **subset** of standard SQL that, post-compilation, runs natively on every supported backend. Migration files are the only place users write SQL (queries are built by reactor-data from PostgREST input). The dialect therefore covers DDL + RLS-shaped policy declarations + function declarations, **not arbitrary SELECT/INSERT** in user code.

### 7.2 Type system

See §4.1. `reactor_id` and the eight base types are the only allowed column types in v0. `jsonb` is permitted but JSONB-specific operators (`@>`, `?`, etc.) are **forbidden inside policy expressions** because they don't translate to SQLite (use `->>` for key extraction, which the compiler emits as `json_extract` on SQLite).

### 7.3 Forbidden constructs (lint-enforced)

The lint pass runs against the parsed AST before storage and rejects:

- Postgres-only types (`hstore`, `cidr`, `inet`, `tsvector`, `xml`, `box`, `point`, `geometry`, …)
- SQLite-only constructs (e.g. `WITHOUT ROWID` tables)
- `partial indexes` with non-portable predicates (whitelist of safe predicate forms)
- Native `CREATE POLICY` (use Reactor's `policy ... on ...` form instead)
- Stored procedures in PL/pgSQL or any language other than `sql` (PL/v8, plpgsql, etc.)
- `MATERIALIZED VIEW` (no SQLite equivalent)
- `WITH RECURSIVE` (allowed in v0.2 after we add a portability test matrix)
- Cross-schema references except into `_reactor_data` (which is read-only to user code)
- `EXTENSION` statements (Reactor manages extensions; user migrations can't `CREATE EXTENSION`)

### 7.4 Allowed DDL

| Statement | Notes |
|---|---|
| `CREATE TABLE` | Columns of allowed types; `PRIMARY KEY`, `FOREIGN KEY`, `UNIQUE`, `CHECK`, `DEFAULT` |
| `CREATE INDEX` | Simple expression indexes; `UNIQUE`, `WHERE` with whitelisted predicates |
| `ALTER TABLE ADD COLUMN` | With `DEFAULT` and `NOT NULL` |
| `ALTER TABLE DROP COLUMN` | |
| `ALTER TABLE RENAME` | Table and column rename |
| `DROP TABLE`, `DROP INDEX` | |
| `CREATE FUNCTION ... LANGUAGE sql AS $$ ... $$` | SQL body only; restricted SELECT/INSERT/UPDATE/DELETE inside |
| `policy <name> on <table> for <scope> using (...) check (...)` | Reactor extension, parsed by §8 grammar |

### 7.5 Compiler pipeline

```
*.sql migration file
    │
    ▼
sqlparser-rs (Postgres dialect, pre-extended)
    │ + Reactor `policy` extension parser (recursive-descent over sqlparser tokens)
    ▼
Reactor AST (dialect::ast)
    │
    ▼ lint pass (dialect::lint) ── rejects on first forbidden construct
    │
    ▼
canonicalize (e.g. `reactor_id` → backend-native type)
    │
    ├──→ emit::postgres → PG-native DDL → applied
    └──→ (v0.2) emit::sqlite → SQLite DDL → applied
```

The compiler is **migration-time only**. Once migrations are applied and metadata is populated, user queries (the PostgREST CRUD path) go through the §9 planner, which generates parameterised SQL directly from `QueryPlan` and doesn't re-enter the dialect compiler.

### 7.6 Migration source layout

Project root (defined by `reactor.toml` in spec §7) contains:

```
migrations/
  001_init.sql
  002_add_tags.sql
  ...
```

reactor-data's `MigrationRunner` discovers files lexicographically, computes sha256 of each, checks `_reactor_data.migrations` for a matching `(version, source_sha256)`, applies missing ones in a transaction, and refreshes `_reactor_data.tables`. Mismatched sha256 on a previously-applied version is a hard error (`migration_drift`).

For v0, migrations are also embedded in `reactor-data-server`'s binary via `sqlx::migrate!()` for the metadata schema itself. User migrations are *external* (from the project's `migrations/` directory, configured via `REACTOR_DATA_MIGRATIONS_DIR`).

---

## 8. Policy engine (the load-bearing decision)

### 8.1 Why one engine, not two

Native Postgres RLS is *not* used. Every read and write of user data passes through `policy::compile` first. Justifications recap (locked in v0 planning conversation):

1. Single source of truth — one parser/evaluator means no PG-vs-SQLite skew.
2. Policies can reference `reactor-auth` semantics directly (`auth.has_permission(...)`).
3. Same policy text works on any future backend (SQLite, DuckDB, MySQL, …).
4. `auth.has_permission` short-circuits in Rust against `AuthCtx.permissions` — no DB roundtrip and the predicate is folded to `TRUE`/`FALSE` at plan time.
5. No `SET LOCAL request.jwt.claim.*` ceremony; no `CREATE POLICY` DDL in user migrations.

The trade-off — losing native PG RLS as a defence-in-depth layer — is mitigated by the single-app-role connection model in §6.2: the only path to the user's data is through reactor-data, so policy-bypass requires DB-credential theft, not an API misuse.

### 8.2 DSL grammar

Inline in migration files. The grammar is parsed as an extension to standard SQL by `policy::grammar` (recursive descent over the sqlparser token stream once the `policy` keyword is recognised).

```ebnf
policy_stmt    := "policy" ident "on" table_ref "for" scope_list
                  ( "using" "(" expr ")" )?
                  ( "check" "(" expr ")" )?
                  ";"
scope_list     := scope ( "," scope )*
scope          := "select" | "insert" | "update" | "delete"
expr           := boolean expression over:
                    - column refs (bare or `table.col`)
                    - SQL literals
                    - auth.user_id() | auth.org_id() | auth.role()
                    - auth.has_permission(string_literal)
                    - auth.in_org(reactor_id_literal_or_expr)
                    - comparison & logical ops
                    - IN (...), IS NULL/NOT NULL
                    - safe subqueries: SELECT col FROM same_table WHERE ...
```

Concrete example:

```sql
create table todos (
  id          reactor_id primary key,
  org_id      reactor_id not null,
  title       text not null,
  done        bool not null default false,
  created_at  timestamptz not null default now()
);

policy todos_tenant on todos
  for select, update, delete
  using (org_id = auth.org_id());

policy todos_insert on todos
  for insert
  check (org_id = auth.org_id() and auth.has_permission('data:todos:write'));

policy todos_admin_delete on todos
  for delete
  using (auth.has_permission('data:todos:delete'));
```

A table with **no** policies is **deny-by-default for non-`*` callers**. A caller holding the `*` (god) permission bypasses policy evaluation entirely (audit-logged). Owners of an org hold `*` by default per `reactor-auth` §8.2.

### 8.3 Compilation

```rust
// reactor-data/src/policy/compile.rs
pub enum PolicyDecision {
    AlwaysAllow,                 // every relevant policy folded to TRUE
    AlwaysDeny,                  // any required policy folded to FALSE
    Conditional(SqlFragment),    // AND-joined predicate to append to WHERE
}

pub fn compile_for_scope(
    table: &TableMeta,
    scope: PolicyScope,          // Select | Insert | Update | Delete
    ctx:   &DataCtx,
) -> PolicyDecision;
```

Algorithm:
1. Load all policies on `table` whose `scopes` include `scope`.
2. For each policy expression, evaluate `auth.*` calls eagerly against `ctx.auth`:
   - `auth.user_id()` → bound parameter `$P_user_id`
   - `auth.org_id()` → bound parameter `$P_org_id` (or constant fold if static)
   - `auth.role()` → bound parameter `$P_role`
   - `auth.has_permission('x')` → folded to `TRUE` if `ctx.has_permission("x")`, else `FALSE`
   - `auth.in_org(expr)` → `expr = $P_org_id`
3. Constant-fold the resulting boolean tree.
4. Combine policies: SELECT/UPDATE/DELETE require **any `using` policy to permit** (logical OR); INSERT/UPDATE additionally require **all `check` policies to permit** (logical AND).
5. Emit:
   - All folded to `TRUE` → `AlwaysAllow`
   - All folded to `FALSE` → `AlwaysDeny` (HTTP 403 immediately, no SQL issued)
   - Mixed → `Conditional(<remaining predicate as SQL>)`

`Conditional` predicates are appended to the user's `WHERE` clause for SELECT/UPDATE/DELETE. INSERT bodies are checked by `policy::eval` against the proposed row before SQL is emitted.

### 8.4 Audit & errors

When a policy denies a mutation:
- Single-row → `403 policy_denied` with the policy name & scope.
- Bulk → write succeeds for permitted rows, returns `207 multi_status` with a per-row outcome array. (PostgREST returns `403` for the whole batch; we diverge here because the agent UX is better. Flagged in §17 if we want to revisit.)

Audit row written in the same transaction as the mutation, with `event_type` and `details.policy_decisions: [{ row, decision, policy }]`.

---

## 9. Query translator

### 9.1 Path

```
HTTP request
    │
    ▼ middleware::auth  (Authorization, X-Reactor-Org) → DataCtx (via AuthClient)
    │
    ▼ routes/crud       (route → table + verb)
    │
    ▼ query::*          (parse querystring → QueryPlan)
    │   - filter::parse
    │   - select::parse + embedded resolution against SchemaSnapshot
    │   - order::parse
    │   - pagination::parse
    │   - prefer::parse
    │
    ▼ policy::compile_for_scope(table, verb, ctx)
    │   - AlwaysDeny → 403 immediately
    │   - AlwaysAllow → no predicate added
    │   - Conditional → predicate appended to plan.where_clause
    │
    ▼ execute::plan      (QueryPlan → sqlparser-rs AST → parameterised SQL string)
    │
    ▼ DataStore::Tx      (parameterised execute_raw / fetch_rows)
    │
    ▼ execute::format    (rows → JSON, Range header, audit row in same txn, commit)
```

The planner never concatenates user-provided strings into SQL. Filter values, embed args, payload values — all become bound parameters. Column names are validated against `SchemaSnapshot` before they reach SQL generation.

### 9.2 `QueryPlan`

```rust
pub struct QueryPlan {
    pub table:       TableRef,
    pub verb:        Verb,                  // Select | Insert | Update | Delete
    pub projection:  Projection,            // columns + embedded resources
    pub filters:     FilterTree,            // and/or/not over leaf ops
    pub order_by:    Vec<OrderItem>,
    pub pagination:  Pagination,
    pub returning:   Returning,             // from Prefer: return=...
    pub count:       CountMode,             // none | exact | planned | estimated
    pub payload:     Option<Payload>,       // for insert/update
    pub upsert:      Option<UpsertMode>,    // from Prefer: resolution=...
    pub policy_pred: Option<SqlFragment>,   // attached by policy::compile
}
```

### 9.3 Embedded resource resolution

For `?select=id,title,author(id,name)` on `posts`:
1. Parse projection into a tree.
2. For each named non-column ident (`author`), look up `posts.foreign_keys` in `SchemaSnapshot`.
3. Match `author` → FK whose column name (or `referenced_table` if unique) matches; reject ambiguity with `400 ambiguous_embed` and hint the disambiguator (`author:users!posts_author_id_fkey`).
4. Recurse into the joined table (apply its policies, repeat).
5. Emit as `LEFT JOIN LATERAL (SELECT jsonb_build_object(...) FROM users WHERE ... AND <policy>)`.

---

## 10. Auth integration

reactor-data is the first real consumer of `reactor_core::auth::AuthClient`. Wiring:

```rust
// reactor-data-server/src/main.rs
let auth: Arc<dyn AuthClient> = match config.deployment {
    Deployment::Monolith => {
        let auth_service = reactor_auth::AuthService::new(auth_pool, auth_config).await?;
        Arc::new(reactor_auth::client::InProcessAuthClient::new(Arc::new(auth_service)))
    }
    Deployment::Microservices => Arc::new(
        reactor_auth::client::RemoteAuthClient::builder()
            .base_url(config.auth_url.clone())
            .internal_secret(config.internal_secret.clone())
            .build()?,
    ),
};

let state = DataState::new(store, auth.clone(), config.clone());
let app = reactor_data::router(state);
```

Note: while reactor-data depends only on `reactor-core::auth::AuthClient`, the **server binary** `reactor-data-server` does take `reactor-auth` as a dependency to be able to construct an `InProcessAuthClient`. Library callers (e.g. a unified Reactor monolith) construct the client themselves and inject it.

### 10.1 Middleware

```rust
async fn auth_middleware<B>(
    State(state): State<DataState>,
    headers: HeaderMap,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, DataError> {
    let token = headers.get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(DataError::Unauthorized)?;

    // OrgRef accepts both UUID and slug; resolution happens in reactor-auth
    let requested_org: Option<OrgRef> = headers.get("x-reactor-org")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.parse().unwrap());  // OrgRef::from_str: UUID parse first, else treat as slug

    let ctx = state.auth.resolve_ctx(token, requested_org.as_ref()).await?;
    req.extensions_mut().insert(DataCtx::from_auth(ctx, request_id));
    Ok(next.run(req).await)
}
```

**Note on slug-vs-uuid:** `X-Reactor-Org` accepts both UUID and slug per the auth design. The `OrgRef` enum wraps either form; resolution happens inside `AuthClient::resolve_ctx`. reactor-data passes the header value through unchanged. For the remote client, the `OrgRef` travels in the JSON body of `/_internal/resolve_ctx` and reactor-auth resolves slugs server-side.

### 10.2 What `auth.*` policy helpers compile to

| DSL call | Compiles to (Postgres) | Source |
|---|---|---|
| `auth.user_id()` | bound param `$N` | `ctx.auth.user_id` |
| `auth.org_id()` | bound param `$N` | `ctx.auth.active_org` (deny if `None`) |
| `auth.role()` | bound param `$N` | name of role assigned in active org |
| `auth.has_permission('x')` | folded `TRUE`/`FALSE` | `ctx.has_permission("x")` |
| `auth.in_org(<expr>)` | `<expr> = $N` | `ctx.auth.active_org` |

`auth.role()` requires a DB call into reactor-auth in the `RemoteAuthClient` case because role name isn't in the JWT — but `resolve_ctx` already loads role metadata server-side and returns it on `AuthCtx`, so the lookup is local.

### 10.3 Auth model: Reactor-native only

Per the locked decision: reactor-data does **not** support Supabase-style `anon` / `service_role` keys. Every request bears a JWT issued by reactor-auth. Three flavours, all transparent to reactor-data because they all flow through `verify_token`:

1. **User token** (`sub = user_<id>`, `amr ∈ {[pwd], [pwd, totp], [oauth:google], ...}`)
2. **API-key token** (`sub = apikey:<id>`, `amr = [apikey]`) — opaque API keys at the auth layer, JWT envelope at the wire layer per `reactor-auth` design §8.3 + §17.
3. **Service account token** (future) — same shape, `sub = svc:<name>`. Not in v0; called out so the design accommodates it.

There is no anonymous endpoint. `data:public:read` permission on the public role (or whatever role anonymous users would map to in Supabase) is replaced by issuing a short-lived token to anonymous visitors via a dedicated reactor-auth flow if/when a customer needs it. Out of v0 scope.

---

## 11. Configuration

`reactor-data-server` reads from env (12-factor) and optionally `reactor.toml [data]`. Env wins.

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_DATA_DATABASE_URL` | yes | — | Postgres connection string (single app role) |
| `REACTOR_DATA_BIND` | no | `0.0.0.0:8002` | HTTP bind address |
| `REACTOR_DATA_MIGRATIONS_DIR` | no | `./migrations` | User migration source directory |
| `REACTOR_DATA_USER_SCHEMA` | no | `public` | Schema name for user tables |
| `REACTOR_DATA_MAX_EMBED_DEPTH` | no | `5` | Max FK-embed nesting in `?select=` |
| `REACTOR_DATA_MAX_LIMIT` | no | `1000` | Caps `?limit=` / `Range` for read endpoints |
| `REACTOR_DATA_DEFAULT_LIMIT` | no | `100` | Used when no `?limit` / `Range` is provided |
| `REACTOR_DATA_DEPLOYMENT` | no | `monolith` | `monolith` (in-process auth) or `microservices` |
| `REACTOR_DATA_AUTH_URL` | yes (microservices) | — | URL of reactor-auth-server |
| `REACTOR_DATA_INTERNAL_SECRET` | yes (microservices) | — | Shared with reactor-auth's `_internal` endpoints |
| `REACTOR_DATA_AUTH_DATABASE_URL` | yes (monolith) | — | Postgres URL for the auth schema (may equal `REACTOR_DATA_DATABASE_URL`) |
| `REACTOR_DATA_AUTH_DATA_KEY` | yes (monolith) | — | Forwarded to reactor-auth column-encryption key |
| `REACTOR_LOG` | no | `info` | tracing filter |

Boot fails fast on missing required vars (selected by `_DEPLOYMENT` mode). `doctor` subcommand prints diagnostics.

---

## 12. Tracing, metrics, audit

- **Tracing**: same shape as reactor-auth — `tracing` + JSON subscriber; every request has a `request_id` span field; `X-Request-Id` header echoed; spans include `table`, `verb`, `row_count`, and (where applicable) `policy_decision`.
- **Metrics**: Prometheus `/metrics` (basic — request counters, latency histograms, planner timing, db pool stats). Off by default; gated by `REACTOR_DATA_METRICS=1`.
- **Audit**: every mutation (`POST/PATCH/DELETE/POST /rpc`) writes a row to `_reactor_data.audit_events` *in the same transaction* as the data change. Read endpoints are not audited (volume reasons). RPC invocations are audited regardless of read/write nature.

---

## 13. Test surface

- **Unit**: dialect parser & lint pass; policy DSL parser; policy compiler (table of `(policy, ctx) → decision`); filter parser; query planner SQL emission against golden snapshots.
- **Integration**: `testcontainers` boots Postgres, runs reactor-data + reactor-auth migrations, executes the full request → policy → SQL → response pipeline.
- **Conformance with reactor-auth**: a `tests/auth_integration.rs` harness that boots `reactor-auth-server` and `reactor-data-server` as separate Tokio tasks against shared Postgres, signs up a user, creates an org, creates a custom role with `data:todos:read`, issues a token, runs `GET /data/v1/todos` against a seeded table, and asserts the policy fires as expected — once with `InProcessAuthClient` (single binary) and once with `RemoteAuthClient` (split binaries). **This satisfies the §1.4 goal: reactor-data is the integration test for reactor-auth.**
- **Property**: `proptest` on the PostgREST filter parser and on the policy boolean folder (random `AuthCtx` × random policy expr → check decision invariants).
- **Supabase parity sanity**: a small fixture of curl commands sourced from Supabase's docs, run against reactor-data, asserting response shape & headers match. Not exhaustive — just enough to prevent inadvertent divergence.

---

## 14. Cargo workspace additions

Append to root `Cargo.toml` (`[workspace.dependencies]`):

```toml
sqlparser   = { version = "0.51", features = ["serde"] }
nom         = "7"                                # for PostgREST filter querystring grammar
indexmap    = { version = "2", features = ["serde"] }
itertools   = "0.13"
futures     = "0.3"
```

New workspace members:

```toml
[workspace]
members = [
  "crates/reactor-core",
  "crates/reactor-auth",
  "crates/reactor-auth-server",
  "crates/reactor-data",
  "crates/reactor-data-server",
]
```

`reactor-data` crate `Cargo.toml` deps (from workspace): `reactor-core`, `tokio`, `axum`, `tower`, `tower-http`, `serde`, `serde_json`, `sqlx` (postgres+uuid+chrono+json), `uuid`, `chrono`, `thiserror`, `tracing`, `sqlparser`, `nom`, `indexmap`, `async-trait`, `validator`.

`reactor-data-server` crate adds: `tracing-subscriber`, `reactor-auth`, `reactor-data`, `reqwest` (for the `RemoteAuthClient` path).

---

## 15. Build order (v0 slice)

Each task is a self-contained PR. Doc lands first; checkpoints below.

| # | Task | Outcome |
|---|---|---|
| 0 | Land this design doc (`docs/reactor-data.design.md`) | reviewed contract |
| 1 | Workspace skeleton: add `reactor-data` + `reactor-data-server` to root `Cargo.toml`; empty crates compile | `cargo check --workspace` clean |
| 2 | `reactor-data` skeleton: `DataConfig`, `DataState` (holding `Arc<dyn AuthClient>`), `Router::new(state)`, `/data/v1/health` | binary boots; health returns 200 |
| 3 | `_reactor_data` metadata migrations + `DataStore` trait + `PgDataStore` scaffold + `testcontainers` smoke test | metadata schema applies clean against fresh Postgres |
| 4 | Portable SQL dialect: AST + sqlparser wrapper + lint pass (forbidden constructs) + `reactor_id` type mapping + PG emitter | unit tests on representative DDL pass; forbidden constructs rejected with clear errors |
| 5 | `MigrationRunner`: discover/parse/apply user migrations, populate `_reactor_data.tables`, detect drift | integration test runs a multi-file fixture against PG |
| 6 | Policy DSL: grammar + AST + storage in `_reactor_data.policies` (parser + persistence only; no enforcement yet) | migrations containing `policy ... on ... using (...)` parse and persist; round-trip via `ast_json` |
| 7 | Auth middleware: bearer extraction + `AuthClient::resolve_ctx` + `DataCtx` request extension | dummy authenticated route returns 200 with user/org echo |
| 8 | PostgREST querystring parser: `filter`, `select` (flat only — no embeds yet), `order`, `limit`/`offset`, `Prefer` | unit tests on a corpus of querystring → `QueryPlan` cases |
| 9 | CRUD execution path (no policies yet): `GET/POST/PATCH/DELETE /data/v1/{table}` against a seeded table; `Prefer: return=*` and `count=exact` honored; `Content-Range` set | end-to-end CRUD works without auth/policy enforcement (test harness uses a permissive ctx) |
| 10 | Policy enforcement: integrate `policy::compile_for_scope` into the planner; SELECT/UPDATE/DELETE get conditional WHERE predicates; INSERT/UPDATE pre-evaluated against payload; `AlwaysDeny` short-circuits; `*` permission bypass | scenario test: same query returns different rows for different users with different roles |
| 11 | Embedded resources (`?select=col,embed(col,nested(col))`) — FK traversal against `SchemaSnapshot`, JOIN-emit, per-table policy application, depth limit | snapshot tests of generated SQL + integration test with multi-table seed |
| 12 | RPC: `POST /data/v1/rpc/{fn}` for SQL-defined functions; `_reactor_data.rpc_functions` populated from migrations; permission `data:rpc:{name}:invoke` enforced | integration test calling a defined function with named JSON args |
| 13 | Mutation audit (`_reactor_data.audit_events` writes in same txn) + `tower_http::trace::TraceLayer` + JSON tracing + `X-Request-Id` + graceful shutdown | mutations produce audit rows; tracing JSON has request_id |
| 14 | `reactor-data-server` polish: `doctor` (DB connectivity, migration status, auth client reachability), README quickstart, end-to-end reactor-auth integration harness (boots both servers in both topology modes) | exit checklist below passes |

### v0 exit checklist

- [ ] `reactor-data-server` boots against an empty Postgres → `_reactor_data` migrations apply, doctor reports green.
- [ ] `reactor-data-server` with `REACTOR_DATA_MIGRATIONS_DIR` applies a sample project's migrations and populates `_reactor_data.tables` + `_reactor_data.policies`.
- [ ] curl flows against a running reactor-auth + reactor-data:
  - sign up → create org → `POST /data/v1/todos` succeeds for owner
  - second user joined as `member` → `POST /data/v1/todos` denied with `policy_denied`
  - `GET /data/v1/todos` for member returns only that member's rows (policy applied)
  - `GET /data/v1/posts?select=id,title,author(name)` works with embedded resource & per-table policy
  - `POST /data/v1/rpc/my_func { "x": 5 }` returns expected JSON
- [ ] Same harness passes with reactor-auth+reactor-data co-deployed in a single binary (`InProcessAuthClient`) AND split into two binaries (`RemoteAuthClient`).
- [ ] `_reactor_data.audit_events` populated for every mutation in the harness.

### Parallel-safe pairings (if multiple agents/days)

- PR 4 (dialect) and PR 7 (auth middleware) are independent after PR 3.
- PR 8 (querystring parser) and PR 6 (policy DSL parser) are independent — they only meet at PR 10.
- PR 11 (embeds) and PR 12 (RPC) are independent after PR 10.

---

## 16. Decision log

Decisions locked during v0 planning (May 2026):

| Question | Decision | Rationale |
|---|---|---|
| **Local-first DB choice** | SQLite (no libsql) for v0.2's Tauri path | libsql team is focused on Turso (beta); plain SQLite is the simpler, more durable bet. |
| **Policy enforcement** | Rust-owned engine only; no native Postgres RLS | Single source of truth, identical semantics across backends, native composition with `reactor-auth` permissions, free predicate-folding for `auth.has_permission`. Defence-in-depth lost is mitigated by single-app-role connection model. |
| **PostgREST scope** | Full PostgREST in v0 (filters, embedded resources, RPC, Prefer headers) | Supabase parity is the v0 success criterion; embedded resources are the headline feature; skipping them undercuts the parity claim. |
| **RPC definition language** | SQL-defined only in v0 (`CREATE FUNCTION ... LANGUAGE sql`) | Keeps the safety story small; Rust-defined RPCs require dynamic loading or compile-time registration, both v0.2+. |
| **`reactor_id` type** | Alias for `ReactorId` (UUIDv7) from `reactor-core` | Workspace consistency; time-sortable IDs everywhere. |
| **Realtime in v0** | Deferred to v0.2; route reserved (`?subscribe=1` → 426) | Adapter-heavy and orthogonal to core CRUD parity; SSE chosen as the v0.2 transport. |
| **Type generation** | Deferred to v0.2 | Schema metadata is captured in v0; codegen is mechanical and downstream. |
| **Dialect strategy** | Compiler-from-day-one; Postgres adapter ships in v0; SQLite adapter v0.2 | Avoids painful retrofit; migrations are written portably from day one and lint enforces it. |
| **SQL parser** | `sqlparser-rs` with a custom Reactor lint pass + `policy` extension | Mature, multi-dialect, AST is the right abstraction for both migration linting and query planning. |
| **Policy file location** | Inline in `*.sql` migration files (`policy ... on ... using (...)`) | One source of truth per change; agents reason about one file per migration. |
| **`X-Reactor-Org` semantics** | reactor-data forwards the header to `AuthClient::resolve_ctx`; trusts `ctx.active_org` | Single resolver, no duplicated slug-vs-uuid logic. |
| **Auth model** | Reactor JWTs only (user + API-key + future service); no Supabase `anon`/`service_role` | Consistent with reactor-auth's chosen direction; Supabase itself is phasing those out. |
| **v0 success criteria** | (a) reactor-data fully exercises reactor-auth end-to-end (integration harness), and (b) G2 parity with Supabase Data on a single Postgres server | Locks scope: ship CRUD + filters + embeds + RPC + policy enforcement + audit; defer realtime, types, SQLite to v0.2. |
| **`AuthClient::resolve_ctx` widening** | Accept `Option<&OrgRef>` where `OrgRef = Id(OrgId) \| Slug(String)` | Slug resolution stays inside reactor-auth; reactor-data only carries the header value through. No extra pre-resolution call needed. |
| **Bulk-mutation policy denial** | Return `207 Multi-Status` with per-row outcome array | Better agent UX than PostgREST's `403` for entire batch. Permitted rows commit; denied rows are reported individually. |
| **`_reactor_data` schema isolation** | Metadata schema is **not** readable by the user app role | Strict separation; reactor-data owns the metadata schema exclusively. No grants to the application connection role. |
| **JSON-path filter operators** | Deferred to v0.2 alongside SQLite adapter | Both require `json_extract` emission for portability. Not blocking for v0 Supabase parity. |
| **Realtime transport** | TBD; decision locked at start of v0.2 | SSE is simpler; WS matches Supabase JS clients. Either works with the same broadcast core. |

---

## 17. Open questions (deferred)

1. **Schema-per-org for multi-tenancy.** Currently single user schema (`public`) and tenant separation lives entirely in policies. A future per-org schema mode would give native isolation at the cost of cross-tenant operations. Not needed for v0.

---

*End of design doc. Land code against checklist §15 in order, one PR per row, this doc updated as decisions change.*
