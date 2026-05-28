import {
  type RequestContext,
  type Result,
  get,
  post,
  patch,
  del,
} from '@reactor/shared';

// ============================================================================
// Types
// ============================================================================

/** Connector descriptor from the catalog. */
export interface ConnectorDescriptor {
  type_id: string;
  display_name: string;
  version: string;
  runtime: 'native' | 'manifest' | 'airbyte_container';
  auth: AuthDescriptor;
  streams: StreamDescriptor[];
  actions: ActionDescriptor[];
  webhooks: WebhookDescriptor[];
  capabilities: ConnectorCapabilities;
  rate_limits?: RateLimitDescriptor;
  doc_url?: string;
}

/** Authentication descriptor. */
export interface AuthDescriptor {
  kind: AuthKind;
  fields: AuthField[];
  test?: TestCallDescriptor;
}

/** Authentication kind. */
export type AuthKind =
  | { kind: 'oauth2'; authorize_url: string; token_url: string; scopes: string[]; pkce?: boolean; reactor_proxy?: boolean }
  | { kind: 'personal_access_token'; header: string; format: string }
  | { kind: 'basic' }
  | { kind: 'custom'; docs_url: string };

/** Authentication field. */
export interface AuthField {
  name: string;
  label: string;
  sensitive?: boolean;
  required?: boolean;
  description?: string;
}

/** Test call descriptor. */
export interface TestCallDescriptor {
  method: string;
  path: string;
  success_codes?: number[];
}

/** Stream descriptor. */
export interface StreamDescriptor {
  name: string;
  json_schema: Record<string, unknown>;
  supported_modes: SyncMode[];
  cursor_field?: string[];
  primary_key?: string[][];
  supports_outbound?: boolean;
  source_defined?: boolean;
}

/** Sync mode for streams. */
export type SyncMode = 'full_refresh' | 'incremental_append' | 'incremental_dedup';

/** Action descriptor. */
export interface ActionDescriptor {
  name: string;
  input_schema: Record<string, unknown>;
  output_schema: Record<string, unknown>;
  side_effects: SideEffectKind;
  dry_run: DryRunSupport;
  idempotency?: IdempotencyHint;
}

/** Side effect classification. */
export type SideEffectKind = 'reads' | 'mutates' | 'sends';

/** Dry-run support level. */
export type DryRunSupport = 'native' | 'synthesized' | 'unsupported';

/** Idempotency hint. */
export interface IdempotencyHint {
  key_path: string;
  ttl_seconds: number;
}

/** Webhook descriptor. */
export interface WebhookDescriptor {
  name: string;
  verification: VerificationKind;
  event_types: string[];
  replay_window_seconds?: number;
  setup_instructions?: string;
}

/** Webhook verification kind. */
export type VerificationKind =
  | { kind: 'hmac_sha256'; header: string; secret_field: string }
  | { kind: 'ed25519'; header: string; key_id_header?: string }
  | { kind: 'custom'; docs_url: string };

/** Connector capabilities. */
export interface ConnectorCapabilities {
  sandbox_mode?: boolean;
  vendor_test_mode?: boolean;
  cdc?: boolean;
  incremental?: boolean;
  schema_discovery?: boolean;
}

/** Rate limit descriptor. */
export interface RateLimitDescriptor {
  requests_per_second?: number;
  requests_per_minute?: number;
  requests_per_hour?: number;
  requests_per_day?: number;
  concurrent_requests?: number;
}

/** Connector instance. */
export interface Instance {
  id: string;
  connector_type: string;
  name: string;
  config: Record<string, unknown>;
  status: InstanceStatus;
  created_at: string;
  updated_at: string;
}

/** Instance status. */
export type InstanceStatus = 'active' | 'inactive' | 'error' | 'pending_credentials';

/** Connection status. */
export interface ConnectionStatus {
  status: 'succeeded' | 'failed';
  message?: string;
}

/** Action result. */
export interface ActionResult {
  output: unknown;
  dry_run?: boolean;
}

/** Webhook receiver. */
export interface Receiver {
  id: string;
  name: string;
  target: ReceiverTarget;
  status: 'active' | 'inactive';
  filter_expression?: string;
  created_at: string;
  updated_at: string;
}

/** Receiver target. */
export type ReceiverTarget =
  | { type: 'job'; name: string }
  | { type: 'stream'; connection: string }
  | { type: 'action'; instance: string; action: string }
  | { type: 'function'; name: string };

/** Receiver with token (returned on create). */
export interface ReceiverWithToken extends Receiver {
  token: string;
}

/** Rotate token response. */
export interface RotateTokenResponse {
  new_token: string;
  old_token_expires_at: string;
}

// ============================================================================
// Options
// ============================================================================

/** Options for creating an instance. */
export interface CreateInstanceOptions {
  name: string;
  config?: Record<string, unknown>;
}

/** Options for updating an instance. */
export interface UpdateInstanceOptions {
  name?: string;
  config?: Record<string, unknown>;
}

/** Options for invoking an action. */
export interface InvokeActionOptions {
  input?: Record<string, unknown>;
  dryRun?: boolean;
  idempotencyKey?: string;
}

/** Options for creating a receiver. */
export interface CreateReceiverOptions {
  name: string;
  target: ReceiverTarget;
  filterExpression?: string;
}

/** Options for listing instances. */
export interface ListInstancesOptions {
  connectorType?: string;
  status?: InstanceStatus;
  limit?: number;
  offset?: number;
}

// ============================================================================
// Client
// ============================================================================

/** Connect client for managing connectors and integrations. */
export class ConnectClient {
  constructor(private ctx: RequestContext) {}

  // ----------------------------------------
  // Catalog
  // ----------------------------------------

