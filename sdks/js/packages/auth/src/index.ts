/**
 * @reactor/auth - Authentication client for Reactor JS SDK
 *
 * This package provides:
 * - User authentication (sign up, sign in, sign out)
 * - Session management with automatic token refresh
 * - Multi-tab synchronization
 * - Organization and membership management
 * - Permission checking
 * - API key management
 */

export { AuthClient } from './client.js';
export { AuthStateManager } from './state.js';
export { OrgsClient, MembersClient, InvitationsClient } from './orgs.js';
export { PermissionsClient } from './permissions.js';
export { ApiKeysClient } from './api-keys.js';

export {
  detectSessionInUrl,
  cleanUrlAfterDetection,
  detectAndClean,
  type DetectedToken,
  type DetectedTokenType,
} from './url-detect.js';

export type {
  User,
  Session,
  Organization,
  Member,
  Role,
  Invitation,
  ApiKey,
  AuthStateEvent,
  AuthClientOptions,
  SignUpParams,
  SignInParams,
  UpdateUserParams,
  RequestPasswordResetParams,
  ConfirmPasswordResetParams,
  VerifyEmailParams,
  ResendVerificationParams,
  CreateOrgParams,
  UpdateOrgParams,
  CreateInvitationParams,
  UpdateMemberParams,
  CreateApiKeyParams,
  AuthStateChangeCallback,
  PermissionsResponse,
} from './types.js';
