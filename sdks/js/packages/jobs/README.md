# @reactor/jobs

Jobs client for Reactor. Trigger and manage background jobs.

## Installation

```bash
npm install @reactor/jobs @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { JobsClient } from '@reactor/jobs';

const jobs = new JobsClient(ctx);

// Trigger a job
const { data: run, error } = await jobs.trigger('send-email', {
  payload: {
    to: 'user@example.com',
    subject: 'Hello!',
    body: 'Welcome to Reactor',
  },
});

console.log('Job ID:', run.id);

// Wait for completion
const { data: result } = await jobs.wait(run.id, {
  timeout: 30000, // 30 seconds
  pollInterval: 1000,
});

// Trigger with delay
await jobs.trigger('cleanup', {
  payload: { userId: '123' },
  runAt: new Date(Date.now() + 3600000), // 1 hour from now
});

// Trigger with idempotency key
await jobs.trigger('process-order', {
  payload: { orderId: '456' },
  idempotencyKey: 'order-456',
});
```

## Managing Runs

```typescript
// Get run status
const { data: run } = await jobs.runs.get(runId);

// List runs for a job
const { data: runs } = await jobs.runs.list('send-email', {
  status: 'failed',
  limit: 10,
});

// Cancel a run
await jobs.runs.cancel(runId);

// Retry a failed run
await jobs.runs.retry(runId);
```

## Dead Letter Queue

```typescript
// List DLQ entries
const { data: entries } = await jobs.dlq.list({
  job: 'send-email',
  limit: 100,
});

// Retry DLQ entry
await jobs.dlq.retry(entryId);

// Delete DLQ entry
await jobs.dlq.delete(entryId);
```

## Triggers (Admin)

```typescript
// List triggers
const { data: triggers } = await jobs.triggers.list();

// Create a cron trigger
await jobs.triggers.create('daily-cleanup', {
  job: 'cleanup',
  schedule: '0 0 * * *', // midnight daily
  payload: {},
});

// Delete trigger
await jobs.triggers.delete(triggerId);
```

## Documentation

- [Jobs Guide](https://reactor.cloud/docs/jobs)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
