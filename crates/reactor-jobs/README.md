# reactor-jobs

Durable job execution with step checkpointing for Reactor.

## Overview

`reactor-jobs` provides a durable job execution system built on top of `reactor-functions`. Jobs are functions that can:

- **Checkpoint steps** — if a job crashes, completed steps are skipped on retry
- **Persist state** — key-value state survives across steps and retries
- **Sleep durably** — jobs can pause and resume after a duration
- **Emit events** — trigger other jobs via internal pub/sub
- **Retry with backoff** — failed jobs retry with configurable backoff

## Quick Start

### 1. Create a job using the TypeScript SDK

```typescript
import { createJobHandler } from '@reactor/jobs-sdk';

export default createJobHandler(async (ctx) => {
  // Steps are checkpointed
  const user = await ctx.step('fetch-user', async () => {
    return await fetchUser(ctx.payload.userId);
  });

  // State persists across steps
  await ctx.state.set('user_email', user.email);

  // Durable sleep - job pauses and resumes
  await ctx.sleep('wait-24h', '24h');

  // Emit events to trigger other jobs
  await ctx.emit('user.onboarded', { userId: user.id });

  return { status: 'done' };
});
```

### 2. Deploy the job bundle

```bash
# Deploy using the reactor CLI (or via API)
reactor deploy --job my-job --function my-function.ts
```

### 3. Trigger the job

```bash
# Manual trigger
curl -X POST http://localhost:8005/jobs/v1/my-job/trigger \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Reactor-Org: $ORG_ID" \
  -H "Content-Type: application/json" \
  -d '{"userId": "123"}'
```

## Trigger Types

| Type | Description |
|------|-------------|
| `cron` | Schedule-based (e.g., `0 9 * * *`) |
| `webhook` | External HTTP trigger with encrypted token |
| `event` | Internal pub/sub (via `ctx.emit`) |
| `manual` | API trigger |

## Configuration

Environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `REACTOR_JOBS_DATABASE_URL` | PostgreSQL connection URL | Required |
| `REACTOR_JOBS_FUNCTIONS_URL` | reactor-functions server URL | Required |
| `REACTOR_JOBS_FUNCTIONS_API_KEY` | Internal API key | Required |
| `REACTOR_JOBS_WEBHOOK_SECRET` | Webhook token encryption key | Required |
| `REACTOR_JOBS_WORKER_COUNT` | Number of worker tasks | 4 |
| `REACTOR_JOBS_SCHEDULER_INTERVAL_MS` | Scheduler poll interval | 1000 |
| `REACTOR_JOBS_MAX_ORG_CONCURRENT_RUNS` | Per-org concurrency limit | 50 |

## API Endpoints

### Admin Routes (require auth)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/jobs/v1/_admin/jobs` | Create a job |
| GET | `/jobs/v1/_admin/jobs` | List jobs |
| GET | `/jobs/v1/_admin/jobs/{name}` | Get job details |
| DELETE | `/jobs/v1/_admin/jobs/{name}` | Delete a job |
| POST | `/jobs/v1/_admin/jobs/{name}/triggers` | Create trigger |
| GET | `/jobs/v1/_admin/jobs/{name}/triggers` | List triggers |
| DELETE | `/jobs/v1/_admin/jobs/{name}/triggers/{id}` | Delete trigger |
| GET | `/jobs/v1/_admin/jobs/{name}/runs` | List runs |
| GET | `/jobs/v1/_admin/jobs/{name}/runs/{id}` | Get run details |
| POST | `/jobs/v1/_admin/jobs/{name}/runs/{id}/cancel` | Cancel run |
| POST | `/jobs/v1/_admin/jobs/{name}/runs/{id}/retry` | Retry run |
| GET | `/jobs/v1/_admin/jobs/{name}/dlq` | List DLQ entries |
| POST | `/jobs/v1/_admin/jobs/{name}/dlq/{id}/retry` | Retry from DLQ |
| DELETE | `/jobs/v1/_admin/jobs/{name}/dlq/{id}` | Delete DLQ entry |
| GET | `/jobs/v1/_admin/jobs/{name}/logs` | Stream logs (SSE) |

### Trigger Routes

| Method | Path | Description |
|--------|------|-------------|
| POST | `/jobs/v1/{name}/trigger` | Manual trigger (requires auth) |
| POST | `/jobs/v1/webhooks/{token}` | Webhook trigger (no auth) |

### Observability

| Method | Path | Description |
|--------|------|-------------|
| GET | `/jobs/v1/health` | Health check |
| GET | `/jobs/v1/metrics` | Prometheus metrics |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    reactor-jobs-server                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Scheduler  │  │  Worker     │  │    HTTP Routes      │  │
│  │             │  │  Pool       │  │                     │  │
│  │  - cron     │  │             │  │  - admin CRUD       │  │
│  │  - events   │  │  dequeue → │  │  - triggers         │  │
│  │  - sleep    │  │  invoke →  │  │  - webhooks         │  │
│  │             │  │  ack/nack   │  │  - logs/metrics     │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
│         │                │                                    │
│         ▼                ▼                                    │
│  ┌─────────────────────────────────────┐                     │
│  │         reactor-cache               │                     │
│  │    (queue + KV, Postgres SKIP LOCKED)│                    │
│  └─────────────────────────────────────┘                     │
│         │                │                                    │
│         ▼                ▼                                    │
│  ┌─────────────────────────────────────┐                     │
│  │           PostgreSQL                │                     │
│  │  _reactor_jobs.{jobs,triggers,...}  │                     │
│  └─────────────────────────────────────┘                     │
│                          │                                    │
│                          ▼                                    │
│  ┌─────────────────────────────────────┐                     │
│  │       reactor-functions              │                     │
│  │  (Bun, WASM, Lambda runtimes)       │                     │
│  └─────────────────────────────────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

## License

Apache-2.0 OR MIT
