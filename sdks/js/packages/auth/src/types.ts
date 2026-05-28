import type {
  User,
  Session,
  Organization,
  Member,
  Role,
  Invitation,
  ApiKey,
  AuthStateEvent,
  StorageAdapter,
} from '@reactor/shared';

export type { User, Session, Organization, Member, Role, Invitation, ApiKey, AuthStateEvent };

/**
 * Configuration options for the auth client.
 */
export interface AuthClientOptions {
  /** Storage adapter for session persistence */
  storage?: StorageAdapter;
  /** Storage key for the session (default: 'reactor.session') */
  storageKey?: string;
  /** Whether to automatically refresh tokens (default: true) */
  autoRefresh?: boolean;
  /** Whether to persist sessions to storage (default: true) */
  persistSession?: boolean;
  /** Whether to detect sessions from URL (default: true) */
  detectSessionInUrl?: boolean;
}

/**
 * Internal auth state.
 */
export interface AuthState {
  session: Session | null;
  user: User | null;
}

/**
 * Sign up request parameters.
 */
export interface SignUpParams {
  email: string;
  password: string;
  metadata?: Record<string, unknown>;
}

/**
 * Sign in request parameters.
 */
export interface SignInParams {
  email: string;
  password: string;
}

/**
 * Update user request parameters.
 */
export interface UpdateUserParams {
  email?: string;
  password?: string;
  metadata?: Record<string, unknown>;
}

/**
 * Password reset request parameters.
 */
export interface RequestPasswordResetParams {
  email: string;
}

/**
 * Password reset confirmation parameters.
 */
export interface ConfirmPasswordResetParams {
  token: string;
  newPassword: string;
}

/**
 * Email verification parameters.
 */
export interface VerifyEmailParams {
  token: string;
}

/**
 * Resend verification parameters.
 */
export interface ResendVerificationParams {
  email: string;
}

/**
 * Organization creation parameters.
 */
export interface CreateOrgParams {
  slug: string;
  name: string;
  metadata?: Record<string, unknown>;
}

/**
 * Organization update parameters.
 */
export interface UpdateOrgParams {
  name?: string;
  metadata?: Record<string, unknown>;
}

/**
 * Invitation creation parameters.
 */
export interface CreateInvitationParams {
  email: string;
  roleId?: string;
}

/**
 * Member update parameters.
 */
export interface UpdateMemberParams {
  roleId: string;
}

/**
 * API key creation parameters.
 */
export interface CreateApiKeyParams {
  name: string;
  scopes?: string[];
  expiresAt?: string;
}

/**
 * Auth state change callback.
 */
export type AuthStateChangeCallback = (
  event: AuthStateEvent,
  session: Session | null
) => void;

/**
 * Response from signup endpoint.
 */
export interface SignUpResponse {
  user: User;
  session: {
    access_token: string;
    refresh_token: string;
    expires_at: string;
  };
}

/**
 * Response from login endpoint.
 */
export interface LoginResponse {
  user: User;
  access_token: string;
  refresh_token: string;
  expires_at: string;
}

/**
 * Response from token refresh endpoint.
 */
export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  expires_at: string;
}

/**
 * Permissions response.
 */
export interface PermissionsResponse {
  permissions: string[];
  org?: {
    id: string;
    slug: string;
    role_id: string;
  };
}
