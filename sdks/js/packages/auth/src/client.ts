import {
  type RequestContext,
  type Result,
  type User,
  type Session,
  type AuthStateSubscription,
  detectStorageAdapter,
  get,
  post,
  patch,
  del,
  ok,
  err,
  AuthError,
} from '@reactor/shared';

import {
  type AuthClientOptions,
  type SignUpParams,
  type SignInParams,
  type UpdateUserParams,
  type RequestPasswordResetParams,
  type ConfirmPasswordResetParams,
  type ResendVerificationParams,
  type AuthStateChangeCallback,
  type SignUpResponse,
  type LoginResponse,
  type TokenResponse,
} from './types.js';

import { AuthStateManager } from './state.js';
import { detectAndClean, type DetectedToken } from './url-detect.js';
import { OrgsClient } from './orgs.js';
import { PermissionsClient } from './permissions.js';
import { ApiKeysClient } from './api-keys.js';

/**
 * Authentication client for Reactor.
 *
 * Handles user authentication, session management, token refresh, and multi-tab sync.
 */
export class AuthClient {
  private stateManager: AuthStateManager;
  private initialized = false;
  private initPromise: Promise<void> | null = null;

  /** Organizations client */
  readonly orgs: OrgsClient;
  /** Permissions client */
  readonly permissions: PermissionsClient;
  /** API keys client */
  readonly apiKeys: ApiKeysClient;

  constructor(
    private ctx: RequestContext,
    options: AuthClientOptions = {}
  ) {
    const {
      storage = detectStorageAdapter(),
      storageKey = 'reactor.session',
      autoRefresh = true,
      persistSession = true,
      detectSessionInUrl = true,
    } = options;

    this.stateManager = new AuthStateManager(
      persistSession ? storage : null,
      storageKey,
      autoRefresh,
      persistSession
    );

    // Set up refresh callback
    this.stateManager.setRefreshCallback(async () => {
      const session = this.stateManager.getSession();
      if (!session?.refresh_token) {
        return null;
      }

      const result = await this.refreshSessionInternal(session.refresh_token);
      if (result.error) {
        return null;
      }

      return result.data;
    });

    // Initialize sub-clients
    this.orgs = new OrgsClient(ctx);
    this.permissions = new PermissionsClient(ctx);
    this.apiKeys = new ApiKeysClient(ctx);

    // Auto-initialize if in browser
    if (typeof window !== 'undefined' && detectSessionInUrl) {
      this.initPromise = this.initialize(detectSessionInUrl);
    }
  }

  /**
   * Initialize the auth client.
   * Loads session from storage and handles URL tokens.
   */
  async initialize(detectUrl = true): Promise<void> {
    if (this.initialized) {
      return;
    }

    if (this.initPromise) {
      await this.initPromise;
      return;
    }

    this.initPromise = this.doInitialize(detectUrl);
    await this.initPromise;
  }

  private async doInitialize(detectUrl: boolean): Promise<void> {
    // Load from storage first
    await this.stateManager.initialize();

    // Handle URL tokens
    if (detectUrl) {
      const detected = detectAndClean(true);
      if (detected) {
        await this.handleDetectedToken(detected);
      }
    }

    this.initialized = true;
  }

  private async handleDetectedToken(detected: DetectedToken): Promise<void> {
    switch (detected.type) {
      case 'verify':
        // Auto-verify email
        await this.verifyEmail({ token: detected.token });
        break;

      case 'password_reset':
        // Store token for later use - don't auto-process
        // User needs to provide new password
        break;

      case 'oauth':
        if (detected.params) {
          // Build session from OAuth tokens
          const { access_token, refresh_token, expires_at } = detected.params;
          if (access_token && refresh_token) {
            // Fetch user info
            const userResult = await get<User>(
              { ...this.ctx, getAccessToken: () => access_token },
              '/auth/v1/user'
            );

            if (!userResult.error && userResult.data) {
              const session: Session = {
                access_token,
                refresh_token,
                expires_at: expires_at || new Date(Date.now() + 3600 * 1000).toISOString(),
                user: userResult.data,
              };
              await this.stateManager.setSession(session, 'SIGNED_IN');
            }
          }
        }
        break;

      case 'invite':
        // Store for later - user may need to sign in first
        break;
    }
  }

  /**
   * Sign up a new user.
   */
  async signUp(params: SignUpParams): Promise<Result<{ user: User; session: Session }>> {
    const result = await post<SignUpResponse>(this.ctx, '/auth/v1/signup', {
      email: params.email,
      password: params.password,
      metadata: params.metadata ?? {},
    });

    if (result.error) {
      return result;
    }

    const { user, session: sessionData } = result.data;
    const session: Session = {
      access_token: sessionData.access_token,
      refresh_token: sessionData.refresh_token,
      expires_at: sessionData.expires_at,
      user,
    };

    await this.stateManager.setSession(session, 'SIGNED_IN');

    return ok({ user, session });
  }

  /**
   * Sign in with email and password.
   */
  async signIn(params: SignInParams): Promise<Result<{ user: User; session: Session }>> {
    const result = await post<LoginResponse>(this.ctx, '/auth/v1/login', {
      email: params.email,
      password: params.password,
    });

    if (result.error) {
      return result;
    }

    const { user, access_token, refresh_token, expires_at } = result.data;
    const session: Session = {
      access_token,
      refresh_token,
      expires_at,
      user,
    };

    await this.stateManager.setSession(session, 'SIGNED_IN');

    return ok({ user, session });
  }

