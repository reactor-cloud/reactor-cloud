/**
 * Token types that can be detected from URLs.
 */
export type DetectedTokenType = 'verify' | 'password_reset' | 'oauth' | 'invite';

/**
 * Result of URL token detection.
 */
export interface DetectedToken {
  type: DetectedTokenType;
  token: string;
  /** Additional parameters from the URL */
  params?: Record<string, string>;
}

/**
 * Detect session or verification tokens from URL.
 *
 * Supports:
 * - Query parameter: ?token=... (email verification, password reset)
 * - Query parameter: ?invite_token=... (invitation acceptance)
 * - Hash fragment: #access_token=...&refresh_token=... (OAuth)
 */
export function detectSessionInUrl(): DetectedToken | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const url = new URL(window.location.href);

  // Check query parameters
  const token = url.searchParams.get('token');
  const tokenType = url.searchParams.get('type');
  const inviteToken = url.searchParams.get('invite_token');

  if (token) {
    if (tokenType === 'password_reset') {
      return { type: 'password_reset', token };
    }
    return { type: 'verify', token };
  }

  if (inviteToken) {
    return { type: 'invite', token: inviteToken };
  }

  // Check hash fragment for OAuth
  const hash = url.hash.substring(1);
  if (hash) {
    const hashParams = new URLSearchParams(hash);
    const accessToken = hashParams.get('access_token');
    const refreshToken = hashParams.get('refresh_token');

    if (accessToken && refreshToken) {
      return {
        type: 'oauth',
        token: accessToken,
        params: {
          access_token: accessToken,
          refresh_token: refreshToken,
          expires_at: hashParams.get('expires_at') ?? '',
        },
      };
    }
  }

  return null;
}

/**
 * Clean detected tokens from the current URL.
 * Updates browser history without reloading.
 */
export function cleanUrlAfterDetection(): void {
  if (typeof window === 'undefined') {
    return;
  }

  const url = new URL(window.location.href);
  let modified = false;

  // Remove query params
  const paramsToRemove = ['token', 'type', 'invite_token'];
  for (const param of paramsToRemove) {
    if (url.searchParams.has(param)) {
      url.searchParams.delete(param);
      modified = true;
    }
  }

  // Remove hash if it looks like OAuth tokens
  if (url.hash) {
    const hashParams = new URLSearchParams(url.hash.substring(1));
    if (hashParams.has('access_token') || hashParams.has('refresh_token')) {
      url.hash = '';
      modified = true;
    }
  }

  if (modified) {
    window.history.replaceState(null, '', url.toString());
  }
}

/**
 * Detect and return token info, optionally cleaning the URL.
 */
export function detectAndClean(cleanUrl: boolean = true): DetectedToken | null {
  const detected = detectSessionInUrl();

  if (detected && cleanUrl) {
    cleanUrlAfterDetection();
  }

  return detected;
}
