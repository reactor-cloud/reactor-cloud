/**
 * Base error class for job errors.
 */
export class JobError extends Error {
  constructor(message: string, public readonly code: string) {
    super(message);
    this.name = 'JobError';
  }
}

/**
 * Error thrown when a job requests a sleep.
 *
 * This is caught by the runtime and results in the job being paused.
 */
export class JobSleepError extends JobError {
  constructor(
    public readonly stepName: string,
    public readonly durationMs: number
  ) {
    super(`Sleep requested: ${stepName} for ${durationMs}ms`, 'SLEEP_REQUESTED');
    this.name = 'JobSleepError';
  }
}

/**
 * Error thrown when a step fails.
 */
export class StepError extends JobError {
  constructor(
    public readonly stepName: string,
    message: string
  ) {
    super(`Step '${stepName}' failed: ${message}`, 'STEP_FAILED');
    this.name = 'StepError';
  }
}

/**
 * Error thrown when state operations fail.
 */
export class StateError extends JobError {
  constructor(message: string) {
    super(message, 'STATE_ERROR');
    this.name = 'StateError';
  }
}
