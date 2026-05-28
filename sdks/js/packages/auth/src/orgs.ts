import {
  type RequestContext,
  type Result,
  type Organization,
  type Member,
  type Role,
  type Invitation,
  type PaginationParams,
  get,
  post,
  patch,
  del,
} from '@reactor/shared';

import type {
  CreateOrgParams,
  UpdateOrgParams,
  CreateInvitationParams,
  UpdateMemberParams,
} from './types.js';

/**
 * Organizations client for managing organizations, members, and invitations.
 */
export class OrgsClient {
  constructor(private ctx: RequestContext) {}

  /**
   * List organizations the current user belongs to.
   */
  async list(params?: PaginationParams): Promise<Result<Organization[]>> {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set('limit', String(params.limit));
    if (params?.offset) searchParams.set('offset', String(params.offset));

    const query = searchParams.toString();
    const path = `/auth/v1/orgs${query ? `?${query}` : ''}`;

    return get<Organization[]>(this.ctx, path);
  }

  /**
   * Get an organization by ID or slug.
   */
  async get(idOrSlug: string): Promise<Result<Organization>> {
    return get<Organization>(this.ctx, `/auth/v1/orgs/${encodeURIComponent(idOrSlug)}`);
  }

  /**
   * Create a new organization.
   */
  async create(params: CreateOrgParams): Promise<Result<Organization>> {
    return post<Organization>(this.ctx, '/auth/v1/orgs', params);
  }

  /**
   * Update an organization.
   */
  async update(idOrSlug: string, params: UpdateOrgParams): Promise<Result<Organization>> {
    return patch<Organization>(
      this.ctx,
      `/auth/v1/orgs/${encodeURIComponent(idOrSlug)}`,
      params
    );
  }

  /**
   * Delete an organization.
   */
  async delete(idOrSlug: string): Promise<Result<void>> {
    return del<void>(this.ctx, `/auth/v1/orgs/${encodeURIComponent(idOrSlug)}`);
  }

  /**
   * Get a members client scoped to an organization.
   */
  members(orgId: string): MembersClient {
    return new MembersClient(this.ctx, orgId);
  }

  /**
   * Get the invitations client.
   */
  get invitations(): InvitationsClient {
    return new InvitationsClient(this.ctx);
  }

  /**
   * List available roles.
   */
  async listRoles(): Promise<Result<Role[]>> {
    return get<Role[]>(this.ctx, '/auth/v1/roles');
  }
}

/**
 * Members client for managing organization members.
 */
export class MembersClient {
  constructor(
    private ctx: RequestContext,
    private orgId: string
  ) {}

  /**
   * List members of the organization.
   */
  async list(params?: PaginationParams): Promise<Result<Member[]>> {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set('limit', String(params.limit));
    if (params?.offset) searchParams.set('offset', String(params.offset));

    const query = searchParams.toString();
    const path = `/auth/v1/orgs/${encodeURIComponent(this.orgId)}/members${query ? `?${query}` : ''}`;

    return get<Member[]>(this.ctx, path);
  }

  /**
   * Get a specific member.
   */
  async get(userId: string): Promise<Result<Member>> {
    return get<Member>(
      this.ctx,
      `/auth/v1/orgs/${encodeURIComponent(this.orgId)}/members/${encodeURIComponent(userId)}`
    );
  }

  /**
   * Invite a user to the organization.
   */
  async invite(params: CreateInvitationParams): Promise<Result<Invitation>> {
    return post<Invitation>(
      this.ctx,
      `/auth/v1/orgs/${encodeURIComponent(this.orgId)}/invitations`,
      params
    );
  }

  /**
   * Update a member's role.
   */
  async updateRole(userId: string, params: UpdateMemberParams): Promise<Result<Member>> {
    return patch<Member>(
      this.ctx,
      `/auth/v1/orgs/${encodeURIComponent(this.orgId)}/members/${encodeURIComponent(userId)}`,
      params
    );
  }

  /**
   * Remove a member from the organization.
   */
  async remove(userId: string): Promise<Result<void>> {
    return del<void>(
      this.ctx,
      `/auth/v1/orgs/${encodeURIComponent(this.orgId)}/members/${encodeURIComponent(userId)}`
    );
  }
}

/**
 * Invitations client for managing organization invitations.
 */
export class InvitationsClient {
  constructor(private ctx: RequestContext) {}

  /**
   * List pending invitations for the current user.
   */
  async list(params?: PaginationParams): Promise<Result<Invitation[]>> {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set('limit', String(params.limit));
    if (params?.offset) searchParams.set('offset', String(params.offset));

    const query = searchParams.toString();
    const path = `/auth/v1/invitations${query ? `?${query}` : ''}`;

    return get<Invitation[]>(this.ctx, path);
  }

  /**
   * Accept an invitation.
   */
  async accept(params: { token: string }): Promise<Result<Member>> {
    return post<Member>(this.ctx, '/auth/v1/invitations/accept', params);
  }

  /**
   * Revoke an invitation (org admin only).
   */
  async revoke(invitationId: string): Promise<Result<void>> {
    return del<void>(this.ctx, `/auth/v1/invitations/${encodeURIComponent(invitationId)}`);
  }
}
