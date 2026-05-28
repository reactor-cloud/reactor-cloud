import { JobSleepError, StepError, StateError } from './errors.js';
import type { JobContextHeader, JobPayload, StepResult, InternalEndpoints } from './types.js';

/**
 * Options for step execution.
 */
export interface StepOptions {
  /** Retry this step on failure. */
  retry?: boolean;
  /** Maximum retry attempts for this step. */
  maxRetries?: number;
}

/**
 * State operations.
 */
export interface StateOperations {
  /** Get a state value. */
  get<T = unknown>(key: string): T | undefined;
  /** Set a state value. */
  set(key: string, value: unknown): Promise<void>;
  /** Delete a state value. */
  delete(key: string): Promise<void>;
}

/**
 * Log operations.
 */
export interface LogOperations {
  /** Log an info message. */
  info(message: string, data?: Record<string, unknown>): void;
  /** Log a warning message. */
  warn(message: string, data?: Record<string, unknown>): void;
  /** Log an error message. */
  error(message: string, data?: Record<string, unknown>): void;
  /** Log a debug message. */
  debug(message: string, data?: Record<string, unknown>): void;
}

/**
 * Job execution context.
 */
export interface JobContext {
  /** Run ID. */
  readonly runId: string;
  /** Job name. */
  readonly jobName: string;
  /** Current attempt number (1-based). */
  readonly attempt: number;
  /** Job payload. */
  readonly payload: JobPayload;
  /** State operations. */
  readonly state: StateOperations;
  /** Logging operations. */
  readonly log: LogOperations;

  /**
   * Execute a step with checkpointing.
   *
   * If the step was already completed in a previous attempt, returns the cached result.
   * Otherwise, executes the function and persists the result.
   */
  step<T>(name: string, fn: () => T | Promise<T>, options?: StepOptions): Promise<T>;

  /**
   * Emit an event to trigger other jobs.
   */
  emit(topic: string, payload: unknown): Promise<void>;

  /**
   * Sleep for a duration (durable - survives restarts).
   *
   * @param name - Unique name for this sleep step
   * @param duration - Duration string (e.g., '5m', '1h', '24h') or milliseconds
   */
  sleep(name: string, duration: string | number): Promise<void>;
}

/**
 * Parse a duration string into milliseconds.
 */
function parseDuration(duration: string | number): number {
  if (typeof duration === 'number') {
    return duration;
  }

  const match = duration.match(/^(\d+)\s*(ms|s|m|h|d)$/);
  if (!match) {
    throw new Error(`Invalid duration format: ${duration}`);
  }

  const value = parseInt(match[1], 10);
  const unit = match[2];

  switch (unit) {
    case 'ms': return value;
    case 's': return value * 1000;
    case 'm': return value * 60 * 1000;
    case 'h': return value * 60 * 60 * 1000;
    case 'd': return value * 24 * 60 * 60 * 1000;
    default: throw new Error(`Unknown duration unit: ${unit}`);
  }
}

/**
 * Create a job context from the header and endpoints.
 */
