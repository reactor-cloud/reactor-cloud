import { ReactorError } from './errors.js';

/**
 * Result type for SDK operations.
 * Always contains either data or error, never both.
 */
export type Result<T, E = ReactorError> =
  | { data: T; error: null }
  | { data: null; error: E };

/**
 * Create a successful result.
 */
export function ok<T>(data: T): Result<T> {
  return { data, error: null };
}

/**
 * Create an error result.
 */
export function err<E extends ReactorError>(error: E): Result<never, E> {
  return { data: null, error };
}

/**
 * Mixin for adding throwOnError to promises.
 */
export interface ThrowOnError<T> {
  /**
   * Throws if the result contains an error, otherwise returns the data.
   * Useful for `await reactor.from('posts').select('*').throwOnError()` pattern.
   */
  throwOnError(): Promise<T>;
}

/**
 * A promise that resolves to a Result, with a throwOnError method.
 */
export type ResultPromise<T, E = ReactorError> = Promise<Result<T, E>> & ThrowOnError<T>;

/**
 * Wrap a promise returning a Result with the throwOnError mixin.
 */
export function withThrowOnError<T, E extends ReactorError>(
  promise: Promise<Result<T, E>>
): ResultPromise<T, E> {
  const enhanced = promise as ResultPromise<T, E>;

  enhanced.throwOnError = async (): Promise<T> => {
    const result = await promise;
    if (result.error) {
      throw result.error;
    }
    return result.data as T;
  };

  return enhanced;
}

/**
 * Helper to create a ResultPromise from an async operation.
 */
export function createResultPromise<T>(
  operation: () => Promise<T>,
  errorHandler?: (e: unknown) => ReactorError
): ResultPromise<T> {
  const promise = (async (): Promise<Result<T>> => {
    try {
      const data = await operation();
      return ok(data);
    } catch (e) {
      if (e instanceof ReactorError) {
        return err(e);
      }
      if (errorHandler) {
        return err(errorHandler(e));
      }
      throw e;
    }
  })();

  return withThrowOnError(promise);
}
