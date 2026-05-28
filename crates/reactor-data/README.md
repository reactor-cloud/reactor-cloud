# reactor-data

PostgREST-shaped HTTP API with a Rust-owned policy engine for Reactor.cloud.

## Features

- **Full PostgREST Surface**: Filters, embedded resources, pagination, Prefer headers, RPC
- **Portable SQL Dialect**: Compiler-enforced subset that compiles to Postgres (SQLite in v0.2)
- **Rust Policy Engine**: Row-level security evaluated in Rust, not native Postgres RLS
- **Auth Integration**: Works with reactor-auth via InProcessAuthClient or RemoteAuthClient
- **Audit Trail**: All mutations logged to `_reactor_data.audit_events`

## Quick Start

### 1. Start Postgres

```bash
docker run -d --name reactor-db \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  postgres:16-alpine
```

### 2. Set Environment Variables

```bash
# Required
export REACTOR_DATA_DATABASE_URL="postgres://postgres:postgres@localhost/postgres"

# For monolith mode (auth embedded)
export REACTOR_DATA_AUTH_DATABASE_URL="$REACTOR_DATA_DATABASE_URL"
export REACTOR_DATA_AUTH_DATA_KEY="your-32-byte-encryption-key-here"

# Optional
export REACTOR_DATA_BIND="0.0.0.0:8002"
export REACTOR_DATA_USER_SCHEMA="public"
export REACTOR_DATA_MIGRATIONS_DIR="./migrations"
```

### 3. Create Migrations

```sql
-- migrations/001_todos.sql
CREATE TABLE todos (
    id reactor_id PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL,
    title TEXT NOT NULL,
    done BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Allow org members to read their own org's todos
policy todos_read on todos for select
using (auth.org_id() = org_id);

-- Allow org admins to insert
policy todos_insert on todos for insert
check (auth.has_permission('data:todos:write') AND auth.org_id() = org_id);
```

### 4. Start the Server

```bash
cargo run -p reactor-data-server
```

### 5. Make Requests

```bash
# Create a todo (with auth token)
curl -X POST http://localhost:8002/data/v1/todos \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Learn Reactor", "org_id": "your-org-id"}'

# List todos with filters
curl "http://localhost:8002/data/v1/todos?done=eq.false&order=created_at.desc" \
  -H "Authorization: Bearer $TOKEN"

# Update a todo
curl -X PATCH "http://localhost:8002/data/v1/todos?id=eq.$TODO_ID" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"done": true}'

# Embedded resources
curl "http://localhost:8002/data/v1/posts?select=id,title,author(name),comments(body)" \
  -H "Authorization: Bearer $TOKEN"
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `REACTOR_DATA_DATABASE_URL` | (required) | Postgres connection URL |
| `REACTOR_DATA_BIND` | `0.0.0.0:8002` | HTTP bind address |
| `REACTOR_DATA_USER_SCHEMA` | `public` | Schema for user tables |
| `REACTOR_DATA_MIGRATIONS_DIR` | (none) | Path to migrations directory |
| `REACTOR_DATA_RUN_MIGRATIONS` | `true` | Apply migrations on startup |
| `REACTOR_DATA_MAX_EMBED_DEPTH` | `5` | Max nested embed depth |
| `REACTOR_DATA_MAX_LIMIT` | `1000` | Max rows per request |
| `REACTOR_DATA_DEPLOYMENT` | `monolith` | `monolith` or `microservices` |
| `REACTOR_DATA_METRICS` | `false` | Enable /metrics endpoint |

### Monolith Mode (default)

Auth service embedded in the same process:

```bash
REACTOR_DATA_AUTH_DATABASE_URL="postgres://..."
REACTOR_DATA_AUTH_DATA_KEY="your-encryption-key"
```

### Microservices Mode

Auth as a separate service:

```bash
REACTOR_DATA_DEPLOYMENT="microservices"
REACTOR_DATA_AUTH_URL="http://localhost:8001"
```

## CLI Commands

```bash
# Start the server
reactor-data-server

# Run health checks
reactor-data-server doctor

# Show help
reactor-data-server --help
```

## API Reference

### CRUD Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/data/v1/{table}` | Select rows |
| POST | `/data/v1/{table}` | Insert rows |
| PATCH | `/data/v1/{table}` | Update rows |
| DELETE | `/data/v1/{table}` | Delete rows |

### Query Parameters

- `select`: Column selection and embeds — `?select=id,title,author(name)`
- `order`: Sort order — `?order=created_at.desc.nullslast`
- `limit`, `offset`: Pagination — `?limit=10&offset=20`
- Filters: `?column=op.value` — `?status=eq.active&age=gte.18`

### Filter Operators

| Operator | Description | Example |
|----------|-------------|---------|
| eq | Equals | `?id=eq.5` |
| neq | Not equals | `?status=neq.deleted` |
| gt, gte | Greater than | `?age=gte.18` |
| lt, lte | Less than | `?price=lt.100` |
| like, ilike | Pattern match | `?name=like.*Smith*` |
| in | In list | `?id=in.(1,2,3)` |
| is | Is null/true/false | `?deleted_at=is.null` |
| cs | Contains (arrays) | `?tags=cs.{a,b}` |
| cd | Contained by | `?tags=cd.{a,b,c}` |
| ov | Overlaps | `?tags=ov.{x,y}` |

### Prefer Headers

```http
Prefer: return=representation    # Return inserted/updated rows
Prefer: return=minimal          # Return only status
Prefer: count=exact             # Include total count
Prefer: resolution=merge-duplicates  # Upsert mode
```

### RPC

```bash
# Call a SQL function
curl -X POST http://localhost:8002/data/v1/rpc/my_function \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"arg1": "value", "arg2": 42}'
```

## Policy DSL

Policies are defined inline in migration files:

```sql
-- Row-level read policy
policy policy_name on table_name for select
using (condition);

-- Row-level write policy with check
policy policy_name on table_name for insert, update
check (condition);

-- Combined
policy policy_name on table_name for all
using (read_condition)
check (write_condition);
```

### Auth Builtins

| Function | Description |
|----------|-------------|
| `auth.user_id()` | Current user's ID |
| `auth.org_id()` | Current org ID |
| `auth.role()` | User's role in org |
| `auth.has_permission('perm')` | Check permission |
| `auth.in_org(org_id)` | Check org membership |

## Architecture

```
┌─────────────────┐
│ HTTP Request    │
└────────┬────────┘
         │
    ┌────▼────┐
    │ Router  │
    └────┬────┘
         │
┌────────▼────────┐
│ Auth Middleware │──► AuthClient (InProcess/Remote)
└────────┬────────┘
         │
    ┌────▼────┐      ┌──────────────┐
    │ Policy  │◄─────│ PolicyStore  │
    │ Engine  │      └──────────────┘
    └────┬────┘
         │
┌────────▼────────┐
│ SQL Execution   │
└────────┬────────┘
         │
┌────────▼────────┐
│ Postgres/SQLite │
└─────────────────┘
```

## Development

```bash
# Run tests
cargo test -p reactor-data

# Check lints
cargo clippy -p reactor-data

# Run with debug logging
RUST_LOG=debug cargo run -p reactor-data-server
```

## License

See repository root for license information.
