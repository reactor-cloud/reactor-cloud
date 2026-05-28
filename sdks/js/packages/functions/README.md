# @reactor/functions

Functions client for Reactor. Invoke serverless functions deployed to Reactor.

## Installation

```bash
npm install @reactor/functions @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { FunctionsClient } from '@reactor/functions';

const functions = new FunctionsClient(ctx);

// Invoke a function
const { data, error } = await functions.invoke('hello-world', {
  body: { name: 'World' },
});

// Invoke with headers
const { data } = await functions.invoke('protected-fn', {
  body: { data: 'value' },
  headers: { 'X-Custom-Header': 'value' },
});

// Streaming response
const stream = await functions.invokeStream('generate-text', {
  body: { prompt: 'Hello' },
});

for await (const chunk of stream) {
  console.log(chunk);
}
```

## API Reference

### `invoke(name, options)`

Invoke a function and get a JSON response.

**Parameters:**
- `name` - Function name
- `options.body` - Request body (JSON-serializable)
- `options.headers` - Custom headers

**Returns:** `Promise<Result<T>>`

### `invokeStream(name, options)`

Invoke a function and get a streaming response.

**Returns:** `Promise<AsyncIterable<Uint8Array>>`

### `invokeRaw(name, options)`

Invoke a function and get the raw response.

**Returns:** `Promise<Response>`

## Admin Operations

```typescript
// List functions
const { data: fns } = await functions.list();

// Get function
const { data: fn } = await functions.get('my-function');

// Get function versions
const { data: versions } = await functions.versions('my-function');

// Rollback to version
await functions.rollback('my-function', versionId);

// Get logs
const { data: logs } = await functions.logs('my-function', {
  since: new Date(Date.now() - 3600000),
  level: 'error',
});

// Environment variables
const { data: env } = await functions.env.list('my-function');
await functions.env.set('my-function', 'KEY', 'value');
await functions.env.delete('my-function', 'KEY');
```

## Documentation

- [Functions Guide](https://reactor.cloud/docs/functions)
- [Deploying Functions](https://reactor.cloud/docs/functions#deployment)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
