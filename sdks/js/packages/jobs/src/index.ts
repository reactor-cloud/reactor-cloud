import {
  type RequestContext,
  type Result,
  get,
  post,
  del,
  ok,
} from '@reactor/shared';

export type RunStatus = 'pending' | 'running' | 'succeeded' | 'failed' | 'cancelled';

export interface JobRun {
  id: string;
  job_name: string;
  status: RunStatus;
  payload?: unknown;
  result?: unknown;
  error?: string;
  started_at?: string;
  completed_at?: string;
  created_at: string;
}

export interface TriggerOptions {
  payload?: unknown;
  idempotencyKey?: string;
}

export interface ListRunsOptions {
  jobName?: string;
  status?: RunStatus;
  limit?: number;
  offset?: number;
}

export interface WaitOptions {
  timeoutMs?: number;
  pollIntervalMs?: number;
}

export interface JobTrigger {
  id: string;
  job_name: string;
  cron?: string;
  webhook?: boolean;
  created_at: string;
}

export interface DlqEntry {
  id: string;
  job_name: string;
  payload: unknown;
  error: string;
  attempts: number;
  created_at: string;
}

export class JobsClient {
  constructor(private ctx: RequestContext) {}

  /**
   * Trigger a job run.
   */
  async trigger(name: string, options?: TriggerOptions): Promise<Result<{ runId: string }>> {
    const headers: Record<string, string> = {};
    if (options?.idempotencyKey) {
      headers['Idempotency-Key'] = options.idempotencyKey;
    }

    return post(
      this.ctx,
      `/jobs/v1/trigger/${encodeURIComponent(name)}`,
      { payload: options?.payload },
      { headers }
    );
  }

  /** Job runs management */
  get runs() {
    return {
      get: async (runId: string): Promise<Result<JobRun>> =>
        get(this.ctx, `/jobs/v1/runs/${encodeURIComponent(runId)}`),

      list: async (options?: ListRunsOptions): Promise<Result<JobRun[]>> => {
        const params = new URLSearchParams();
        if (options?.jobName) params.set('job_name', options.jobName);
        if (options?.status) params.set('status', options.status);
        if (options?.limit) params.set('limit', String(options.limit));
        if (options?.offset) params.set('offset', String(options.offset));
        return get(this.ctx, `/jobs/v1/runs?${params}`);
      },

      cancel: async (runId: string): Promise<Result<void>> =>
        post(this.ctx, `/jobs/v1/runs/${encodeURIComponent(runId)}/cancel`, {}),

      wait: async (runId: string, options?: WaitOptions): Promise<Result<JobRun>> => {
        const timeout = options?.timeoutMs ?? 60000;
        const startTime = Date.now();
        let pollInterval = options?.pollIntervalMs ?? 1000;
        const maxInterval = 30000;

        while (Date.now() - startTime < timeout) {
          const result = await get<JobRun>(this.ctx, `/jobs/v1/runs/${encodeURIComponent(runId)}`);

          if (result.error) {
            return result;
          }

          const run = result.data;
          if (run.status === 'succeeded' || run.status === 'failed' || run.status === 'cancelled') {
            return ok(run);
          }

          // Exponential backoff with cap
          await new Promise((r) => setTimeout(r, pollInterval));
          pollInterval = Math.min(pollInterval * 2, maxInterval);
        }

        // Return current state on timeout
        return get(this.ctx, `/jobs/v1/runs/${encodeURIComponent(runId)}`);
      },
    };
  }

  /** Dead letter queue management */
  get dlq() {
    return {
      list: async (options?: { limit?: number; offset?: number }): Promise<Result<DlqEntry[]>> => {
        const params = new URLSearchParams();
        if (options?.limit) params.set('limit', String(options.limit));
        if (options?.offset) params.set('offset', String(options.offset));
        return get(this.ctx, `/jobs/v1/dlq?${params}`);
      },

      retry: async (entryId: string): Promise<Result<{ runId: string }>> =>
        post(this.ctx, `/jobs/v1/dlq/${encodeURIComponent(entryId)}/retry`, {}),

      remove: async (entryId: string): Promise<Result<void>> =>
        del(this.ctx, `/jobs/v1/dlq/${encodeURIComponent(entryId)}`),
    };
  }

  /** Trigger management (admin) */
  get triggers() {
    return {
      create: async (
        jobName: string,
        config: { cron?: string; webhook?: boolean }
      ): Promise<Result<JobTrigger>> =>
        post(this.ctx, `/jobs/v1/triggers`, { job_name: jobName, ...config }),

      list: async (jobName?: string): Promise<Result<JobTrigger[]>> => {
        const params = jobName ? `?job_name=${encodeURIComponent(jobName)}` : '';
        return get(this.ctx, `/jobs/v1/triggers${params}`);
      },

      delete: async (triggerId: string): Promise<Result<void>> =>
        del(this.ctx, `/jobs/v1/triggers/${encodeURIComponent(triggerId)}`),
    };
  }
}

export function createJobsClient(ctx: RequestContext): JobsClient {
  return new JobsClient(ctx);
}
