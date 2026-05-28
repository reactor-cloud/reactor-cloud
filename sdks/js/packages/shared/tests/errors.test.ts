import { describe, it, expect } from 'vitest';
import {
  ReactorError,
  AuthError,
  ValidationError,
  NotFoundError,
  ConflictError,
  RateLimitError,
  ServerError,
  NetworkError,
  errorFromResponse,
  isErrorEnvelope,
} from '../src/errors.js';

describe('errors', () => {
  describe('ReactorError', () => {
    it('should create error with all properties', () => {
      const error = new ReactorError('Test error', 'test_code', 400, {
        hint: 'Test hint',
      });

      expect(error.message).toBe('Test error');
      expect(error.code).toBe('test_code');
      expect(error.statusCode).toBe(400);
      expect(error.hint).toBe('Test hint');
      expect(error.name).toBe('ReactorError');
      expect(error).toBeInstanceOf(Error);
    });

    it('should serialize to JSON', () => {
      const error = new ReactorError('Test', 'code', 400, { hint: 'Hint' });
      const json = error.toJSON();

      expect(json).toEqual({
        name: 'ReactorError',
        message: 'Test',
        code: 'code',
        statusCode: 400,
        hint: 'Hint',
      });
    });
  });

  describe('specific error types', () => {
    it('should create AuthError with 401 status', () => {
      const error = new AuthError('Unauthorized', 'invalid_token');
      expect(error.statusCode).toBe(401);
      expect(error.name).toBe('AuthError');
    });

    it('should create ValidationError with fields', () => {
      const error = new ValidationError('Invalid input', 'validation_error', {
        fields: { email: ['Invalid format'] },
      });
      expect(error.statusCode).toBe(400);
      expect(error.fields).toEqual({ email: ['Invalid format'] });
    });

    it('should create NotFoundError with 404 status', () => {
      const error = new NotFoundError('Resource not found');
      expect(error.statusCode).toBe(404);
      expect(error.code).toBe('not_found');
    });

    it('should create ConflictError with 409 status', () => {
      const error = new ConflictError('Already exists');
      expect(error.statusCode).toBe(409);
    });

    it('should create RateLimitError with retryAfter', () => {
      const error = new RateLimitError('Too many requests', 'rate_limited', {
        retryAfter: 60,
      });
      expect(error.statusCode).toBe(429);
      expect(error.retryAfter).toBe(60);
    });

    it('should create ServerError with 500 status', () => {
      const error = new ServerError('Internal error');
      expect(error.statusCode).toBe(500);
    });

    it('should create NetworkError with 0 status', () => {
      const error = new NetworkError('Connection failed');
      expect(error.statusCode).toBe(0);
      expect(error.code).toBe('network_error');
    });
  });

  describe('isErrorEnvelope', () => {
    it('should return true for valid error envelope', () => {
      expect(
        isErrorEnvelope({
          error: { code: 'test', message: 'Test error' },
        })
      ).toBe(true);
    });

    it('should return false for invalid envelopes', () => {
      expect(isErrorEnvelope(null)).toBe(false);
      expect(isErrorEnvelope({})).toBe(false);
      expect(isErrorEnvelope({ error: null })).toBe(false);
      expect(isErrorEnvelope({ error: { code: 'test' } })).toBe(false);
      expect(isErrorEnvelope('string')).toBe(false);
    });
  });

  describe('errorFromResponse', () => {
    it('should create ValidationError for 400', () => {
      const error = errorFromResponse(400, {
        error: { code: 'invalid_input', message: 'Bad request' },
      });
      expect(error).toBeInstanceOf(ValidationError);
      expect(error.code).toBe('invalid_input');
    });

    it('should create AuthError for 401', () => {
      const error = errorFromResponse(401, {
        error: { code: 'unauthorized', message: 'Not authenticated' },
      });
      expect(error).toBeInstanceOf(AuthError);
    });

    it('should create NotFoundError for 404', () => {
      const error = errorFromResponse(404, {
        error: { code: 'not_found', message: 'Not found' },
      });
      expect(error).toBeInstanceOf(NotFoundError);
    });

    it('should create ConflictError for 409', () => {
      const error = errorFromResponse(409, {
        error: { code: 'conflict', message: 'Conflict' },
      });
      expect(error).toBeInstanceOf(ConflictError);
    });

    it('should create RateLimitError for 429', () => {
      const error = errorFromResponse(429, {
        error: { code: 'rate_limited', message: 'Too many requests' },
      });
      expect(error).toBeInstanceOf(RateLimitError);
    });

    it('should create ServerError for 5xx', () => {
      const error = errorFromResponse(500, {
        error: { code: 'server_error', message: 'Internal error' },
      });
      expect(error).toBeInstanceOf(ServerError);
    });

    it('should handle string body', () => {
      const error = errorFromResponse(400, 'Plain text error');
      expect(error.message).toBe('Plain text error');
      expect(error.code).toBe('unknown');
    });

    it('should handle unknown body shape', () => {
      const error = errorFromResponse(400, { unexpected: 'shape' });
      expect(error.message).toBe('An unknown error occurred');
    });
  });
});
