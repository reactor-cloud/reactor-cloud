import {
  type RequestContext,
  type Result,
  get,
  post,
} from '@reactor/shared';

import type { PermissionsResponse } from './types.js';

/**
 * Permissions client for checking user permissions.
 */
export class PermissionsClient {
  constructor(private ctx: RequestContext) {}

  /**
   * Get all permissions for the current user.
   * Optionally scoped to a specific organization.
   */
  async get(options?: { org?: string }): Promise<Result<PermissionsResponse>> {
    const searchParams = new URLSearchParams();
    if (options?.org) {
      searchParams.set('org', options.org);
    }

    const query = searchParams.toString();
    const path = `/auth/v1/permissions${query ? `?${query}` : ''}`;

    return get<PermissionsResponse>(this.ctx, path);
  }

  /**
   * Check if the current user has specific permissions.
   * Returns true if all requested permissions are granted.
   */
  async check(
    permissions: string[],
    options?: { org?: string }
  ): Promise<Result<{ allowed: boolean; missing?: string[] }>> {
    return post<{ allowed: boolean; missing?: string[] }>(
      this.ctx,
      '/auth/v1/permissions/check',
      {
        permissions,
        org: options?.org,
      }
    );
  }

  /**
   * Resolve the current organization context.
   * Returns the organization determined by the X-Reactor-Org header or default.
   */
  async resolveContext(): Promise<Result<{ org?: { id: string; slug: string; role_id: string } }>> {
    return get<{ org?: { id: string; slug: string; role_id: string } }>(
      this.ctx,
      '/auth/v1/permissions/ctx'
    );
  }
}
