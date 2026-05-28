# @reactor/shared

Shared utilities, types, and HTTP helpers for Reactor SDK packages.

> **Note:** This is an internal package. For most use cases, install `@reactor/client` instead.

## Installation

```bash
npm install @reactor/shared
```

## Contents

### Types

- `User`, `Session`, `Organization`, `Member`, `Role`, `Invitation`, `ApiKey`
- `Result<T>`, `ReactorError`, `AuthError`, `ValidationError`, `NotFoundError`
- `RequestContext`, `StorageAdapter`

### HTTP Helpers

```typescript
import { get, post, patch, del, ok, err } from '@reactor/shared';

// Make typed requests
const result = await get<User>(ctx, '/auth/v1/user');

// Handle results
if (result.error) {
  console.error(result.error.message);
} else {
  console.log(result.data);
}
```

### Storage Adapters

```typescript
import { detectStorageAdapter, localStorageAdapter, memoryStorageAdapter } from '@reactor/shared';

// Auto-detect (localStorage in browser, memory in Node)
const storage = detectStorageAdapter();

// Or use specific adapter
const storage = localStorageAdapter();
```

## License

MIT
