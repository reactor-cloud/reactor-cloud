import Foundation
import ReactorShared

/// Organizations client for managing orgs, members, roles, and invitations.
public final class OrgsClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    // MARK: - Organizations
    
    /// Create a new organization.
    ///
    /// - Parameters:
    ///   - name: Organization name
    ///   - slug: URL-friendly slug
    ///   - metadata: Optional metadata
    /// - Returns: The created organization
    public func create(
        name: String,
        slug: String,
        metadata: [String: AnyCodable] = [:]
    ) async throws -> Organization {
        struct CreateOrgRequest: Encodable {
            let name: String
            let slug: String
            let metadata: [String: AnyCodable]
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/orgs",
            method: .post,
            body: CreateOrgRequest(name: name, slug: slug, metadata: metadata)
        )
    }
    
    /// List organizations the current user belongs to.
    public func list() async throws -> [Organization] {
        try await request(ctx, path: "/auth/v1/orgs")
    }
    
    /// Get an organization by ID.
    ///
    /// - Parameter id: Organization ID
    public func get(id: String) async throws -> Organization {
        try await request(ctx, path: "/auth/v1/orgs/\(id)")
    }
    
    /// Get an organization by slug.
    ///
    /// - Parameter slug: Organization slug
    public func getBySlug(_ slug: String) async throws -> Organization {
        try await request(ctx, path: "/auth/v1/orgs/slug/\(slug)")
    }
    
    /// Update an organization.
    ///
    /// - Parameters:
    ///   - id: Organization ID
    ///   - name: New name (optional)
    ///   - metadata: New metadata (optional)
    public func update(
        id: String,
        name: String? = nil,
        metadata: [String: AnyCodable]? = nil
    ) async throws -> Organization {
        struct UpdateOrgRequest: Encodable {
            let name: String?
            let metadata: [String: AnyCodable]?
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/orgs/\(id)",
            method: .patch,
            body: UpdateOrgRequest(name: name, metadata: metadata)
        )
    }
    
    /// Delete an organization.
    ///
    /// - Parameter id: Organization ID
    public func delete(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/orgs/\(id)", method: .delete)
    }
    
    // MARK: - Members
    
    /// List members of an organization.
    ///
    /// - Parameter orgId: Organization ID
    public func listMembers(orgId: String) async throws -> [Member] {
        try await request(ctx, path: "/auth/v1/orgs/\(orgId)/members")
    }
    
    /// Get a member by user ID.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - userId: User ID
    public func getMember(orgId: String, userId: String) async throws -> Member {
        try await request(ctx, path: "/auth/v1/orgs/\(orgId)/members/\(userId)")
    }
    
    /// Update a member's role.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - userId: User ID
    ///   - roleId: New role ID
    public func updateMember(orgId: String, userId: String, roleId: String) async throws -> Member {
        struct UpdateMemberRequest: Encodable {
            let role_id: String
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/orgs/\(orgId)/members/\(userId)",
            method: .patch,
            body: UpdateMemberRequest(role_id: roleId)
        )
    }
    
    /// Remove a member from an organization.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - userId: User ID
    public func removeMember(orgId: String, userId: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/orgs/\(orgId)/members/\(userId)", method: .delete)
    }
    
    // MARK: - Roles
    
    /// List roles for an organization.
    ///
    /// - Parameter orgId: Organization ID
    public func listRoles(orgId: String) async throws -> [Role] {
        try await request(ctx, path: "/auth/v1/orgs/\(orgId)/roles")
    }
    
    /// Create a role.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - name: Role name
    ///   - description: Role description
    ///   - permissions: List of permissions
    public func createRole(
        orgId: String,
        name: String,
        description: String? = nil,
        permissions: [String] = []
    ) async throws -> Role {
        struct CreateRoleRequest: Encodable {
            let name: String
            let description: String?
            let permissions: [String]
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/orgs/\(orgId)/roles",
            method: .post,
            body: CreateRoleRequest(name: name, description: description, permissions: permissions)
        )
    }
    
    /// Update a role.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - roleId: Role ID
    ///   - name: New name (optional)
    ///   - description: New description (optional)
    ///   - permissions: New permissions (optional)
    public func updateRole(
        orgId: String,
        roleId: String,
        name: String? = nil,
        description: String? = nil,
        permissions: [String]? = nil
    ) async throws -> Role {
        struct UpdateRoleRequest: Encodable {
            let name: String?
            let description: String?
            let permissions: [String]?
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/orgs/\(orgId)/roles/\(roleId)",
            method: .patch,
            body: UpdateRoleRequest(name: name, description: description, permissions: permissions)
        )
    }
    
    /// Delete a role.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - roleId: Role ID
    public func deleteRole(orgId: String, roleId: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/orgs/\(orgId)/roles/\(roleId)", method: .delete)
    }
    
    // MARK: - Invitations
    
    /// List pending invitations for an organization.
    ///
    /// - Parameter orgId: Organization ID
    public func listInvitations(orgId: String) async throws -> [Invitation] {
        try await request(ctx, path: "/auth/v1/orgs/\(orgId)/invitations")
    }
    
    /// Invite a user to an organization.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - email: Email to invite
    ///   - roleId: Role to assign
    public func invite(orgId: String, email: String, roleId: String) async throws -> Invitation {
        struct InviteRequest: Encodable {
            let email: String
            let role_id: String
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/orgs/\(orgId)/invitations",
            method: .post,
            body: InviteRequest(email: email, role_id: roleId)
        )
    }
    
    /// Revoke an invitation.
    ///
    /// - Parameters:
    ///   - orgId: Organization ID
    ///   - invitationId: Invitation ID
    public func revokeInvitation(orgId: String, invitationId: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/orgs/\(orgId)/invitations/\(invitationId)", method: .delete)
    }
    
    /// Accept an invitation (current user).
    ///
    /// - Parameter token: Invitation token
    public func acceptInvitation(token: String) async throws -> Member {
        struct AcceptRequest: Encodable {
            let token: String
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/invitations/accept",
            method: .post,
            body: AcceptRequest(token: token)
        )
    }
}
