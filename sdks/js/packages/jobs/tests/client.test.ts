import { describe, it, expect, vi, beforeEach } from 'vitest';
import { JobsClient } from '../src/index.js';
import type { RequestContext } from '@reactor/shared';

describe('JobsClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let mockCtx: RequestContext;
  let jobs: JobsClient;

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
    jobs = new JobsClient(mockCtx);
  });

  describe('trigger()', () => {
    it('should trigger a job', async () => {
      const mockRun = {
        id: 'run-123',
        job_name: 'email-sender',
        status: 'pending' as const,
        created_at: '2024-01-01T00:00:00Z',
        started_at: null,
        completed_at: null,
        payload: { to: 'test@example.com' },
        result: null,
        error: null,
        attempt: 1,
        max_attempts: 3,
      };
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockRun)),
      });

      const result = await jobs.trigger('email-sender', {
        payload: { to: 'test@example.com' },
      });

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockRun);
    });
  });

  describe('runs.get()', () => {
    it('should get a job run', async () => {
      const mockRun = {
        id: 'run-123',
        job_name: 'email-sender',
        status: 'completed' as const,
        created_at: '2024-01-01T00:00:00Z',
        started_at: '2024-01-01T00:00:01Z',
        completed_at: '2024-01-01T00:00:05Z',
        payload: { to: 'test@example.com' },
        result: { sent: true },
        error: null,
        attempt: 1,
        max_attempts: 3,
      };
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockRun)),
      });

      const result = await jobs.runs.get('run-123');

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockRun);
    });
  });

  describe('runs.list()', () => {
    it('should list job runs', async () => {
      const mockRuns = [
        {
          id: 'run-1',
          job_name: 'job1',
          status: 'completed' as const,
          created_at: '2024-01-01',
          started_at: '2024-01-01',
          completed_at: '2024-01-01',
          payload: {},
          result: {},
          error: null,
          attempt: 1,
          max_attempts: 3,
        },
        {
          id: 'run-2',
          job_name: 'job1',
          status: 'pending' as const,
          created_at: '2024-01-02',
          started_at: null,
          completed_at: null,
          payload: {},
          result: null,
          error: null,
          attempt: 1,
          max_attempts: 3,
        },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockRuns)),
      });

      const result = await jobs.runs.list();

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockRuns);
    });
  });

  describe('runs.cancel()', () => {
    it('should cancel a job run', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify({ cancelled: true })),
      });

      const result = await jobs.runs.cancel('run-123');

      expect(result.error).toBeNull();
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/jobs/v1/runs/run-123/cancel'),
        expect.objectContaining({ method: 'POST' })
      );
    });
  });
});
