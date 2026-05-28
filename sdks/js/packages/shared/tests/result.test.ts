import { describe, it, expect } from 'vitest';
import { ok, err, withThrowOnError, createResultPromise } from '../src/result.js';
import { ReactorError, ValidationError } from '../src/errors.js';

describe('result', () => {
  describe('ok', () => {
    it('should create success result', () => {
      const result = ok({ id: 1, name: 'test' });

      expect(result.data).toEqual({ id: 1, name: 'test' });
      expect(result.error).toBeNull();
    });

    it('should handle null data', () => {
      const result = ok(null);

      expect(result.data).toBeNull();
      expect(result.error).toBeNull();
    });
  });

  describe('err', () => {
    it('should create error result', () => {
      const error = new ValidationError('Invalid input', 'invalid');
      const result = err(error);

      expect(result.data).toBeNull();
      expect(result.error).toBe(error);
    });
  });

  describe('withThrowOnError', () => {
    it('should return data on success', async () => {
      const promise = Promise.resolve(ok({ id: 1 }));
      const enhanced = withThrowOnError(promise);

      const data = await enhanced.throwOnError();

      expect(data).toEqual({ id: 1 });
    });

    it('should throw on error', async () => {
      const error = new ValidationError('Invalid', 'invalid');
      const promise = Promise.resolve(err(error));
      const enhanced = withThrowOnError(promise);

      await expect(enhanced.throwOnError()).rejects.toThrow(error);
    });

    it('should work as normal promise', async () => {
      const promise = Promise.resolve(ok({ id: 1 }));
      const enhanced = withThrowOnError(promise);

      const result = await enhanced;

      expect(result.data).toEqual({ id: 1 });
    });
  });

  describe('createResultPromise', () => {
    it('should wrap successful async operation', async () => {
      const result = await createResultPromise(async () => {
        return { id: 1 };
      });

      expect(result.data).toEqual({ id: 1 });
      expect(result.error).toBeNull();
    });

    it('should catch ReactorError', async () => {
      const error = new ValidationError('Invalid', 'invalid');
      const result = await createResultPromise(async () => {
        throw error;
      });

      expect(result.data).toBeNull();
      expect(result.error).toBe(error);
    });

    it('should use error handler for non-ReactorError', async () => {
      const result = await createResultPromise(
        async () => {
          throw new Error('Generic error');
        },
        (e) => new ReactorError((e as Error).message, 'unknown', 500)
      );

      expect(result.data).toBeNull();
      expect(result.error?.message).toBe('Generic error');
    });

    it('should have throwOnError method', async () => {
      const promise = createResultPromise(async () => ({ id: 1 }));

      const data = await promise.throwOnError();

      expect(data).toEqual({ id: 1 });
    });
  });
});
