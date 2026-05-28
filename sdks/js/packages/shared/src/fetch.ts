import {
  ReactorError,
  NetworkError,
  TimeoutError,
  AbortError,
  errorFromResponse,
  isErrorEnvelope,
} from './errors.js';
import type { Result } from './result.js';
import { ok, err } from './result.js';

/**
 * SDK version - injected at build time.
 */
export const SDK_VERSION = '0.1.0';

/**
 * Request options for the fetch wrapper.
 */
export interface RequestOptions {
  /** HTTP method */
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  /** Request body (will be JSON-stringified) */
  body?: unknown;
  /** Additional headers */
  headers?: Record<string, string>;
  /** AbortSignal for cancellation */
  signal?: AbortSignal;
  /** Timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Number of retries on 5xx/network errors (default: 3) */
  retries?: number;
  /** Expected response type */
  responseType?: 'json' | 'text' | 'blob' | 'stream';
}

/**
 * Request context shared across all SDK operations.
 */
export interface RequestContext {
  /** Base URL for API requests */
  baseUrl: string;
  /** Project key (anon key) */
  projectKey?: string;
  /** Current access token (JWT) */
  getAccessToken?: () => string | null | Promise<string | null>;
  /** Custom fetch implementation */
  fetch?: typeof fetch;
  /** Default headers to include in all requests */
  defaultHeaders?: Record<string, string>;
  /** Default timeout in milliseconds */
  defaultTimeout?: number;
  /** Default number of retries */
  defaultRetries?: number;
}

/**
 * Delay helper for retry backoff.
 */
function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Calculate exponential backoff delay.
 */
function getBackoffDelay(attempt: number, baseMs: number = 1000, maxMs: number = 10000): number {
  const delay = Math.min(baseMs * Math.pow(2, attempt), maxMs);
  // Add jitter (±25%)
  return delay * (0.75 + Math.random() * 0.5);
}

/**
 * Check if an error is retryable.
 */
function isRetryable(status: number): boolean {
  return status >= 500 || status === 429;
}

/**
 * Make an HTTP request with the SDK conventions.
 */
export async function request<T>(
  ctx: RequestContext,
  path: string,
  options: RequestOptions = {}
): Promise<Result<T, ReactorError>> {
  const {
    method = 'GET',
    body,
    headers: customHeaders = {},
    signal,
    timeout = ctx.defaultTimeout ?? 30000,
    retries = ctx.defaultRetries ?? 3,
    responseType = 'json',
  } = options;

  const fetchFn = ctx.fetch ?? globalThis.fetch;

  // Build headers
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: responseType === 'json' ? 'application/json' : '*/*',
    'X-Reactor-Client': `js/${SDK_VERSION}`,
    ...ctx.defaultHeaders,
    ...customHeaders,
  };

  // Add project key if available
  if (ctx.projectKey) {
    headers['X-Reactor-Project-Key'] = ctx.projectKey;
  }

  // Add auth token if available
  if (ctx.getAccessToken) {
    const token = await ctx.getAccessToken();
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }
  }

  // Build URL
  const url = new URL(path, ctx.baseUrl).toString();

  // Build request init
  const init: RequestInit = {
    method,
    headers,
    signal,
  };

  if (body !== undefined && method !== 'GET') {
    init.body = JSON.stringify(body);
  }

  // Retry loop
  let lastError: ReactorError | null = null;
  let attempt = 0;

  while (attempt <= retries) {
    // Create timeout controller
    const timeoutController = new AbortController();
    const timeoutId = setTimeout(() => timeoutController.abort(), timeout);

    // Combine signals if user provided one
    let combinedSignal = timeoutController.signal;
    if (signal) {
      const combined = new AbortController();
      signal.addEventListener('abort', () => combined.abort(signal.reason));
      timeoutController.signal.addEventListener('abort', () =>
        combined.abort(new TimeoutError())
      );
      combinedSignal = combined.signal;
    }

    try {
      const response = await fetchFn(url, {
        ...init,
        signal: combinedSignal,
      });

      clearTimeout(timeoutId);

      // Handle non-OK responses
      if (!response.ok) {
        let responseBody: unknown;
        try {
          responseBody = await response.json();
        } catch {
          responseBody = await response.text().catch(() => '');
        }

        const error = errorFromResponse(response.status, responseBody);

        // Retry on 5xx or 429
        if (isRetryable(response.status) && attempt < retries) {
          lastError = error;
          attempt++;
          await delay(getBackoffDelay(attempt));
          continue;
        }

        return err(error);
      }

      // Parse response based on type
      let data: T;
      switch (responseType) {
        case 'text':
          data = (await response.text()) as T;
          break;
        case 'blob':
          data = (await response.blob()) as T;
          break;
        case 'stream':
          data = response.body as T;
          break;
        case 'json':
        default:
          // Handle empty responses
          const text = await response.text();
          if (!text) {
            data = null as T;
          } else {
            const parsed = JSON.parse(text);
            // Check if it's an error envelope (some APIs return 200 with error body)
            if (isErrorEnvelope(parsed)) {
              return err(errorFromResponse(parsed.error.status ?? 500, parsed));
            }
            data = parsed as T;
          }
          break;
      }

      return ok(data);
    } catch (e) {
      clearTimeout(timeoutId);

      // Handle abort
      if (e instanceof DOMException && e.name === 'AbortError') {
        if (signal?.aborted) {
          return err(new AbortError());
        }
        return err(new TimeoutError());
      }

      // Handle network errors
      if (e instanceof TypeError || (e instanceof Error && e.message.includes('fetch'))) {
        const networkError = new NetworkError(e.message, { cause: e as Error });

        // Retry on network errors
        if (attempt < retries) {
          lastError = networkError;
          attempt++;
          await delay(getBackoffDelay(attempt));
          continue;
        }

        return err(networkError);
      }

      // Rethrow unexpected errors
      throw e;
    }
  }

  // Should not reach here, but return last error if we do
  return err(lastError ?? new NetworkError('Request failed'));
}

/**
 * Helper for GET requests.
 */
export function get<T>(
  ctx: RequestContext,
  path: string,
  options?: Omit<RequestOptions, 'method' | 'body'>
): Promise<Result<T, ReactorError>> {
  return request<T>(ctx, path, { ...options, method: 'GET' });
}

/**
 * Helper for POST requests.
 */
export function post<T>(
  ctx: RequestContext,
  path: string,
  body?: unknown,
  options?: Omit<RequestOptions, 'method' | 'body'>
): Promise<Result<T, ReactorError>> {
  return request<T>(ctx, path, { ...options, method: 'POST', body });
}

/**
 * Helper for PUT requests.
 */
export function put<T>(
  ctx: RequestContext,
  path: string,
  body?: unknown,
  options?: Omit<RequestOptions, 'method' | 'body'>
): Promise<Result<T, ReactorError>> {
  return request<T>(ctx, path, { ...options, method: 'PUT', body });
}

/**
 * Helper for PATCH requests.
 */
export function patch<T>(
  ctx: RequestContext,
  path: string,
  body?: unknown,
  options?: Omit<RequestOptions, 'method' | 'body'>
): Promise<Result<T, ReactorError>> {
  return request<T>(ctx, path, { ...options, method: 'PATCH', body });
}

/**
 * Helper for DELETE requests.
 */
export function del<T>(
  ctx: RequestContext,
  path: string,
  options?: Omit<RequestOptions, 'method'>
): Promise<Result<T, ReactorError>> {
  return request<T>(ctx, path, { ...options, method: 'DELETE' });
}
