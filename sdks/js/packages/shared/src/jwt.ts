/**
 * Decoded JWT payload structure for Reactor tokens.
 */
export interface JWTPayload {
  /** Subject (user ID) */
  sub: string;
  /** Email address */
  email?: string;
  /** Whether email is verified */
  email_verified?: boolean;
  /** Expiration timestamp (seconds since epoch) */
  exp: number;
  /** Issued at timestamp (seconds since epoch) */
  iat: number;
  /** Issuer */
  iss?: string;
  /** Audience */
  aud?: string | string[];
  /** Organization memberships */
  orgs?: Array<{
    id: string;
    slug: string;
    role_id: string;
    permissions: string[];
  }>;
  /** User metadata */
  metadata?: Record<string, unknown>;
  /** Any additional claims */
  [key: string]: unknown;
}

/**
 * Decode a JWT without verifying the signature.
 * For client-side use only - server should always verify.
 *
 * @param token - JWT string
 * @returns Decoded payload or null if invalid
 */
export function decodeJwt(token: string): JWTPayload | null {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) {
      return null;
    }

    const payload = parts[1];
    if (!payload) {
      return null;
    }

    // Base64url decode
    const base64 = payload.replace(/-/g, '+').replace(/_/g, '/');
    const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4);

    let jsonString: string;
    if (typeof atob === 'function') {
      // Browser
      jsonString = atob(padded);
    } else if (typeof Buffer !== 'undefined') {
      // Node.js
      jsonString = Buffer.from(padded, 'base64').toString('utf-8');
    } else {
      return null;
    }

    return JSON.parse(jsonString) as JWTPayload;
  } catch {
    return null;
  }
}

/**
 * Check if a JWT is expired.
 *
 * @param token - JWT string or decoded payload
 * @param bufferSeconds - Seconds before actual expiry to consider it expired (default: 0)
 * @returns True if expired or will expire within buffer
 */
export function isJwtExpired(token: string | JWTPayload, bufferSeconds: number = 0): boolean {
  const payload = typeof token === 'string' ? decodeJwt(token) : token;
  if (!payload || typeof payload.exp !== 'number') {
    return true;
  }

  const now = Math.floor(Date.now() / 1000);
  return payload.exp <= now + bufferSeconds;
}

/**
 * Get the expiration date of a JWT.
 *
 * @param token - JWT string or decoded payload
 * @returns Date object or null if invalid
 */
export function getJwtExpiry(token: string | JWTPayload): Date | null {
  const payload = typeof token === 'string' ? decodeJwt(token) : token;
  if (!payload || typeof payload.exp !== 'number') {
    return null;
  }

  return new Date(payload.exp * 1000);
}

/**
 * Get seconds until JWT expiration.
 *
 * @param token - JWT string or decoded payload
 * @returns Seconds remaining (negative if expired) or null if invalid
 */
export function getJwtTimeRemaining(token: string | JWTPayload): number | null {
  const payload = typeof token === 'string' ? decodeJwt(token) : token;
  if (!payload || typeof payload.exp !== 'number') {
    return null;
  }

  const now = Math.floor(Date.now() / 1000);
  return payload.exp - now;
}
