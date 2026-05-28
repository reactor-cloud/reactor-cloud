/**
 * @reactor/shared - Shared utilities for Reactor JS SDK
 *
 * This package provides common functionality used across all Reactor SDK packages:
 * - Error types and handling
 * - Result type for error handling
 * - JWT decoding
 * - Storage adapters for session persistence
 * - Query string utilities
 * - HTTP request wrapper
 */

// Errors
export {
  ReactorError,
  AuthError,
  ForbiddenError,
  ValidationError,
  NotFoundError,
  ConflictError,
  RateLimitError,
  ServerError,
  NetworkError,
  AbortError,
  TimeoutError,
  errorFromResponse,
  isErrorEnvelope,
  type ErrorEnvelope,
} from './errors.js';

// Result type
export {
  type Result,
  type ResultPromise,
  type ThrowOnError,
  ok,
  err,
  withThrowOnError,
  createResultPromise,
} from './result.js';

// JWT utilities
export {
  type JWTPayload,
  decodeJwt,
  isJwtExpired,
  getJwtExpiry,
  getJwtTimeRemaining,
} from './jwt.js';

// Storage adapters
export {
  type StorageAdapter,
  memoryAdapter,
  localStorageAdapter,
  sessionStorageAdapter,
  cookieAdapter,
  detectStorageAdapter,
} from './storage-adapter.js';

// Query utilities
export {
  type FilterOperator,
  type FilterValue,
  type OrderDirection,
  type OrderNulls,
  type QueryParams,
  encodeFilterValue,
  buildFilterExpression,
  buildOrderExpression,
  queryParamsToSearchParams,
  buildUrl,
  parseContentRange,
  parseSelectColumns,
  encodePathSegment,
} from './query.js';

// Fetch utilities
export {
  type RequestOptions,
  type RequestContext,
  SDK_VERSION,
  request,
  get,
  post,
  put,
  patch,
  del,
} from './fetch.js';

// Common types
export {
  type User,
  type Session,
  type Organization,
  type Member,
  type Role,
  type Invitation,
  type ApiKey,
  type PaginationParams,
  type PaginatedResponse,
  type AuthStateEvent,
  type AuthStateSubscription,
  type DatabaseSchema,
  type GenericDatabase,
  type TableRow,
  type TableInsert,
  type TableUpdate,
} from './types.js';
