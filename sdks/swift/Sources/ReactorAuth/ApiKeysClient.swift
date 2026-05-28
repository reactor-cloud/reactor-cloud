import Foundation
import ReactorShared

/// API keys client for managing API keys.
public final class ApiKeysClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Response from creating an API key, includes the full key (only shown once).
    public struct CreateApiKeyResponse: Decodable, Sendable {
        public let id: String
        public let name: String
        public let key: String
        public let keyPrefix: String
        public let scopes: [String]
        public let expiresAt: Date?
        public let createdAt: Date
        
        enum CodingKeys: String, CodingKey {
            case id, name, key, scopes
            case keyPrefix = "key_prefix"
            case expiresAt = "expires_at"
            case createdAt = "created_at"
        }
    }
    
    /// Create a new API key.
    ///
    /// - Parameters:
    ///   - name: Key name
    ///   - scopes: Permission scopes
    ///   - expiresAt: Optional expiration date
    /// - Returns: The created key (includes full key, only shown once)
    public func create(
        name: String,
        scopes: [String] = [],
        expiresAt: Date? = nil
    ) async throws -> CreateApiKeyResponse {
        struct CreateRequest: Encodable {
            let name: String
            let scopes: [String]
            let expires_at: Date?
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/api-keys",
            method: .post,
            body: CreateRequest(name: name, scopes: scopes, expires_at: expiresAt)
        )
    }
    
    /// List API keys.
    public func list() async throws -> [ApiKey] {
        try await request(ctx, path: "/auth/v1/api-keys")
    }
    
    /// Get an API key by ID.
    ///
    /// - Parameter id: API key ID
    public func get(id: String) async throws -> ApiKey {
        try await request(ctx, path: "/auth/v1/api-keys/\(id)")
    }
    
    /// Update an API key.
    ///
    /// - Parameters:
    ///   - id: API key ID
    ///   - name: New name (optional)
    ///   - scopes: New scopes (optional)
    public func update(
        id: String,
        name: String? = nil,
        scopes: [String]? = nil
    ) async throws -> ApiKey {
        struct UpdateRequest: Encodable {
            let name: String?
            let scopes: [String]?
        }
        
        return try await request(
            ctx,
            path: "/auth/v1/api-keys/\(id)",
            method: .patch,
            body: UpdateRequest(name: name, scopes: scopes)
        )
    }
    
    /// Revoke (delete) an API key.
    ///
    /// - Parameter id: API key ID
    public func revoke(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/api-keys/\(id)", method: .delete)
    }
}
