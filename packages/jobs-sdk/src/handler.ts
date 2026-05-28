import { createContext, type JobContext } from './context.js';
import { JobSleepError, JobError } from './errors.js';
import type { JobContextHeader, JobPayload, InternalEndpoints } from './types.js';

/**
 * Job handler function type.
 */
export type JobHandler<T = unknown> = (ctx: JobContext) => T | Promise<T>;

/**
 * Environment variables for job execution.
 */
interface JobEnv {
  /** Internal API URL. */
  REACTOR_JOBS_INTERNAL_URL?: string;
  /** Internal auth token. */
  REACTOR_JOBS_INTERNAL_TOKEN?: string;
  /** Alternative: full context as JSON (for env-based runtimes). */
  REACTOR_JOB_CONTEXT?: string;
}

/**
 * Parse the job context from header or environment.
 */
function parseContext(
  request: Request,
  env: JobEnv
): { header: JobContextHeader; endpoints: InternalEndpoints } {
  // Try to get context from header first
  const headerValue = request.headers.get('X-Reactor-Job-Context');

  if (headerValue) {
    try {
      const header = JSON.parse(headerValue) as JobContextHeader;
      const endpoints: InternalEndpoints = {
        baseUrl: env.REACTOR_JOBS_INTERNAL_URL || 'http://localhost:8005',
        token: env.REACTOR_JOBS_INTERNAL_TOKEN || '',
      };
      return { header, endpoints };
    } catch (e) {
      throw new JobError('Invalid X-Reactor-Job-Context header', 'INVALID_CONTEXT');
    }
  }

  // Try to get from environment (for non-HTTP runtimes)
  if (env.REACTOR_JOB_CONTEXT) {
    try {
      const ctx = JSON.parse(env.REACTOR_JOB_CONTEXT) as {
        header: JobContextHeader;
        endpoints: InternalEndpoints;
      };
      return ctx;
    } catch (e) {
      throw new JobError('Invalid REACTOR_JOB_CONTEXT environment variable', 'INVALID_CONTEXT');
    }
  }

  throw new JobError('Missing job context', 'MISSING_CONTEXT');
}

/**
 * Create a job handler that can be exported for Bun.serve.
 *
 * @example
 * ```ts
 * export default createJobHandler(async (ctx) => {
 *   const result = await ctx.step('my-step', async () => {
 *     return { success: true };
 *   });
 *   return result;
 * });
 * ```
 */
export function createJobHandler<T>(handler: JobHandler<T>) {
  return {
    async fetch(request: Request, env: JobEnv = {}): Promise<Response> {
      // Only accept POST requests
      if (request.method !== 'POST') {
        return new Response(JSON.stringify({ error: 'Method not allowed' }), {
          status: 405,
          headers: { 'Content-Type': 'application/json' },
        });
      }

      try {
        // Parse context
        const { header, endpoints } = parseContext(request, env);

        // Parse payload
        let payload: JobPayload = {};
        const contentType = request.headers.get('content-type');
        if (contentType?.includes('application/json')) {
          payload = await request.json() as JobPayload;
        }

        // Create context
        const ctx = createContext(header, payload, endpoints);

        // Execute handler
        const result = await handler(ctx);

        // Return success response
        return new Response(JSON.stringify({ status: 'completed', result }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        });
      } catch (error) {
        // Handle sleep requests
        if (error instanceof JobSleepError) {
          return new Response(JSON.stringify({
            status: 'sleeping',
            stepName: error.stepName,
            durationMs: error.durationMs,
          }), {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          });
        }

        // Handle other errors
        const message = error instanceof Error ? error.message : String(error);
        const code = error instanceof JobError ? error.code : 'UNKNOWN_ERROR';

        console.error(JSON.stringify({
          ts: new Date().toISOString(),
          level: 'error',
          message: `Job handler failed: ${message}`,
          code,
          stack: error instanceof Error ? error.stack : undefined,
        }));

        return new Response(JSON.stringify({
          status: 'failed',
          error: { code, message },
        }), {
          status: 500,
          headers: { 'Content-Type': 'application/json' },
        });
      }
    },
  };
}
