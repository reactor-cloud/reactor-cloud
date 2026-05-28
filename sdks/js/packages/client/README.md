# @reactor/client

The official Reactor JS/TS SDK. A unified client for all Reactor capabilities.

## Installation

```bash
npm install @reactor/client
# or
pnpm add @reactor/client
# or
yarn add @reactor/client
```

## Quick Start

```typescript
import { createClient } from '@reactor/client';
import type { Database } from './database.types';

const reactor = createClient<Database>('https://your-project.reactor.cloud', {
  key: 'rk_pub_...',
});

// Authentication
const { data: { user } } = await reactor.auth.signIn({
  email: 'user@example.com',
  password: 'password',
});

// Data queries (PostgREST-style)
const { data: posts } = await reactor.from('posts')
  .select('*, author:users(*)')
  .eq('published', true)
  .order('created_at', { ascending: false })
  .limit(10);

// Storage
const { data } = await reactor.storage
  .from('avatars')
  .upload('me.jpg', file, { contentType: 'image/jpeg' });

// Functions
const { data: result } = await reactor.functions.invoke('process-order', {
  body: { orderId: '123' },
});

// Jobs
await reactor.jobs.trigger('send-email', {
  payload: { to: 'user@example.com', subject: 'Hello!' },
});
```

## API Reference

### `createClient<Schema>(url, options)`

Creates a new Reactor client instance.

**Parameters:**
- `url` - The Reactor API URL
- `options.key` - Project key (anon key) for authentication
- `options.org` - Default organization context
- `options.fetch` - Custom fetch implementation
- `options.headers` - Global headers for all requests
- `options.auth` - Auth-specific options
- `options.storage` - Custom storage adapter for session persistence

**Returns:** `ReactorClient<Schema>`

### Client Properties

- `auth` - Authentication client
- `from(table)` - Data query builder
- `rpc(fn, args)` - RPC function calls
- `storage` - Storage client
- `functions` - Functions client
- `jobs` - Jobs client
- `sites` - Sites admin client
- `realtime` - Realtime client

## TypeScript Support

Generate types from your database schema:

```bash
reactor types generate --output ./database.types.ts
```

Then use them with the client:

```typescript
import { createClient } from '@reactor/client';
import type { Database } from './database.types';

const reactor = createClient<Database>(url, options);

// Full type safety for queries
const { data } = await reactor.from('users').select('id, email');
// data is typed as { id: string; email: string }[]
```

## Documentation

- [Full API Reference](https://reactor.cloud/docs)
- [Authentication](https://reactor.cloud/docs/auth)
- [Data](https://reactor.cloud/docs/data)
- [Storage](https://reactor.cloud/docs/storage)
- [Functions](https://reactor.cloud/docs/functions)
- [Jobs](https://reactor.cloud/docs/jobs)

## License

MIT
