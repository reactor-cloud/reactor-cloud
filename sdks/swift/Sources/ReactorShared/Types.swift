import Foundation

/// User object returned from auth endpoints.
public struct User: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let email: String
    public let emailVerified: Bool
    public let metadata: [String: AnyCodable]
    public let createdAt: Date
    
    public init(
        id: String,
        email: String,
        emailVerified: Bool,
        metadata: [String: AnyCodable] = [:],
        createdAt: Date
    ) {
        self.id = id
        self.email = email
        self.emailVerified = emailVerified
        self.metadata = metadata
        self.createdAt = createdAt
    }
    
    enum CodingKeys: String, CodingKey {
        case id
        case email
        case emailVerified = "email_verified"
        case metadata
        case createdAt = "created_at"
    }
}

/// Session object containing tokens.
public struct Session: Codable, Sendable, Equatable {
    public let accessToken: String
    public let refreshToken: String
    public let expiresAt: Date
    public let user: User
    
    public init(
        accessToken: String,
        refreshToken: String,
        expiresAt: Date,
        user: User
    ) {
        self.accessToken = accessToken
        self.refreshToken = refreshToken
        self.expiresAt = expiresAt
        self.user = user
    }
    
    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case refreshToken = "refresh_token"
        case expiresAt = "expires_at"
        case user
    }
}

/// Organization object.
public struct Organization: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let slug: String
    public let name: String
    public let metadata: [String: AnyCodable]
    public let createdAt: Date
    public let updatedAt: Date
    
    public init(
        id: String,
        slug: String,
        name: String,
        metadata: [String: AnyCodable] = [:],
        createdAt: Date,
        updatedAt: Date
    ) {
        self.id = id
        self.slug = slug
        self.name = name
        self.metadata = metadata
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
    
    enum CodingKeys: String, CodingKey {
        case id, slug, name, metadata
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

/// Organization membership.
public struct Member: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let userId: String
    public let orgId: String
    public let roleId: String
    public let user: User
    public let createdAt: Date
    public let updatedAt: Date
    
    public init(
        id: String,
        userId: String,
        orgId: String,
        roleId: String,
        user: User,
        createdAt: Date,
        updatedAt: Date
    ) {
        self.id = id
        self.userId = userId
        self.orgId = orgId
        self.roleId = roleId
        self.user = user
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
    
    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case orgId = "org_id"
        case roleId = "role_id"
        case user
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

/// Organization role.
public struct Role: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let name: String
    public let description: String?
    public let permissions: [String]
    public let isDefault: Bool
    
    public init(
        id: String,
        name: String,
        description: String?,
        permissions: [String],
        isDefault: Bool
    ) {
        self.id = id
        self.name = name
        self.description = description
        self.permissions = permissions
        self.isDefault = isDefault
    }
    
    enum CodingKeys: String, CodingKey {
        case id, name, description, permissions
        case isDefault = "is_default"
    }
}

/// Organization invitation.
public struct Invitation: Codable, Sendable, Equatable, Identifiable {
    public enum Status: String, Codable, Sendable {
        case pending
        case accepted
        case expired
        case revoked
    }
    
    public let id: String
    public let orgId: String
    public let email: String
    public let roleId: String
    public let status: Status
    public let expiresAt: Date
    public let createdAt: Date
    
    public init(
        id: String,
        orgId: String,
        email: String,
        roleId: String,
        status: Status,
        expiresAt: Date,
        createdAt: Date
    ) {
        self.id = id
        self.orgId = orgId
        self.email = email
        self.roleId = roleId
        self.status = status
        self.expiresAt = expiresAt
        self.createdAt = createdAt
    }
    
    enum CodingKeys: String, CodingKey {
        case id
        case orgId = "org_id"
        case email
        case roleId = "role_id"
        case status
        case expiresAt = "expires_at"
        case createdAt = "created_at"
    }
}

/// API key.
public struct ApiKey: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let name: String
    public let keyPrefix: String
    public let scopes: [String]
    public let lastUsedAt: Date?
    public let expiresAt: Date?
    public let createdAt: Date
    
    public init(
        id: String,
        name: String,
        keyPrefix: String,
        scopes: [String],
        lastUsedAt: Date?,
        expiresAt: Date?,
        createdAt: Date
    ) {
        self.id = id
        self.name = name
        self.keyPrefix = keyPrefix
        self.scopes = scopes
        self.lastUsedAt = lastUsedAt
        self.expiresAt = expiresAt
        self.createdAt = createdAt
    }
    
    enum CodingKeys: String, CodingKey {
        case id, name, scopes
        case keyPrefix = "key_prefix"
        case lastUsedAt = "last_used_at"
        case expiresAt = "expires_at"
        case createdAt = "created_at"
    }
}

/// Auth state change events.
public enum AuthStateEvent: String, Sendable {
    case initialSession = "INITIAL_SESSION"
    case signedIn = "SIGNED_IN"
    case signedOut = "SIGNED_OUT"
    case tokenRefreshed = "TOKEN_REFRESHED"
    case userUpdated = "USER_UPDATED"
}

/// Generic pagination parameters.
public struct PaginationParams: Sendable {
    public let limit: Int?
    public let offset: Int?
    
    public init(limit: Int? = nil, offset: Int? = nil) {
        self.limit = limit
        self.offset = offset
    }
}

/// Paginated response wrapper.
public struct PaginatedResponse<T: Sendable>: Sendable {
    public let data: [T]
    public let total: Int?
    public let limit: Int
    public let offset: Int
    
    public init(data: [T], total: Int?, limit: Int, offset: Int) {
        self.data = data
        self.total = total
        self.limit = limit
        self.offset = offset
    }
}

/// Type-erased Codable value for metadata fields.
public struct AnyCodable: Codable, Equatable, @unchecked Sendable {
    public let value: Any
    
    public init(_ value: Any) {
        self.value = value
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        
        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map { $0.value }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Unable to decode value")
        }
    }
    
    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        
        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            throw EncodingError.invalidValue(value, .init(codingPath: container.codingPath, debugDescription: "Unable to encode value"))
        }
    }
    
    public static func == (lhs: AnyCodable, rhs: AnyCodable) -> Bool {
        switch (lhs.value, rhs.value) {
        case is (NSNull, NSNull):
            return true
        case let (l as Bool, r as Bool):
            return l == r
        case let (l as Int, r as Int):
            return l == r
        case let (l as Double, r as Double):
            return l == r
        case let (l as String, r as String):
            return l == r
        case let (l as [Any], r as [Any]):
            return l.count == r.count && zip(l, r).allSatisfy { AnyCodable($0) == AnyCodable($1) }
        case let (l as [String: Any], r as [String: Any]):
            guard l.count == r.count else { return false }
            for (key, lValue) in l {
                guard let rValue = r[key], AnyCodable(lValue) == AnyCodable(rValue) else { return false }
            }
            return true
        default:
            return false
        }
    }
}