  /** Access the connector catalog. */
  get catalog() {
    return {
      /** List available connectors. */
      list: async (): Promise<Result<ConnectorDescriptor[]>> =>
        get(this.ctx, '/connect/v1/catalog'),

      /** Get connector details. */
      get: async (connectorType: string): Promise<Result<ConnectorDescriptor>> =>
        get(this.ctx, `/connect/v1/catalog/${encodeURIComponent(connectorType)}`),
    };
  }

  // ----------------------------------------
  // Instances
  // ----------------------------------------

  /** Manage connector instances. */
  get instances() {
    return {
      /** List instances. */
      list: async (options?: ListInstancesOptions): Promise<Result<Instance[]>> => {
        const params = new URLSearchParams();
        if (options?.connectorType) params.set('connector_type', options.connectorType);
        if (options?.status) params.set('status', options.status);
        if (options?.limit) params.set('limit', String(options.limit));
        if (options?.offset) params.set('offset', String(options.offset));
        const query = params.toString();
        return get(this.ctx, `/connect/v1/instances${query ? '?' + query : ''}`);
      },

      /** Create a new instance. */
      create: async (
        connectorType: string,
        options: CreateInstanceOptions
      ): Promise<Result<Instance>> =>
        post(this.ctx, '/connect/v1/instances', {
          connector_type: connectorType,
          name: options.name,
          config: options.config ?? {},
        }),

      /** Get instance details. */
      get: async (instanceId: string): Promise<Result<Instance>> =>
        get(this.ctx, `/connect/v1/instances/${encodeURIComponent(instanceId)}`),

      /** Update an instance. */
      update: async (
        instanceId: string,
        options: UpdateInstanceOptions
      ): Promise<Result<Instance>> =>
        patch(this.ctx, `/connect/v1/instances/${encodeURIComponent(instanceId)}`, options),

      /** Delete an instance. */
      delete: async (instanceId: string): Promise<Result<void>> =>
        del(this.ctx, `/connect/v1/instances/${encodeURIComponent(instanceId)}`),

      /** Test instance credentials. */
      check: async (instanceId: string): Promise<Result<ConnectionStatus>> =>
        post(this.ctx, `/connect/v1/instances/${encodeURIComponent(instanceId)}/check`, {}),

      /** Set credentials for an instance. */
      setCredentials: async (
        instanceId: string,
        credentials: Record<string, unknown>
      ): Promise<Result<void>> =>
        post(
          this.ctx,
          `/connect/v1/instances/${encodeURIComponent(instanceId)}/credentials`,
          credentials
        ),
    };
  }

  // ----------------------------------------
  // Actions
  // ----------------------------------------

  /**
   * Invoke an action on a connector instance.
   *
   * @example
   * ```ts
   * const result = await connect.invoke('stripe-prod', 'createCustomer', {
   *   input: { email: 'user@example.com', name: 'Test User' },
   *   dryRun: true,
   * });
   * ```
   */
  async invoke(
    instanceId: string,
    action: string,
    options?: InvokeActionOptions
  ): Promise<Result<ActionResult>> {
    const headers: Record<string, string> = {};
    if (options?.idempotencyKey) {
      headers['Idempotency-Key'] = options.idempotencyKey;
    }

    return post(
      this.ctx,
      `/connect/v1/instances/${encodeURIComponent(instanceId)}/actions/${encodeURIComponent(action)}`,
      {
        input: options?.input ?? {},
        dry_run: options?.dryRun ?? false,
      },
      { headers }
    );
  }

  // ----------------------------------------
  // Receivers
  // ----------------------------------------

  /** Manage webhook receivers. */
  get receivers() {
    return {
      /** List receivers. */
      list: async (): Promise<Result<Receiver[]>> =>
        get(this.ctx, '/connect/v1/receivers'),

      /** Create a new receiver. */
      create: async (options: CreateReceiverOptions): Promise<Result<ReceiverWithToken>> =>
        post(this.ctx, '/connect/v1/receivers', {
          name: options.name,
          target: options.target,
          filter_expression: options.filterExpression,
        }),

      /** Get receiver details. */
      get: async (receiverId: string): Promise<Result<Receiver>> =>
        get(this.ctx, `/connect/v1/receivers/${encodeURIComponent(receiverId)}`),

      /** Delete a receiver. */
      delete: async (receiverId: string): Promise<Result<void>> =>
        del(this.ctx, `/connect/v1/receivers/${encodeURIComponent(receiverId)}`),

      /** Rotate receiver token. */
      rotate: async (
        receiverId: string,
        graceSeconds?: number
      ): Promise<Result<RotateTokenResponse>> =>
        post(this.ctx, `/connect/v1/receivers/${encodeURIComponent(receiverId)}/rotate`, {
          grace_seconds: graceSeconds ?? 300,
        }),
    };
  }
}

/**
 * Create a Connect client.
 *
 * @example
 * ```ts
 * import { createClient } from '@reactor/client';
 * import { createConnectClient } from '@reactor/connect';
 *
 * const reactor = createClient({ endpoint: 'https://api.example.com' });
 * const connect = createConnectClient(reactor.context);
 *
 * // List available connectors
 * const { data: connectors } = await connect.catalog.list();
 *
 * // Create a Stripe instance
 * const { data: instance } = await connect.instances.create('stripe', {
 *   name: 'stripe-prod',
 *   config: { api_key: 'sk_test_...' },
 * });
 *
 * // Invoke an action
 * const { data: result } = await connect.invoke(instance.id, 'createCustomer', {
 *   input: { email: 'user@example.com' },
 *   dryRun: true,
 * });
 * ```
 */
export function createConnectClient(ctx: RequestContext): ConnectClient {
  return new ConnectClient(ctx);
}