export function createContext(
  header: JobContextHeader,
  payload: JobPayload,
  endpoints: InternalEndpoints
): JobContext {
  const stateCache = new Map<string, unknown>(Object.entries(header.state));

  const state: StateOperations = {
    get<T = unknown>(key: string): T | undefined {
      return stateCache.get(key) as T | undefined;
    },

    async set(key: string, value: unknown): Promise<void> {
      stateCache.set(key, value);

      // Persist to server
      const response = await fetch(`${endpoints.baseUrl}/_internal/state`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${endpoints.token}`,
          'X-Reactor-Run-Id': header.runId,
        },
        body: JSON.stringify({ key, value }),
      });

      if (!response.ok) {
        throw new StateError(`Failed to set state key '${key}': ${response.statusText}`);
      }
    },

    async delete(key: string): Promise<void> {
      stateCache.delete(key);

      // Persist to server
      const response = await fetch(`${endpoints.baseUrl}/_internal/state/${encodeURIComponent(key)}`, {
        method: 'DELETE',
        headers: {
          'Authorization': `Bearer ${endpoints.token}`,
          'X-Reactor-Run-Id': header.runId,
        },
      });

      if (!response.ok) {
        throw new StateError(`Failed to delete state key '${key}': ${response.statusText}`);
      }
    },
  };

  const log: LogOperations = {
    info(message: string, data?: Record<string, unknown>) {
      console.log(JSON.stringify({
        ts: new Date().toISOString(),
        level: 'info',
        runId: header.runId,
        message,
        ...data,
      }));
    },

    warn(message: string, data?: Record<string, unknown>) {
      console.warn(JSON.stringify({
        ts: new Date().toISOString(),
        level: 'warn',
        runId: header.runId,
        message,
        ...data,
      }));
    },

    error(message: string, data?: Record<string, unknown>) {
      console.error(JSON.stringify({
        ts: new Date().toISOString(),
        level: 'error',
        runId: header.runId,
        message,
        ...data,
      }));
    },

    debug(message: string, data?: Record<string, unknown>) {
      console.debug(JSON.stringify({
        ts: new Date().toISOString(),
        level: 'debug',
        runId: header.runId,
        message,
        ...data,
      }));
    },
  };

  return {
    runId: header.runId,
    jobName: header.jobName,
    attempt: header.attempt,
    payload,
    state,
    log,

    async step<T>(name: string, fn: () => T | Promise<T>, _options?: StepOptions): Promise<T> {
      // Check if step was already completed
      if (name in header.stepCache) {
        log.debug(`Step '${name}' returning cached result`);
        return header.stepCache[name] as T;
      }

      // Notify server that step is starting
      const startResponse = await fetch(`${endpoints.baseUrl}/_internal/steps`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${endpoints.token}`,
          'X-Reactor-Run-Id': header.runId,
        },
        body: JSON.stringify({ name }),
      });

      if (!startResponse.ok) {
        throw new StepError(name, `Failed to start step: ${startResponse.statusText}`);
      }

      const { stepId } = await startResponse.json() as { stepId: string };

      try {
        // Execute the step function
        log.info(`Starting step '${name}'`);
        const result = await fn();

        // Persist the result
        const completeResponse = await fetch(`${endpoints.baseUrl}/_internal/steps/${stepId}`, {
          method: 'PUT',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${endpoints.token}`,
          },
          body: JSON.stringify({ status: 'completed', output: result }),
        });

        if (!completeResponse.ok) {
          throw new StepError(name, `Failed to complete step: ${completeResponse.statusText}`);
        }

        // Cache locally for this run
        header.stepCache[name] = result;
        log.info(`Step '${name}' completed`);

        return result;
      } catch (error) {
        // Mark step as failed
        await fetch(`${endpoints.baseUrl}/_internal/steps/${stepId}`, {
          method: 'PUT',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${endpoints.token}`,
          },
          body: JSON.stringify({
            status: 'failed',
            error: error instanceof Error ? error.message : String(error),
          }),
        });

        log.error(`Step '${name}' failed`, { error: String(error) });
        throw error;
      }
    },

    async emit(topic: string, eventPayload: unknown): Promise<void> {
      const response = await fetch(`${endpoints.baseUrl}/_internal/events`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${endpoints.token}`,
          'X-Reactor-Run-Id': header.runId,
        },
        body: JSON.stringify({ topic, payload: eventPayload }),
      });

      if (!response.ok) {
        throw new Error(`Failed to emit event: ${response.statusText}`);
      }

      log.info(`Emitted event to topic '${topic}'`);
    },

    async sleep(name: string, duration: string | number): Promise<void> {
      // Check if sleep was already completed
      if (name in header.stepCache) {
        log.debug(`Sleep '${name}' already completed`);
        return;
      }

      const durationMs = parseDuration(duration);

      // Notify server about sleep request
      const response = await fetch(`${endpoints.baseUrl}/_internal/sleep`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${endpoints.token}`,
          'X-Reactor-Run-Id': header.runId,
        },
        body: JSON.stringify({ name, durationMs }),
      });

      if (!response.ok) {
        throw new Error(`Failed to request sleep: ${response.statusText}`);
      }

      log.info(`Sleeping for ${durationMs}ms (step: ${name})`);

      // Throw to exit the function - the scheduler will wake us up
      throw new JobSleepError(name, durationMs);
    },
  };
}
