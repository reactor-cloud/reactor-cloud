/**
 * Storage adapter interface for session persistence.
 * Supports both sync and async implementations.
 */
export interface StorageAdapter {
  getItem(key: string): string | null | Promise<string | null>;
  setItem(key: string, value: string): void | Promise<void>;
  removeItem(key: string): void | Promise<void>;
}

/**
 * In-memory storage adapter.
 * Useful for server-side rendering, testing, or when no persistent storage is available.
 */
export function memoryAdapter(): StorageAdapter {
  const store = new Map<string, string>();

  return {
    getItem(key: string): string | null {
      return store.get(key) ?? null;
    },
    setItem(key: string, value: string): void {
      store.set(key, value);
    },
    removeItem(key: string): void {
      store.delete(key);
    },
  };
}

/**
 * localStorage adapter for browsers.
 * Falls back gracefully if localStorage is unavailable.
 */
export function localStorageAdapter(): StorageAdapter {
  const isAvailable = (() => {
    try {
      if (typeof window === 'undefined' || typeof localStorage === 'undefined') {
        return false;
      }
      const test = '__reactor_test__';
      localStorage.setItem(test, test);
      localStorage.removeItem(test);
      return true;
    } catch {
      return false;
    }
  })();

  if (!isAvailable) {
    console.warn(
      '[@reactor/shared] localStorage is not available, falling back to memory storage'
    );
    return memoryAdapter();
  }

  return {
    getItem(key: string): string | null {
      try {
        return localStorage.getItem(key);
      } catch {
        return null;
      }
    },
    setItem(key: string, value: string): void {
      try {
        localStorage.setItem(key, value);
      } catch {
        // Storage full or blocked
      }
    },
    removeItem(key: string): void {
      try {
        localStorage.removeItem(key);
      } catch {
        // Ignore errors
      }
    },
  };
}

/**
 * sessionStorage adapter for browsers.
 * Data persists only for the session.
 */
export function sessionStorageAdapter(): StorageAdapter {
  const isAvailable = (() => {
    try {
      if (typeof window === 'undefined' || typeof sessionStorage === 'undefined') {
        return false;
      }
      const test = '__reactor_test__';
      sessionStorage.setItem(test, test);
      sessionStorage.removeItem(test);
      return true;
    } catch {
      return false;
    }
  })();

  if (!isAvailable) {
    console.warn(
      '[@reactor/shared] sessionStorage is not available, falling back to memory storage'
    );
    return memoryAdapter();
  }

  return {
    getItem(key: string): string | null {
      try {
        return sessionStorage.getItem(key);
      } catch {
        return null;
      }
    },
    setItem(key: string, value: string): void {
      try {
        sessionStorage.setItem(key, value);
      } catch {
        // Storage full or blocked
      }
    },
    removeItem(key: string): void {
      try {
        sessionStorage.removeItem(key);
      } catch {
        // Ignore errors
      }
    },
  };
}

/**
 * Cookie-based storage adapter.
 * Useful for SSR scenarios where cookies are accessible server-side.
 */
export function cookieAdapter(options?: {
  /** Cookie path (default: '/') */
  path?: string;
  /** Cookie domain */
  domain?: string;
  /** Secure flag (default: true in production) */
  secure?: boolean;
  /** SameSite attribute (default: 'lax') */
  sameSite?: 'strict' | 'lax' | 'none';
  /** Max age in seconds */
  maxAge?: number;
}): StorageAdapter {
  const {
    path = '/',
    domain,
    secure = typeof window !== 'undefined' && window.location.protocol === 'https:',
    sameSite = 'lax',
    maxAge = 60 * 60 * 24 * 30, // 30 days
  } = options ?? {};

  const isAvailable = typeof document !== 'undefined';

  if (!isAvailable) {
    console.warn('[@reactor/shared] document.cookie is not available, falling back to memory storage');
    return memoryAdapter();
  }

  return {
    getItem(key: string): string | null {
      try {
        const cookies = document.cookie.split(';');
        for (const cookie of cookies) {
          const [name, ...valueParts] = cookie.trim().split('=');
          if (name === key) {
            const value = valueParts.join('=');
            return decodeURIComponent(value);
          }
        }
        return null;
      } catch {
        return null;
      }
    },
    setItem(key: string, value: string): void {
      try {
        const parts = [
          `${key}=${encodeURIComponent(value)}`,
          `path=${path}`,
          `max-age=${maxAge}`,
          `samesite=${sameSite}`,
        ];
        if (domain) parts.push(`domain=${domain}`);
        if (secure) parts.push('secure');
        document.cookie = parts.join('; ');
      } catch {
        // Cookie setting failed
      }
    },
    removeItem(key: string): void {
      try {
        const parts = [
          `${key}=`,
          `path=${path}`,
          'max-age=0',
          'expires=Thu, 01 Jan 1970 00:00:00 GMT',
        ];
        if (domain) parts.push(`domain=${domain}`);
        document.cookie = parts.join('; ');
      } catch {
        // Ignore errors
      }
    },
  };
}

/**
 * Detect and return the best available storage adapter.
 */
export function detectStorageAdapter(): StorageAdapter {
  // Try localStorage first (browser)
  if (typeof window !== 'undefined' && typeof localStorage !== 'undefined') {
    try {
      const test = '__reactor_test__';
      localStorage.setItem(test, test);
      localStorage.removeItem(test);
      return localStorageAdapter();
    } catch {
      // localStorage not available
    }
  }

  // Fall back to memory
  return memoryAdapter();
}
