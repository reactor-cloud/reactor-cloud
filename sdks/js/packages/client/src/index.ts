/**
 * @reactor/client - The official Reactor JS/TS SDK
 *
 * This is the main package users install. It composes all capability packages
 * and provides a unified client interface.
 *
 * @example
 * ```ts
 * import { createClient } from '@reactor/client';
 * import type { Database } from './database.types';
 *
 * const reactor = createClient<Database>('https://reactor.cloud', {
 *   key: 'rk_pub_...',
 * });
 *
 * // Auth
 * await reactor.auth.signIn({ email, password });
 *
 * // Data (PostgREST-style)
 * const { data } = await reactor.from('posts').select('*').eq('published', true);
 *
 * // Storage
 * await reactor.storage.from('avatars').upload('me.jpg', file);
 *
 * // Functions
 * await reactor.functions.invoke('process-order', { body: { orderId } });
 *
 * // Jobs
 * await reactor.jobs.trigger('send-email', { payload: { to, subject } });
 * ```
 */

import {
  type RequestContext,
  type StorageAdapter,
  detectStorageAdapter,
} from '@reactor/shared';

import { ReactorAnalytics, type AnalyticsConfig } from '@reactor/analytics';
import { AuthClient, type AuthClientOptions } from '@reactor/auth';
import { DataClient, type GenericSchema } from '@reactor/data';
import { StorageClient } from '@reactor/storage';
import { FunctionsClient } from '@reactor/functions';
import { JobsClient } from '@reactor/jobs';
import { SitesClient } from '@reactor/sites';
import { RealtimeClient } from '@reactor/realtime';

/**
 * Options for creating a Reactor client.
 */
export interface ReactorClientOptions {
  /** Project key (anon key) - safe for browser bundles */
  key?: string;
  /** Default organization context */
  org?: string;
  /** Custom fetch implementation */
  fetch?: typeof fetch;
  /** Global headers for all requests */
  headers?: Record<string, string>;
  /** Auth-specific options */
  auth?: AuthClientOptions;
  /** Analytics-specific options */
  analytics?: Omit<AnalyticsConfig, 'projectKey' | 'endpoint'> & { enabled?: boolean };
  /** Custom storage adapter for session persistence */
  storage?: StorageAdapter;
}

/**
 * The unified Reactor client interface.
 */
export interface ReactorClient<Schema extends GenericSchema = GenericSchema> {
  /** Authentication client */
  auth: AuthClient;
  /** Analytics client */
  analytics: ReactorAnalytics;
  /** Data query builder (PostgREST-style) */
  from: DataClient<Schema>['from'];
  /** RPC calls */
  rpc: DataClient<Schema>['rpc'];
  /** Storage client */
  storage: StorageClient;
  /** Functions client */
  functions: FunctionsClient;
  /** Jobs client */
  jobs: JobsClient;
  /** Sites admin client */
  sites: SitesClient;
  /** Realtime client (stub) */
  realtime: RealtimeClient;
}

/**
 * Create a Reactor client.
 *
 * @param url - The Reactor API URL (e.g., 'https://reactor.cloud')
 * @param options - Client configuration options
 * @returns A configured Reactor client
 *
 * @example
 * ```ts
 * import { createClient } from '@reactor/client';
 *
 * const reactor = createClient('https://reactor.cloud', {
 *   key: 'rk_pub_...',
 * });
 * ```
 */
export function createClient<Schema extends GenericSchema = GenericSchema>(
  url: string,
  options: ReactorClientOptions = {}
): ReactorClient<Schema> {
  const { key, org, fetch: customFetch, headers, auth: authOptions, analytics: analyticsOptions } = options;

  // Create the shared request context
  const ctx: RequestContext = {
    baseUrl: url,
    projectKey: key,
    fetch: customFetch,
    defaultHeaders: {
      ...headers,
      ...(org && { 'X-Reactor-Org': org }),
    },
  };

  // Create the auth client first (for token management)
  const storage = options.storage ?? authOptions?.storage ?? detectStorageAdapter();
  const authClient = new AuthClient(ctx, {
    ...authOptions,
    storage,
  });

  // Update context with auth token getter
  const authenticatedCtx: RequestContext = {
    ...ctx,
    getAccessToken: () => authClient.getAccessToken(),
  };

  // Create analytics client
  const analyticsEnabled = analyticsOptions?.enabled !== false && !!key;
  const analyticsClient = new ReactorAnalytics({
    projectKey: key || '',
    endpoint: `${url}/analytics/v1`,
    ...analyticsOptions,
  });

  // Wire up auto-identify when user signs in
  if (analyticsEnabled) {
    authClient.onAuthStateChange((event, session) => {
      if (event === 'SIGNED_IN' && session?.user) {
        analyticsClient.identify(session.user.id, {
          email: session.user.email,
          ...(session.user.metadata as Record<string, string | number | boolean | null | undefined>),
        });
      } else if (event === 'SIGNED_OUT') {
        analyticsClient.reset();
      }
    });
  }

  // Create capability clients
  const dataClient = new DataClient<Schema>(authenticatedCtx);
  const storageClient = new StorageClient(authenticatedCtx);
  const functionsClient = new FunctionsClient(authenticatedCtx);
  const jobsClient = new JobsClient(authenticatedCtx);
  const sitesClient = new SitesClient(authenticatedCtx);
  const realtimeClient = new RealtimeClient(authenticatedCtx);

  return {
    auth: authClient,
    analytics: analyticsClient,
    from: dataClient.from.bind(dataClient),
    rpc: dataClient.rpc.bind(dataClient),
    storage: storageClient,
    functions: functionsClient,
    jobs: jobsClient,
    sites: sitesClient,
    realtime: realtimeClient,
  };
}

// Re-export commonly used types
export type {
  User,
  Session,
  Organization,
  Member,
  Role,
  Invitation,
  ApiKey,
  AuthStateEvent,
  AuthStateSubscription,
  Result,
  ReactorError,
  AuthError,
  ValidationError,
  NotFoundError,
  StorageAdapter,
} from '@reactor/shared';

export type {
  AuthClientOptions,
  SignUpParams,
  SignInParams,
  UpdateUserParams,
} from '@reactor/auth';

export type {
  GenericSchema,
  CountMode,
  FilterOperator,
} from '@reactor/data';

export type {
  FileObject,
  Bucket,
  UploadOptions,
  ListOptions,
} from '@reactor/storage';

export type {
  InvokeOptions,
  FunctionVersion,
  FunctionLog,
} from '@reactor/functions';

export type {
  JobRun,
  RunStatus,
  TriggerOptions,
  WaitOptions,
} from '@reactor/jobs';

export type {
  Deployment,
  Domain,
  Site,
} from '@reactor/sites';

export type {
  RealtimeChannel,
  RealtimeEvent,
  RealtimePayload,
} from '@reactor/realtime';

export type {
  AnalyticsConfig,
  EventProperties,
  UserTraits,
  PageContext,
} from '@reactor/analytics';

export { ReactorAnalytics } from '@reactor/analytics';
