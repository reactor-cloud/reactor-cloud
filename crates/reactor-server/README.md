# reactor-server

Unified Reactor.cloud server for G1/G2 topologies.

## Overview

`reactor-server` is a single binary that mounts all capability routers (auth, data, storage, functions, jobs, sites) in one process against a shared PostgreSQL connection pool. It's designed for:

- **G1 (Tauri)**: Desktop embedding via `reactor_server::run(cfg)`
- **G2 (Single VPS)**: One process per host with all capabilities

For production G3 topologies, use the per-capability `reactor-{cap}-server` binaries instead.

## Quick Start

1. **Create a Postgres database**:

```bash
createdb reactor
```

2. **Create a `Reactor.toml` configuration**:

```toml
[database]
url = "postgres://localhost/reactor"

[admin]
token = "your-secure-admin-token"

[auth]
data_key = "base64-32-byte-key-here"
public_url = "http://localhost:8000"

[data]
user_schema = "public"

[storage]
backend = "fs"
fs_base_path = "./.reactor/blobs"
signing_secret = "your-signing-secret"

[functions]
workdir = "./.reactor/functions"
data_key = "base64-32-byte-key-here"

[jobs]
webhook_secret = "your-webhook-secret"
```

3. **Run migrations**:

```bash
reactor-server migrate
```

4. **Start the server**:

```bash
reactor-server
```

5. **Verify it's running**:

```bash
# Health check
curl http://localhost:8000/health

# Version info (requires auth)
curl -H "Authorization: Bearer your-secure-admin-token" \
  http://localhost:8000/_admin/version

# Doctor probes
curl -H "Authorization: Bearer your-secure-admin-token" \
  http://localhost:8000/_admin/doctor
```

## Deploy a Bundle

```bash
# Create and deploy a bundle
curl -X POST \
  -H "Authorization: Bearer your-secure-admin-token" \
  -F "bundle=@deploy.tar.zst" \
  http://localhost:8000/_admin/deploy
```

## Configuration

Configuration is loaded from (in order of precedence):
1. Environment variables (`REACTOR_*`, use `__` for nesting)
2. `Reactor.toml` file

See `examples/Reactor.toml` for a complete configuration example.

## Features

The crate supports feature flags for different topologies:

- `g1-tauri` — Minimal set for Tauri desktop embedding
- `g2-full` — Full capability set (default)

Individual capabilities can be toggled:
- `cap-auth`, `cap-data`, `cap-storage`, `cap-functions`, `cap-jobs`, `cap-sites`

## Embedding

For Tauri or other embedding scenarios:

```rust
use reactor_server::{run, ReactorConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ReactorConfig::load()?;
    run(config).await
}
```

## API Endpoints

### Public (no auth)
- `GET /health` — Composite health check
- `GET /metrics` — Prometheus metrics

### Admin (requires Bearer token)
- `GET /_admin/version` — Version info
- `POST /_admin/migrate` — Run all migrations
- `GET /_admin/doctor` — Health probes
- `POST /_admin/deploy` — Deploy a bundle
- `POST /_admin/shutdown` — Trigger graceful shutdown

### Capability Routes
Each enabled capability mounts its routes:
- `/auth/v1/*` — Authentication endpoints
- `/data/v1/*` — Data API (PostgREST-shaped)
- `/storage/v1/*` — Object storage
- `/fn/v1/*` — Function invocation
- `/jobs/v1/*` — Job management
- `/sites/*` — Static site serving

## License

Apache-2.0 OR MIT
