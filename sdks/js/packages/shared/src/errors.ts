/**
 * Base error class for all Reactor SDK errors.
 */
export class ReactorError extends Error {
  /** Error code (e.g., 'invalid_credentials', 'not_found') */
  code: string;
  /** HTTP status code */
  statusCode: number;
  /** Optional hint for resolution */
  hint?: string;
  /** Original error cause */
  override cause?: Error;

  constructor(
    message: string,
    code: string,
    statusCode: number,
    options?: { hint?: string; cause?: Error }
  ) {
    super(message);
    this.name = 'ReactorError';
    this.code = code;
    this.statusCode = statusCode;
    this.hint = options?.hint;
    this.cause = options?.cause;

    // Maintain proper stack trace in V8 environments
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }

  toJSON() {
    return {
      name: this.name,
      message: this.message,
      code: this.code,
      statusCode: this.statusCode,
      hint: this.hint,
    };
  }
}

/**
 * Authentication/authorization errors (401, 403).
 */
export class AuthError extends ReactorError {
  constructor(message: string, code: string, options?: { hint?: string; cause?: Error }) {
    super(message, code, 401, options);
    this.name = 'AuthError';
  }
}

/**
 * Forbidden error (403).
 */
export class ForbiddenError extends ReactorError {
  constructor(message: string, code: string, options?: { hint?: string; cause?: Error }) {
    super(message, code, 403, options);
    this.name = 'ForbiddenError';
  }
}

/**
 * Validation errors (400, 422).
 */
export class ValidationError extends ReactorError {
  /** Field-level errors */
  fields?: Record<string, string[]>;

  constructor(
    message: string,
    code: string,
    options?: { hint?: string; cause?: Error; fields?: Record<string, string[]> }
  ) {
    super(message, code, 400, options);
    this.name = 'ValidationError';
    this.fields = options?.fields;
  }
}

/**
 * Resource not found (404).
 */
export class NotFoundError extends ReactorError {
  constructor(message: string, code: string = 'not_found', options?: { hint?: string; cause?: Error }) {
    super(message, code, 404, options);
    this.name = 'NotFoundError';
  }
}

/**
 * Conflict error (409).
 */
export class ConflictError extends ReactorError {
  constructor(message: string, code: string = 'conflict', options?: { hint?: string; cause?: Error }) {
    super(message, code, 409, options);
    this.name = 'ConflictError';
  }
}

/**
 * Rate limit exceeded (429).
 */
export class RateLimitError extends ReactorError {
  /** Seconds until retry is allowed */
  retryAfter?: number;

  constructor(
    message: string,
    code: string = 'rate_limited',
    options?: { hint?: string; cause?: Error; retryAfter?: number }
  ) {
    super(message, code, 429, options);
    this.name = 'RateLimitError';
    this.retryAfter = options?.retryAfter;
  }
}

/**
 * Server error (5xx).
 */
export class ServerError extends ReactorError {
  constructor(message: string, code: string = 'server_error', options?: { hint?: string; cause?: Error }) {
    super(message, code, 500, options);
    this.name = 'ServerError';
  }
}

/**
 * Network/connection errors.
 */
export class NetworkError extends ReactorError {
  constructor(message: string, options?: { cause?: Error }) {
    super(message, 'network_error', 0, options);
    this.name = 'NetworkError';
  }
}

/**
 * Request aborted by user.
 */
export class AbortError extends ReactorError {
  constructor(message: string = 'Request was aborted') {
    super(message, 'aborted', 0);
    this.name = 'AbortError';
  }
}

/**
 * Request timeout.
 */
export class TimeoutError extends ReactorError {
  constructor(message: string = 'Request timed out') {
    super(message, 'timeout', 0);
    this.name = 'TimeoutError';
  }
}

/**
 * Server error response envelope.
 */
export interface ErrorEnvelope {
  error: {
    code: string;
    message: string;
    status?: number;
    hint?: string;
    fields?: Record<string, string[]>;
  };
}

/**
 * Check if an object looks like a server error envelope.
 */
export function isErrorEnvelope(obj: unknown): obj is ErrorEnvelope {
  if (typeof obj !== 'object' || obj === null) {
    return false;
  }
  if (!('error' in obj)) {
    return false;
  }
  const envelope = obj as { error: unknown };
  if (typeof envelope.error !== 'object' || envelope.error === null) {
    return false;
  }
  const error = envelope.error as Record<string, unknown>;
  return typeof error.code === 'string' && typeof error.message === 'string';
}

/**
 * Create an appropriate error from an HTTP response.
 */
export function errorFromResponse(status: number, body: unknown): ReactorError {
  let code = 'unknown';
  let message = 'An unknown error occurred';
  let hint: string | undefined;
  let fields: Record<string, string[]> | undefined;

  if (isErrorEnvelope(body)) {
    code = body.error.code;
    message = body.error.message;
    hint = body.error.hint;
    fields = body.error.fields;
  } else if (typeof body === 'string') {
    message = body;
  }

  const opts = { hint, fields };

  switch (status) {
    case 400:
    case 422:
      return new ValidationError(message, code, opts);
    case 401:
      return new AuthError(message, code, { hint });
    case 403:
      return new ForbiddenError(message, code, { hint });
    case 404:
      return new NotFoundError(message, code, { hint });
    case 409:
      return new ConflictError(message, code, { hint });
    case 429:
      return new RateLimitError(message, code, { hint });
    default:
      if (status >= 500) {
        return new ServerError(message, code, { hint });
      }
      return new ReactorError(message, code, status, { hint });
  }
}
