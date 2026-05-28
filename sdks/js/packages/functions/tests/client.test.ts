import { describe, it, expect, vi, beforeEach } from 'vitest';
import { FunctionsClient } from '../src/index.js';
import type { RequestContext } from '@reactor/shared';

describe('FunctionsClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let mockCtx: RequestContext;
  let functions: FunctionsClient;

  beforeEach(() => {
    mockFetch = vi.fn();
    mockCtx = {
      baseUrl: 'https://api.reactor.cloud',
      projectKey: 'rk_test_123',
      fetch: mockFetch,
      getAccessToken: async () => 'mock-token',
      defaultRetries: 0, // Disable retries for tests
      defaultTimeout: 60000, // Long timeout for tests
    };
    functions = new FunctionsClient(mockCtx);
  });

  describe('invoke()', () => {
    it('should invoke a function and return JSON', async () => {
      const mockData = { result: 'success', value: 42 };
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockData)),
      });

      const result = await functions.invoke<typeof mockData>('my-function', {
        body: { input: 'test' },
      });

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockData);
      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.reactor.cloud/functions/v1/invoke/my-function',
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({ input: 'test' }),
        })
      );
    });

    it('should handle function errors', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 500,
        json: () => Promise.resolve({
          error: { code: 'FUNCTION_ERROR', message: 'Something went wrong' },
        }),
      });

      const result = await functions.invoke('failing-function');

      expect(result.data).toBeNull();
      expect(result.error).not.toBeNull();
    });

    it('should encode function names with special characters', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{}'),
      });

      await functions.invoke('my/function name');

      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.reactor.cloud/functions/v1/invoke/my%2Ffunction%20name',
        expect.any(Object)
      );
    });
  });

  describe('invokeRaw()', () => {
    it('should return raw response', async () => {
      const mockResponse = { ok: true, body: 'raw body' };
      mockFetch.mockResolvedValue(mockResponse);

      const response = await functions.invokeRaw('raw-function', {
        body: { test: true },
      });

      expect(response).toBe(mockResponse);
      expect(mockFetch).toHaveBeenCalled();
    });
  });

  describe('deploy()', () => {
    it('should deploy a function bundle', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify({ version: 'v1.0.0' })),
      });

      const bundle = new Blob(['function code']);
      const result = await functions.deploy('my-function', bundle, { version: 'v1.0.0' });

      expect(result.error).toBeNull();
      expect(result.data).toEqual({ version: 'v1.0.0' });
    });
  });

  describe('env.list()', () => {
    it('should list environment variables', async () => {
      const mockVars = [
        { name: 'API_KEY', created_at: '2024-01-01', updated_at: '2024-01-01' },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockVars)),
      });

      const result = await functions.env.list('my-function');

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockVars);
    });
  });

  describe('logs.list()', () => {
    it('should list function logs', async () => {
      const mockLogs = [
        { timestamp: '2024-01-01T00:00:00Z', level: 'info' as const, message: 'Test log' },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockLogs)),
      });

      const result = await functions.logs.list('my-function', { limit: 10 });

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockLogs);
    });
  });

  describe('versions.list()', () => {
    it('should list function versions', async () => {
      const mockVersions = [
        { version: 'v1.0.0', created_at: '2024-01-01', size_bytes: 1024, active: true },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockVersions)),
      });

      const result = await functions.versions.list('my-function');

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockVersions);
    });
  });
});
