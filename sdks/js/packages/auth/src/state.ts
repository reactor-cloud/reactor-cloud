import {
  type StorageAdapter,
  type Session,
  type User,
  type AuthStateEvent,
  isJwtExpired,
  getJwtTimeRemaining,
} from '@reactor/shared';

import type { AuthState, AuthStateChangeCallback } from './types.js';

const STORAGE_KEY_DEFAULT = 'reactor.session';
const REFRESH_MARGIN_SECONDS = 60;

/**
 * Manages auth state including session storage, token refresh, and multi-tab sync.
 */
export class AuthStateManager {
  private state: AuthState = { session: null, user: null };
  private listeners: Set<AuthStateChangeCallback> = new Set();
  private refreshTimer: ReturnType<typeof setTimeout> | null = null;
  private refreshPromise: Promise<void> | null = null;
  private storageListenerBound = false;

  constructor(
    private storage: StorageAdapter | null,
    private storageKey: string = STORAGE_KEY_DEFAULT,
    private autoRefresh: boolean = true,
    private persistSession: boolean = true,
    private onRefresh?: () => Promise<Session | null>
  ) {}

  /**
   * Initialize state from storage.
   */
  async initialize(): Promise<Session | null> {
    if (!this.persistSession || !this.storage) {
      return null;
    }

    try {
      const stored = await this.storage.getItem(this.storageKey);
      if (!stored) {
        return null;
      }

      const session = JSON.parse(stored) as Session;

      // Check if token is expired
      if (isJwtExpired(session.access_token)) {
        // Try to refresh
        if (this.onRefresh) {
          const refreshed = await this.onRefresh();
          if (refreshed) {
            await this.setSession(refreshed, 'INITIAL_SESSION');
            return refreshed;
          }
        }
        // Clear expired session
        await this.clearSession();
        return null;
      }

      await this.setSession(session, 'INITIAL_SESSION');
      return session;
    } catch {
      return null;
    }
  }

  /**
   * Get current session.
   */
  getSession(): Session | null {
    return this.state.session;
  }

  /**
   * Get current user.
   */
  getUser(): User | null {
    return this.state.user;
  }

  /**
   * Set the current session.
   */
  async setSession(session: Session | null, event: AuthStateEvent): Promise<void> {
    this.state.session = session;
    this.state.user = session?.user ?? null;

    // Persist to storage
    if (this.persistSession && this.storage) {
      if (session) {
        await this.storage.setItem(this.storageKey, JSON.stringify(session));
      } else {
        await this.storage.removeItem(this.storageKey);
      }
    }

    // Schedule refresh
    if (session && this.autoRefresh) {
      this.scheduleRefresh(session.access_token);
    } else {
      this.cancelRefresh();
    }

    // Setup multi-tab sync
    if (session && !this.storageListenerBound) {
      this.setupStorageListener();
    }

    // Notify listeners
    this.notifyListeners(event, session);
  }

  /**
   * Clear the current session.
   */
  async clearSession(): Promise<void> {
    await this.setSession(null, 'SIGNED_OUT');
  }

  /**
   * Subscribe to auth state changes.
   */
  onAuthStateChange(callback: AuthStateChangeCallback): { unsubscribe: () => void } {
    this.listeners.add(callback);

    // Emit initial state
    if (this.state.session) {
      callback('INITIAL_SESSION', this.state.session);
    }

    return {
      unsubscribe: () => {
        this.listeners.delete(callback);
      },
    };
  }

  /**
   * Set the refresh callback.
   */
  setRefreshCallback(onRefresh: () => Promise<Session | null>): void {
    this.onRefresh = onRefresh;

    // If we have a session, schedule refresh
    if (this.state.session && this.autoRefresh) {
      this.scheduleRefresh(this.state.session.access_token);
    }
  }

  /**
   * Manually trigger a refresh.
   */
  async refresh(): Promise<Session | null> {
    if (!this.onRefresh) {
      return null;
    }

    // Deduplicate concurrent refresh calls
    if (this.refreshPromise) {
      await this.refreshPromise;
      return this.state.session;
    }

    this.refreshPromise = this.doRefresh();

    try {
      await this.refreshPromise;
      return this.state.session;
    } finally {
      this.refreshPromise = null;
    }
  }

  private async doRefresh(): Promise<void> {
    if (!this.onRefresh) {
      return;
    }

    try {
      const session = await this.onRefresh();
      if (session) {
        await this.setSession(session, 'TOKEN_REFRESHED');
      } else {
        await this.clearSession();
      }
    } catch {
      await this.clearSession();
    }
  }

  private scheduleRefresh(accessToken: string): void {
    this.cancelRefresh();

    const remaining = getJwtTimeRemaining(accessToken);
    if (remaining === null || remaining <= 0) {
      // Token already expired, refresh now
      this.refresh();
      return;
    }

    // Schedule refresh at exp - REFRESH_MARGIN_SECONDS
    const delay = Math.max((remaining - REFRESH_MARGIN_SECONDS) * 1000, 0);

    this.refreshTimer = setTimeout(() => {
      this.refresh();
    }, delay);
  }

  private cancelRefresh(): void {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }
  }

  private setupStorageListener(): void {
    if (typeof window === 'undefined' || this.storageListenerBound) {
      return;
    }

    this.storageListenerBound = true;

    window.addEventListener('storage', (event) => {
      if (event.key !== this.storageKey) {
        return;
      }

      if (event.newValue === null) {
        // Session cleared in another tab
        this.state.session = null;
        this.state.user = null;
        this.cancelRefresh();
        this.notifyListeners('SIGNED_OUT', null);
      } else {
        try {
          const session = JSON.parse(event.newValue) as Session;
          this.state.session = session;
          this.state.user = session.user;

          // Reschedule refresh with new token
          if (this.autoRefresh) {
            this.scheduleRefresh(session.access_token);
          }

          this.notifyListeners('TOKEN_REFRESHED', session);
        } catch {
          // Ignore parse errors
        }
      }
    });
  }

  private notifyListeners(event: AuthStateEvent, session: Session | null): void {
    for (const listener of this.listeners) {
      try {
        listener(event, session);
      } catch {
        // Ignore listener errors
      }
    }
  }

  /**
   * Cleanup resources.
   */
  destroy(): void {
    this.cancelRefresh();
    this.listeners.clear();
  }
}
