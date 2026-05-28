import {
  type RequestContext,
  type Result,
  type ApiKey,
  type PaginationParams,
  get,
  post,
  del,
} from '@reactor/shared';

import type { CreateApiKeyParams } from './types.js';

/**
 * API keys client for managing user API keys.
 */
export class ApiKeysClient {
  constructor(private ctx: RequestContext) {}

  /**
   * List API keys for the current user.
   */
  async list(params?: PaginationParams): Promise<Result<ApiKey[]>> {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set('limit', String(params.limit));
    if (params?.offset) searchParams.set('offset', String(params.offset));

    const query = searchParams.toString();
    const path = `/auth/v1/keys${query ? `?${query}` : ''}`;

    return get<ApiKey[]>(this.ctx, path);
  }

  /**
   * Create a new API key.
   * Returns the full key value only once - store it securely.
   */
  async create(params: CreateApiKeyParams): Promise<Result<ApiKey & { key: string }>> {
    return post<ApiKey & { key: string }>(this.ctx, '/auth/v1/keys', params);
  }

  /**
   * Revoke an API key.
   */
  async revoke(keyId: string): Promise<Result<void>> {
    return del<void>(this.ctx, `/auth/v1/keys/${encodeURIComponent(keyId)}`);
  }
}
