import { describe, it, expect, vi, beforeEach } from 'vitest';
import { StorageClient, StorageBucketClient } from '../src/index.js';
import type { RequestContext } from '@reactor/shared';

describe('StorageClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let mockCtx: RequestContext;
  let storage: StorageClient;

  beforeEach(() => {
    mockFetch = vi.fn();
    mockCtx = {
      baseUrl: 'https://api.reactor.cloud',
      projectKey: 'rk_test_123',
      fetch: mockFetch,
      getAccessToken: async () => 'mock-token',
      defaultRetries: 0,
      defaultTimeout: 60000,
    };
    storage = new StorageClient(mockCtx);
  });

  describe('from()', () => {
    it('should return a StorageBucketClient', () => {
      const bucket = storage.from('avatars');
      expect(bucket).toBeInstanceOf(StorageBucketClient);
    });
  });
});

describe('StorageBucketClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let mockCtx: RequestContext;
  let bucket: StorageBucketClient;

  beforeEach(() => {
    mockFetch = vi.fn();
    mockCtx = {
      baseUrl: 'https://api.reactor.cloud',
      projectKey: 'rk_test_123',
      fetch: mockFetch,
      getAccessToken: async () => 'mock-token',
      defaultRetries: 0,
      defaultTimeout: 60000,
    };
    bucket = new StorageBucketClient(mockCtx, 'avatars');
  });

  describe('upload()', () => {
    it('should upload a file', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify({
          path: 'avatars/test.jpg',
          id: 'file-123',
          fullPath: 'avatars/test.jpg',
        })),
      });

      const blob = new Blob(['test'], { type: 'image/jpeg' });
      const result = await bucket.upload('test.jpg', blob);

      expect(result.error).toBeNull();
      expect(result.data).toEqual({
        path: 'avatars/test.jpg',
        id: 'file-123',
        fullPath: 'avatars/test.jpg',
      });
    });
  });

  describe('download()', () => {
    it('should download a file', async () => {
      const mockBlob = new Blob(['test content']);
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(''),
        blob: () => Promise.resolve(mockBlob),
      });

      const result = await bucket.download('test.jpg');

      expect(result.error).toBeNull();
      expect(result.data).toBeInstanceOf(Blob);
    });
  });

  describe('getPublicUrl()', () => {
    it('should return public URL', () => {
      const url = bucket.getPublicUrl('test.jpg');
      expect(url).toBe('https://api.reactor.cloud/storage/v1/object/public/avatars/test.jpg');
    });
  });

  describe('createSignedUrl()', () => {
    it('should create signed URL', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify({ signedUrl: 'https://signed-url.example.com' })),
      });

      const result = await bucket.createSignedUrl('test.jpg', 3600);

      expect(result.error).toBeNull();
      expect(result.data?.signedUrl).toBe('https://signed-url.example.com');
    });
  });

  describe('list()', () => {
    it('should list files', async () => {
      const mockFiles = [
        { name: 'file1.jpg', id: '1', created_at: '2024-01-01', updated_at: '2024-01-01' },
        { name: 'file2.jpg', id: '2', created_at: '2024-01-01', updated_at: '2024-01-01' },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockFiles)),
      });

      const result = await bucket.list();

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockFiles);
    });
  });
});
