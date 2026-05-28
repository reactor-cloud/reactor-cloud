# reactor-analytics

Product analytics capability for the Reactor BaaS platform.

## Overview

`reactor-analytics` provides a complete product analytics solution including:

- **Event Ingestion**: High-throughput event tracking with batching and backpressure handling
- **Identity Stitching**: Anonymous-to-user ID linking via identify and alias operations
- **Query Plane**: SQL-compiled queries for events, aggregates, funnels, retention, and paths
- **Privacy**: GDPR-compliant consent management, data erasure, and DNT/GPC header support
- **Observability**: Prometheus metrics and tracing spans for monitoring

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      JS SDK (@reactor/analytics)            │
│  track() | identify() | alias() | page() | optIn/optOut()   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Analytics API (Axum)                      │
│  POST /track | /batch | /identify | /alias | /consent/*     │
│  POST /query | /erase | /export                             │
│  GET  /snippet.js | /metrics | /health                      │
└─────────────────────────────────────────────────────────────┘
                              │
            ┌─────────────────┼─────────────────┐
            ▼                 ▼                 ▼
┌───────────────────┐ ┌─────────────────┐ ┌───────────────────┐
│    Enrichment     │ │ Background      │ │   Query Compiler  │
│  IP truncation    │ │ Batcher         │ │   SQL generation  │
│  UA parsing       │ │ COPY flush      │ │   Hot-column      │
│  Geo lookup       │ │ Backpressure    │ │   awareness       │
│  UTM extraction   │ │ handling        │ │                   │
└───────────────────┘ └─────────────────┘ └───────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      PostgreSQL                              │
│  Partitioned events table | BRIN indexes | Time-based       │
│  monthly partitions                                          │
└─────────────────────────────────────────────────────────────┘
```

## Features

### Event Ingestion

- `POST /analytics/v1/track` - Single event tracking
- `POST /analytics/v1/batch` - Batch event tracking (up to 100 events)
- `POST /analytics/v1/identify` - User identification
- `POST /analytics/v1/alias` - Identity aliasing

### System Events

System events (prefixed with `$`) have hot-column optimizations:

| Event | Description |
|-------|-------------|
| `$pageview` | Page view tracking |
| `$identify` | User identification |
| `$alias` | Identity aliasing |
| `$session_start` | Session start |
| `$session_end` | Session end |
| `$autocapture` | Auto-captured interactions |
| `$error` | Error tracking |

### Privacy & Consent

- `POST /analytics/v1/consent/opt-out` - Opt out of tracking
- `POST /analytics/v1/consent/opt-in` - Opt back in
- `POST /analytics/v1/consent/status` - Check consent status
- `POST /analytics/v1/erase` - GDPR right-to-erasure
- `POST /analytics/v1/export` - GDPR data portability

### Query API

`POST /analytics/v1/query` supports multiple query types:

```json
{
  "kind": "aggregate",
  "project_id": "...",
  "time_range": { "relative": "last_7_days" },
  "measures": [{ "measure": "count" }],
  "group_by": ["day"]
}
```

Query kinds: `events`, `aggregate`, `breakdown`, `funnel`, `retention`, `path`

## Configuration

Environment variables (prefixed with `REACTOR_ANALYTICS_`):

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection URL | Required |
| `AUTH_URL` | Auth service URL for JWT validation | Required |
| `BIND_ADDR` | Server bind address | `0.0.0.0:8083` |
| `GEO_DB_PATH` | Path to MaxMind GeoLite2 database | None |
| `BATCH_SIZE` | Events per batch flush | 1000 |
| `FLUSH_INTERVAL_MS` | Batch flush interval | 1000 |

## Usage

### Standalone Server

```bash
# Run migrations
reactor-analytics-server migrate

# Start server
reactor-analytics-server serve

# Health check
reactor-analytics-server doctor
```

### As Part of Monolith

When running as part of `reactor-server`, enable the `cap-analytics` feature:

```toml
[dependencies]
reactor-server = { version = "0.1", features = ["cap-analytics"] }
```

Configure via the `analytics` section in your config:

```yaml
analytics:
  enabled: true
  batch_size: 1000
  flush_interval_ms: 1000
```

## JS SDK

```typescript
import { ReactorAnalytics } from '@reactor/analytics';

const analytics = new ReactorAnalytics({
  projectKey: 'pk_...',
  endpoint: 'https://api.reactor.cloud/analytics/v1',
  autoPageview: true,
  autoErrors: true,
});

// Track custom event
analytics.track('button_clicked', { button_id: 'signup' });

// Identify user
analytics.identify('user_123', { email: 'user@example.com' });

// Manual page view
analytics.page('Dashboard');
```

## Client for Server-Side Runtimes

For Functions/Jobs/Sites runtimes:

```rust
use reactor_analytics::{AnalyticsClient, AnalyticsClientBuilder, TrackEvent};

// In-process mode (for monolith)
let client = AnalyticsClientBuilder::new()
    .org_id(org_id)
    .project_id(project_id)
    .in_process(batcher_sender)
    .build()?;

// HTTP mode (for lambda/isolated runtimes)
let client = AnalyticsClientBuilder::new()
    .org_id(org_id)
    .project_id(project_id)
    .http_fallback("https://api.reactor.cloud/analytics/v1", "pk_...")
    .build()?;

// Track event
client.track(TrackEvent::new("function_invoked")
    .with_property("function_name", "process-order")
).await?;
```

## License

MIT
