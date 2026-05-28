# @reactor/storage

Storage client for Reactor. Upload, download, and manage files in buckets.

## Installation

```bash
npm install @reactor/storage @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { StorageClient } from '@reactor/storage';

const storage = new StorageClient(ctx);

// Upload a file
const { data, error } = await storage
  .from('avatars')
  .upload('user-123/avatar.jpg', file, {
    contentType: 'image/jpeg',
    upsert: true,
  });

// Download a file
const { data: blob } = await storage
  .from('avatars')
  .download('user-123/avatar.jpg');

// Get public URL
const { data: { publicUrl } } = storage
  .from('avatars')
  .getPublicUrl('user-123/avatar.jpg');

// Create signed URL
const { data: { signedUrl } } = await storage
  .from('private-docs')
  .createSignedUrl('report.pdf', 3600); // 1 hour

// List files
const { data: files } = await storage
  .from('avatars')
  .list('user-123/', {
    limit: 100,
    offset: 0,
  });

// Delete file
await storage.from('avatars').remove(['user-123/avatar.jpg']);

// Move/copy file
await storage.from('avatars').move('old-path.jpg', 'new-path.jpg');
await storage.from('avatars').copy('source.jpg', 'destination.jpg');
```

## Bucket Management

```typescript
// List buckets
const { data: buckets } = await storage.listBuckets();

// Create bucket
await storage.createBucket('my-bucket', {
  public: false,
  fileSizeLimit: 10 * 1024 * 1024, // 10MB
});

// Get bucket
const { data: bucket } = await storage.getBucket('my-bucket');

// Update bucket
await storage.updateBucket('my-bucket', { public: true });

// Delete bucket
await storage.deleteBucket('my-bucket');
```

## Documentation

- [Storage Guide](https://reactor.cloud/docs/storage)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
