# reactor-sites

App hosting capability for Reactor — an orchestration layer over `reactor-functions` and `reactor-storage`.

## Overview

`reactor-sites` enables full-stack web application hosting with:

- **Static asset serving** via `reactor-storage`
- **Server-side rendering** via `reactor-functions`
- **Incremental Static Regeneration (ISR)** for dynamic content caching
- **Custom domains** with automatic TLS certificate provisioning
- **Preview deployments** with subdomain routing
- **Framework adapters** for Static, Hono, and Next.js

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        reactor-sites                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐        │
│  │ Host Resolver │  │ Route Matcher │  │ ISR Cache     │        │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘        │
│          │                  │                  │                 │
│          ▼                  ▼                  ▼                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    Dispatch Layer                          │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │  │
│  │  │   Static    │  │  Function   │  │  Prerender  │        │  │
│  │  │  Dispatch   │  │  Dispatch   │  │  Dispatch   │        │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │  │
│  └─────────┼────────────────┼────────────────┼───────────────┘  │
└────────────┼────────────────┼────────────────┼──────────────────┘
             │                │                │
             ▼                ▼                ▼
    ┌────────────────┐ ┌────────────────┐ ┌────────────────┐
    │reactor-storage │ │reactor-functions│ │ reactor-cache  │
    └────────────────┘ └────────────────┘ └────────────────┘
```

## Bundle Format

Sites uses a Vercel Build Output API-shaped bundle format:

```
.reactor/output/
├── config.json           # Manifest with routes, caching rules
├── static/               # Static assets (copied to storage)
│   ├── index.html
│   ├── styles.css
│   └── images/
└── functions/            # SSR functions (deployed to reactor-functions)
    └── api/
        └── handler.func/
            └── index.js
```

## Features

### Framework Adapters

- **Static** (`framework-static`): Pure static sites
- **Hono** (`framework-hono`): Hono.js apps with Bun bundling
- **Next.js** (`framework-nextjs`): Full Next.js support with SSR, ISR, API routes

### Custom Domains

- DNS verification (TXT record) or HTTP verification (.well-known)
- Automatic ACME certificate provisioning (feature-gated: `domain-acme`)
- Certificate renewal via `reactor-jobs`

### ISR (Incremental Static Regeneration)

- Time-based revalidation
- On-demand revalidation via `/revalidate` endpoint
- Tag-based invalidation
- Stale-while-revalidate pattern

### Preview Deployments

- Auto-generated preview URLs: `{deployment-id}.{site}.preview.{domain}`
- Instant promotion and rollback
- Full parity with production

## Configuration

Environment variables:

```bash
# Required
DATABASE_URL=postgres://localhost/reactor
FUNCTIONS_URL=http://localhost:3001
STORAGE_URL=http://localhost:3002

# Optional
BIND_ADDRESS=0.0.0.0:3003
DEPLOYMENT_MODE=monolith  # or microservices
AUTH_URL=http://localhost:3000
FUNCTIONS_API_KEY=secret
STORAGE_API_KEY=secret
PREVIEW_SUBDOMAIN=preview
MAX_BUNDLE_SIZE_BYTES=104857600  # 100MB
MAX_STATIC_ASSET_COUNT=10000
```

## API

### Admin Plane

| Method | Path | Description |
|--------|------|-------------|
| GET | `/sites/v1/health` | Health check |
| POST | `/sites/v1/sites` | Create site |
| GET | `/sites/v1/sites` | List sites |
| GET | `/sites/v1/sites/:id` | Get site |
| DELETE | `/sites/v1/sites/:id` | Delete site |
| POST | `/sites/v1/sites/:id/deployments` | Upload bundle |
| GET | `/sites/v1/sites/:id/deployments` | List deployments |
| POST | `/sites/v1/sites/:id/deployments/:did/promote` | Promote deployment |
| POST | `/sites/v1/sites/:id/deployments/:did/rollback` | Rollback |
| POST | `/sites/v1/sites/:id/domains` | Add custom domain |
| POST | `/sites/v1/sites/:id/domains/:did/verify` | Verify domain |
| POST | `/sites/v1/sites/:id/revalidate` | On-demand ISR revalidation |

### Serve Plane

| Method | Path | Description |
|--------|------|-------------|
| * | `/*` | Serve site content (resolved by Host header) |

## CLI

```bash
# Run doctor to check dependencies
reactor-sites-server doctor

# Start server
reactor-sites-server
```

## Development

```bash
# Check compilation
cargo check -p reactor-sites -p reactor-sites-server

# Run tests
cargo test -p reactor-sites

# Build with all features
cargo build -p reactor-sites --all-features
```

## Feature Flags

- `framework-static` (default): Static site adapter
- `framework-hono` (default): Hono.js adapter
- `framework-nextjs` (default): Next.js adapter
- `domain-acme`: ACME certificate provisioning

## Dependencies

- `reactor-core`: Auth types, ID types
- `reactor-policy`: Per-site access policies
- `reactor-cache`: ISR cache backend
- `reactor-functions`: SSR function execution (external service)
- `reactor-storage`: Static asset storage (external service)
- `reactor-jobs`: Certificate renewal, ISR revalidation (external service)

## License

MIT
