# @reactor/data

Data client for Reactor. Query your PostgreSQL database using a PostgREST-style API.

## Installation

```bash
npm install @reactor/data @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { DataClient } from '@reactor/data';
import type { Database } from './database.types';

const data = new DataClient<Database>(ctx);

// Select
const { data: users } = await data.from('users')
  .select('id, email, profile:profiles(*)')
  .eq('active', true)
  .order('created_at', { ascending: false })
  .limit(10);

// Insert
const { data: newUser } = await data.from('users')
  .insert({ email: 'new@example.com', name: 'New User' })
  .select()
  .single();

// Update
await data.from('users')
  .update({ name: 'Updated Name' })
  .eq('id', userId);

// Delete
await data.from('users')
  .delete()
  .eq('id', userId);

// RPC
const { data: result } = await data.rpc('calculate_total', { order_id: '123' });
```

## Query Builder

### Selecting Data

```typescript
// Select specific columns
.select('id, name, email')

// Select with relationships
.select('*, posts(*), profile:profiles(*)')

// Count
.select('*', { count: 'exact' })
```

### Filtering

```typescript
.eq('column', value)        // equals
.neq('column', value)       // not equals
.gt('column', value)        // greater than
.gte('column', value)       // greater than or equal
.lt('column', value)        // less than
.lte('column', value)       // less than or equal
.like('column', pattern)    // LIKE pattern
.ilike('column', pattern)   // case-insensitive LIKE
.is('column', value)        // IS (for null, true, false)
.in('column', [values])     // IN array
.contains('column', value)  // contains (jsonb/array)
.containedBy('column', val) // contained by
.or(filters)                // OR conditions
.not(column, op, value)     // NOT
.filter(column, op, value)  // generic filter
```

### Ordering & Pagination

```typescript
.order('created_at', { ascending: false })
.limit(10)
.range(0, 9)
.single()
.maybeSingle()
```

## TypeScript Support

Generate types from your database:

```bash
reactor types generate --output ./database.types.ts
```

## Documentation

- [Data Guide](https://reactor.cloud/docs/data)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
