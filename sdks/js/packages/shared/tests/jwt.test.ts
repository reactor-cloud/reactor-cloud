import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { decodeJwt, isJwtExpired, getJwtExpiry, getJwtTimeRemaining } from '../src/jwt.js';

describe('jwt', () => {
  const createToken = (payload: Record<string, unknown>): string => {
    const header = { alg: 'HS256', typ: 'JWT' };
    const encode = (obj: unknown) =>
      Buffer.from(JSON.stringify(obj)).toString('base64url');
    return `${encode(header)}.${encode(payload)}.signature`;
  };

  describe('decodeJwt', () => {
    it('should decode a valid JWT', () => {
      const payload = {
        sub: 'user-123',
        email: 'test@example.com',
        exp: 1700000000,
        iat: 1699996400,
      };
      const token = createToken(payload);

      const decoded = decodeJwt(token);

      expect(decoded).toEqual(payload);
    });

    it('should decode JWT with org claims', () => {
      const payload = {
        sub: 'user-123',
        exp: 1700000000,
        iat: 1699996400,
        orgs: [
          { id: 'org-1', slug: 'acme', role_id: 'admin', permissions: ['*'] },
        ],
      };
      const token = createToken(payload);

      const decoded = decodeJwt(token);

      expect(decoded?.orgs).toHaveLength(1);
      expect(decoded?.orgs?.[0].slug).toBe('acme');
    });

    it('should return null for invalid token format', () => {
      expect(decodeJwt('invalid')).toBeNull();
      expect(decodeJwt('only.two')).toBeNull();
      expect(decodeJwt('')).toBeNull();
    });

    it('should return null for invalid base64', () => {
      expect(decodeJwt('a.!!!invalid!!!.c')).toBeNull();
    });

    it('should return null for invalid JSON', () => {
      const invalidBase64 = Buffer.from('not json').toString('base64url');
      expect(decodeJwt(`a.${invalidBase64}.c`)).toBeNull();
    });
  });

  describe('isJwtExpired', () => {
    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date('2023-11-14T12:00:00Z'));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('should return false for non-expired token', () => {
      const futureExp = Math.floor(Date.now() / 1000) + 3600; // 1 hour from now
      const token = createToken({ exp: futureExp, sub: 'user' });

      expect(isJwtExpired(token)).toBe(false);
    });

    it('should return true for expired token', () => {
      const pastExp = Math.floor(Date.now() / 1000) - 3600; // 1 hour ago
      const token = createToken({ exp: pastExp, sub: 'user' });

      expect(isJwtExpired(token)).toBe(true);
    });

    it('should consider buffer time', () => {
      const exp = Math.floor(Date.now() / 1000) + 30; // 30 seconds from now
      const token = createToken({ exp, sub: 'user' });

      expect(isJwtExpired(token, 0)).toBe(false);
      expect(isJwtExpired(token, 60)).toBe(true); // Will expire within 60s buffer
    });

    it('should accept decoded payload', () => {
      const futureExp = Math.floor(Date.now() / 1000) + 3600;
      expect(isJwtExpired({ exp: futureExp, sub: 'user', iat: 0 })).toBe(false);
    });

    it('should return true for invalid token', () => {
      expect(isJwtExpired('invalid')).toBe(true);
    });
  });

  describe('getJwtExpiry', () => {
    it('should return expiration date', () => {
      const exp = 1700000000;
      const token = createToken({ exp, sub: 'user', iat: 0 });

      const expiry = getJwtExpiry(token);

      expect(expiry).toEqual(new Date(exp * 1000));
    });

    it('should return null for invalid token', () => {
      expect(getJwtExpiry('invalid')).toBeNull();
    });

    it('should accept decoded payload', () => {
      const exp = 1700000000;
      const expiry = getJwtExpiry({ exp, sub: 'user', iat: 0 });

      expect(expiry).toEqual(new Date(exp * 1000));
    });
  });

  describe('getJwtTimeRemaining', () => {
    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date('2023-11-14T12:00:00Z'));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('should return positive seconds for non-expired token', () => {
      const exp = Math.floor(Date.now() / 1000) + 3600;
      const token = createToken({ exp, sub: 'user', iat: 0 });

      const remaining = getJwtTimeRemaining(token);

      expect(remaining).toBe(3600);
    });

    it('should return negative seconds for expired token', () => {
      const exp = Math.floor(Date.now() / 1000) - 3600;
      const token = createToken({ exp, sub: 'user', iat: 0 });

      const remaining = getJwtTimeRemaining(token);

      expect(remaining).toBe(-3600);
    });

    it('should return null for invalid token', () => {
      expect(getJwtTimeRemaining('invalid')).toBeNull();
    });
  });
});
