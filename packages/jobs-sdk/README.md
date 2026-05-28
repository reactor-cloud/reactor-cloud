# @reactor/jobs-sdk

Reactor Jobs SDK for building durable jobs with step checkpointing.

## Installation

```bash
npm install @reactor/jobs-sdk
```

## Quick Start

```typescript
import { createJobHandler } from '@reactor/jobs-sdk';

export default createJobHandler(async (ctx) => {
  // Steps are checkpointed - if the job restarts, completed steps are skipped
  const user = await ctx.step('fetch-user', async () => {
    return await fetchUser(ctx.payload.userId);
  });

  // State persists across steps and retries
  ctx.state.set('user_email', user.email);

  // Send an email
  await ctx.step('send-welcome', async () => {
    return await sendEmail(user.email, 'Welcome!');
  });

  // Durable sleep - job pauses and resumes after the duration
  await ctx.sleep('wait-24h', '24h');

  // Emit events to trigger other jobs
  await ctx.emit('user.onboarded', { userId: user.id });

  // Final step
  await ctx.step('send-followup', async () => {
    return await sendEmail(user.email, 'How are things going?');
  });

  return { status: 'completed' };
});
```

## API

### `createJobHandler(handler)`

Creates a job handler that can be exported for Bun.serve or other serverless runtimes.

### `ctx.step(name, fn, options?)`

Execute a checkpointed step. If the step was already completed in a previous run, returns the cached result.

```typescript
const result = await ctx.step('my-step', async () => {
  return await expensiveOperation();
});
```

### `ctx.state`

Persistent key-value state scoped to the current run.

```typescript
// Get a value (sync, from cache)
const email = ctx.state.get<string>('email');

// Set a value (async, persisted to server)
await ctx.state.set('email', user.email);

// Delete a value
await ctx.state.delete('email');
```

### `ctx.emit(topic, payload)`

Emit an event to trigger jobs subscribed to the topic.

```typescript
await ctx.emit('order.created', { orderId: '123' });
```

### `ctx.sleep(name, duration)`

Pause the job for a duration. The job will be resumed by the scheduler after the duration.

```typescript
// Duration can be a string or milliseconds
await ctx.sleep('wait-5m', '5m');
await ctx.sleep('wait-1h', '1h');
await ctx.sleep('wait-custom', 30000); // 30 seconds
```

Duration format:
- `ms` - milliseconds
- `s` - seconds
- `m` - minutes
- `h` - hours
- `d` - days

### `ctx.log`

Structured logging.

```typescript
ctx.log.info('Processing user', { userId: user.id });
ctx.log.warn('Rate limit approaching');
ctx.log.error('Failed to send email', { error: err.message });
ctx.log.debug('Debug info');
```

### Context Properties

- `ctx.runId` - Unique run identifier
- `ctx.jobName` - Name of the job
- `ctx.attempt` - Current attempt number (1-based)
- `ctx.payload` - Job payload

## Error Handling

Steps that throw errors will cause the job to retry (up to `maxAttempts`). After all retries are exhausted, the job is moved to the Dead Letter Queue (DLQ).

```typescript
await ctx.step('risky-step', async () => {
  // If this throws, the job will retry
  if (Math.random() < 0.1) {
    throw new Error('Random failure');
  }
  return { success: true };
});
```

## Environment Variables

When running outside of Bun.serve (e.g., in Lambda), the SDK can read context from environment variables:

- `REACTOR_JOB_CONTEXT` - Full context JSON (used by wasm/lambda runtimes)
- `REACTOR_JOBS_INTERNAL_URL` - Internal API URL
- `REACTOR_JOBS_INTERNAL_TOKEN` - Internal auth token

## License

Apache-2.0 OR MIT