  /**
   * Sign out the current user.
   * Revokes the refresh token server-side.
   */
  async signOut(): Promise<Result<void>> {
    const session = this.stateManager.getSession();

    if (session) {
      // Best-effort server logout
      await post(this.ctx, '/auth/v1/logout', {
        refresh_token: session.refresh_token,
      }).catch(() => {});
    }

    await this.stateManager.clearSession();

    return ok(undefined);
  }

  /**
   * Get the current session.
   * Refreshes automatically if near expiry.
   */
  async getSession(): Promise<Session | null> {
    await this.initialize();
    return this.stateManager.getSession();
  }

  /**
   * Get the current user.
   * Fetches from server if not cached.
   */
  async getUser(): Promise<Result<User | null>> {
    await this.initialize();

    const session = this.stateManager.getSession();
    if (!session) {
      return ok(null);
    }

    // Return cached user if available
    const cachedUser = this.stateManager.getUser();
    if (cachedUser) {
      return ok(cachedUser);
    }

    // Fetch from server
    const result = await get<User>(this.ctx, '/auth/v1/user');
    if (result.error) {
      return result;
    }

    return ok(result.data);
  }

  /**
   * Update the current user's profile.
   */
  async updateUser(params: UpdateUserParams): Promise<Result<User>> {
    const result = await patch<User>(this.ctx, '/auth/v1/user', params);

    if (!result.error && result.data) {
      // Update cached user
      const session = this.stateManager.getSession();
      if (session) {
        const updatedSession: Session = {
          ...session,
          user: result.data,
        };
        await this.stateManager.setSession(updatedSession, 'USER_UPDATED');
      }
    }

    return result;
  }

  /**
   * Verify an email address with a token.
   */
  async verifyEmail(params: { token: string }): Promise<Result<{ verified: boolean }>> {
    const result = await get<{ verified: boolean }>(
      this.ctx,
      `/auth/v1/verify?token=${encodeURIComponent(params.token)}`
    );

    if (!result.error) {
      // Refresh user to get updated email_verified status
      const session = this.stateManager.getSession();
      if (session) {
        const userResult = await get<User>(this.ctx, '/auth/v1/user');
        if (!userResult.error && userResult.data) {
          const updatedSession: Session = {
            ...session,
            user: userResult.data,
          };
          await this.stateManager.setSession(updatedSession, 'USER_UPDATED');
        }
      }
    }

    return result;
  }

  /**
   * Resend email verification.
   */
  async resendVerification(params: ResendVerificationParams): Promise<Result<void>> {
    return post<void>(this.ctx, '/auth/v1/resend', { email: params.email });
  }

  /**
   * Request a password reset email.
   */
  async requestPasswordReset(params: RequestPasswordResetParams): Promise<Result<void>> {
    return post<void>(this.ctx, '/auth/v1/password-reset/request', {
      email: params.email,
    });
  }

  /**
   * Confirm a password reset with the token and new password.
   */
  async confirmPasswordReset(params: ConfirmPasswordResetParams): Promise<Result<void>> {
    return post<void>(this.ctx, '/auth/v1/password-reset/confirm', {
      token: params.token,
      password: params.newPassword,
    });
  }

  /**
   * Manually refresh the session.
   */
  async refreshSession(): Promise<Result<Session>> {
    const session = await this.stateManager.refresh();
    if (!session) {
      return err(new AuthError('Failed to refresh session', 'refresh_failed'));
    }
    return ok(session);
  }

  private async refreshSessionInternal(refreshToken: string): Promise<Result<Session>> {
    const result = await post<TokenResponse>(this.ctx, '/auth/v1/token', {
      grant_type: 'refresh_token',
      refresh_token: refreshToken,
    });

    if (result.error) {
      return result as Result<Session>;
    }

    const { access_token, refresh_token, expires_at } = result.data;

    // Fetch updated user
    const userResult = await get<User>(
      { ...this.ctx, getAccessToken: () => access_token },
      '/auth/v1/user'
    );

    if (userResult.error) {
      return userResult as Result<Session>;
    }

    const session: Session = {
      access_token,
      refresh_token,
      expires_at,
      user: userResult.data,
    };

    return ok(session);
  }

  /**
   * Set the session manually (for SSR scenarios).
   */
  async setSession(session: Session): Promise<void> {
    await this.stateManager.setSession(session, 'SIGNED_IN');
  }

  /**
   * Subscribe to auth state changes.
   */
  onAuthStateChange(callback: AuthStateChangeCallback): AuthStateSubscription {
    return this.stateManager.onAuthStateChange(callback);
  }

  /**
   * Get the access token for making authenticated requests.
   * Used internally by the request context.
   */
  getAccessToken(): string | null {
    return this.stateManager.getSession()?.access_token ?? null;
  }

  /**
   * Delete the current user's account.
   */
  async deleteUser(): Promise<Result<void>> {
    const result = await del<void>(this.ctx, '/auth/v1/user');

    if (!result.error) {
      await this.stateManager.clearSession();
    }

    return result;
  }
}
