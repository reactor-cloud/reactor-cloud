# reactor-functions

Sandboxed HTTP handlers with pluggable runtime adapters (wasm, bun, lambda).

## Overview

`reactor-functions` implements the Functions capability for Reactor.cloud:

- **Runtime Adapters**: WebAssembly (wasmtime), Bun (subprocess), AWS Lambda
- **Bundle Storage**: Versioned bundles stored in `reactor-storage`
- **Policy Enforcement**: Invoke-time policies via `reactor-policy`
- **Streaming**: Full streaming support for request/response bodies
- **Observability**: Unified audit, invocations, and metrics

## Quickstart

### Prerequisites

- PostgreSQL database
- Running `reactor-auth-server` (or in-process auth)
- Running `reactor-storage-server`
- For Bun runtime: `bun` installed
- For Lambda runtime: AWS credentials configured

### Configuration

Set environment variables:

```bash
# Required
export REACTOR_FUNCTIONS_DATABASE_URL=postgres://...
export REACTOR_FUNCTIONS_STORAGE_URL=http://localhost:8082
export REACTOR_FUNCTIONS_STORAGE_API_KEY=your-storage-key
export REACTOR_FUNCTIONS_DATA_KEY=your-32-byte-hex-key

# Monolith mode (in-process auth)
export REACTOR_FUNCTIONS_DEPLOYMENT=monolith
export REACTOR_FUNCTIONS_AUTH_DATABASE_URL=postgres://...
export REACTOR_FUNCTIONS_AUTH_DATA_KEY=your-auth-data-key

# OR Microservices mode (remote auth)
export REACTOR_FUNCTIONS_DEPLOYMENT=microservices
export REACTOR_FUNCTIONS_AUTH_URL=http://localhost:8080

# Optional: Bun runtime config
export REACTOR_FUNCTIONS_BUN_BIN=bun
export REACTOR_FUNCTIONS_BUN_IDLE_TTL_SECS=300
export REACTOR_FUNCTIONS_BUN_MAX_INSTANCES_PER_FN=8

# Optional: Lambda runtime config
export REACTOR_FUNCTIONS_LAMBDA_REGION=us-east-1
export REACTOR_FUNCTIONS_LAMBDA_ROLE_ARN=arn:aws:iam::...
export REACTOR_FUNCTIONS_LAMBDA_BUNDLE_S3_BUCKET=my-bucket
export REACTOR_FUNCTIONS_LAMBDA_LWA_LAYER_ARN=arn:aws:lambda:...
```

### Run Server

```bash
# Check configuration
cargo run -p reactor-functions-server -- doctor

# Run the server
cargo run -p reactor-functions-server
```

## API

### Health
```
GET /fn/v1/health
```

### Function CRUD
```
POST   /fn/v1/_admin/functions          # Create function
GET    /fn/v1/_admin/functions          # List functions
GET    /fn/v1/_admin/functions/{name}   # Get function
DELETE /fn/v1/_admin/functions/{name}   # Delete function
```

### Deployments
```
POST   /fn/v1/_admin/functions/{name}/deployments              # Create deployment
GET    /fn/v1/_admin/functions/{name}/deployments              # List deployments
GET    /fn/v1/_admin/functions/{name}/deployments/{id}         # Get deployment
POST   /fn/v1/_admin/functions/{name}/promote                  # Promote deployment
POST   /fn/v1/_admin/functions/{name}/rollback                 # Rollback deployment
```

### Environment Variables
```
PUT    /fn/v1/_admin/functions/{name}/env/{key}    # Set env var
GET    /fn/v1/_admin/functions/{name}/env          # List env vars
GET    /fn/v1/_admin/functions/{name}/env/{key}    # Get env var
DELETE /fn/v1/_admin/functions/{name}/env/{key}    # Delete env var
```

### Policies
```
POST   /fn/v1/_admin/functions/{name}/policies              # Create policy
GET    /fn/v1/_admin/functions/{name}/policies              # List policies
DELETE /fn/v1/_admin/functions/{name}/policies/{policy}     # Delete policy
```

### Logs
```
GET /fn/v1/_admin/functions/{name}/logs?follow=1    # Stream logs (SSE)
```

### Invoke
```
* /fn/v1/{name}          # Invoke function
* /fn/v1/{name}/{*path}  # Invoke function with sub-path
```

### Metrics (optional)
```
GET /fn/v1/metrics    # Prometheus metrics (when REACTOR_FUNCTIONS_METRICS=1)
```

## Permissions

| Permission | Description |
|------------|-------------|
| `functions:create` | Create new functions |
| `functions:{name}:admin` | Full admin access to function |
| `functions:{name}:deploy` | Deploy new versions |
| `functions:{name}:invoke` | Invoke the function |
| `functions:{name}:logs` | Stream function logs |
| `functions:*:admin` | Admin access to all functions |
| `functions:*:invoke` | Invoke any function |

## Function Contract

Functions must implement the Web Standard `Request → Response` interface:

### Bun (TypeScript/JavaScript)
```typescript
export default {
  async fetch(request: Request): Promise<Response> {
    return new Response("Hello, World!");
  }
};
```

### WASM (Rust)
```rust
use wasi::{http::{incoming_handler, types::*}};

struct Component;

impl incoming_handler::Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        // Handle request, write response
    }
}
```

## Bundle Format

Bundles are ZIP files with a `manifest.json`:

```json
{
  "name": "my-function",
  "version": "1.0.0",
  "runtime": "bun",
  "entrypoint": "index.ts",
  "timeout_ms": 30000,
  "memory_mb": 256,
  "max_concurrency": 50,
  "min_instances": 0
}
```

## Features

- `runtime-wasm`: WebAssembly runtime (default)
- `runtime-bun`: Bun subprocess runtime (default)
- `runtime-lambda`: AWS Lambda runtime

## License

See repository root.
