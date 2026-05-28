/**
 * Job payload passed to the handler.
 */
export interface JobPayload {
  [key: string]: unknown;
}

/**
 * Result from a step execution.
 */
export interface StepResult<T = unknown> {
  /** Step output data. */
  output: T;
  /** Whether this was a cached result from a previous run. */
  cached: boolean;
}

/**
 * Job context header parsed from X-Reactor-Job-Context.
 */
export interface JobContextHeader {
  /** Run ID. */
  runId: string;
  /** Job name. */
  jobName: string;
  /** Current attempt number. */
  attempt: number;
  /** Cached step outputs from previous attempts. */
  stepCache: Record<string, unknown>;
  /** Current state key-value pairs. */
  state: Record<string, unknown>;
}

/**
 * Internal endpoints configuration.
 */
export interface InternalEndpoints {
  /** Base URL for internal API. */
  baseUrl: string;
  /** Internal auth token. */
  token: string;
}

/**
 * Log entry for structured logging.
 */
export interface LogEntry {
  /** Timestamp. */
  ts: string;
  /** Log level. */
  level: 'info' | 'warn' | 'error' | 'debug';
  /** Run ID. */
  runId: string;
  /** Step name (if in a step). */
  step?: string;
  /** Log message. */
  message: string;
  /** Additional data. */
  data?: Record<string, unknown>;
}
