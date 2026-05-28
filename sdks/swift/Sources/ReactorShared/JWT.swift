import Foundation

/// Decoded JWT payload structure for Reactor tokens.
public struct JWTPayload: Codable, Sendable {
    /// Subject (user ID).
    public let sub: String
    /// Email address.
    public let email: String?
    /// Whether email is verified.
    public let emailVerified: Bool?
    /// Expiration timestamp (seconds since epoch).
    public let exp: Int
    /// Issued at timestamp (seconds since epoch).
    public let iat: Int
    /// Issuer.
    public let iss: String?
    /// Audience.
    public let aud: AudienceClaim?
    /// Organization memberships.
    public let orgs: [OrgClaim]?
    /// User metadata.
    public let metadata: [String: AnyCodable]?
    
    public struct OrgClaim: Codable, Sendable {
        public let id: String
        public let slug: String
        public let roleId: String
        public let permissions: [String]
        
        enum CodingKeys: String, CodingKey {
            case id, slug, permissions
            case roleId = "role_id"
        }
    }
    
    public enum AudienceClaim: Codable, Sendable {
        case single(String)
        case multiple([String])
        
        public init(from decoder: Decoder) throws {
            let container = try decoder.singleValueContainer()
            if let single = try? container.decode(String.self) {
                self = .single(single)
            } else if let multiple = try? container.decode([String].self) {
                self = .multiple(multiple)
            } else {
                throw DecodingError.typeMismatch(
                    AudienceClaim.self,
                    .init(codingPath: decoder.codingPath, debugDescription: "Expected string or array of strings")
                )
            }
        }
        
        public func encode(to encoder: Encoder) throws {
            var container = encoder.singleValueContainer()
            switch self {
            case .single(let s):
                try container.encode(s)
            case .multiple(let arr):
                try container.encode(arr)
            }
        }
    }
    
    enum CodingKeys: String, CodingKey {
        case sub, email, exp, iat, iss, aud, orgs, metadata
        case emailVerified = "email_verified"
    }
}

/// Decode a JWT without verifying the signature.
/// For client-side use only - server should always verify.
///
/// - Parameter token: JWT string
/// - Returns: Decoded payload or nil if invalid
public func decodeJWT(_ token: String) -> JWTPayload? {
    let parts = token.split(separator: ".")
    guard parts.count == 3 else { return nil }
    
    let payload = String(parts[1])
    guard let data = base64URLDecode(payload) else { return nil }
    
    let decoder = JSONDecoder()
    return try? decoder.decode(JWTPayload.self, from: data)
}

/// Check if a JWT is expired.
///
/// - Parameters:
///   - token: JWT string or decoded payload
///   - bufferSeconds: Seconds before actual expiry to consider it expired (default: 0)
/// - Returns: True if expired or will expire within buffer
public func isJWTExpired(_ token: String, bufferSeconds: Int = 0) -> Bool {
    guard let payload = decodeJWT(token) else { return true }
    return isJWTExpired(payload, bufferSeconds: bufferSeconds)
}

/// Check if a JWT payload is expired.
public func isJWTExpired(_ payload: JWTPayload, bufferSeconds: Int = 0) -> Bool {
    let now = Int(Date().timeIntervalSince1970)
    return payload.exp <= now + bufferSeconds
}

/// Get the expiration date of a JWT.
///
/// - Parameter token: JWT string
/// - Returns: Date object or nil if invalid
public func getJWTExpiry(_ token: String) -> Date? {
    guard let payload = decodeJWT(token) else { return nil }
    return Date(timeIntervalSince1970: TimeInterval(payload.exp))
}

/// Get seconds until JWT expiration.
///
/// - Parameter token: JWT string
/// - Returns: Seconds remaining (negative if expired) or nil if invalid
public func getJWTTimeRemaining(_ token: String) -> Int? {
    guard let payload = decodeJWT(token) else { return nil }
    let now = Int(Date().timeIntervalSince1970)
    return payload.exp - now
}

/// Get seconds until JWT expiration from payload.
public func getJWTTimeRemaining(_ payload: JWTPayload) -> Int {
    let now = Int(Date().timeIntervalSince1970)
    return payload.exp - now
}

// MARK: - Base64URL Helpers

private func base64URLDecode(_ string: String) -> Data? {
    var base64 = string
        .replacingOccurrences(of: "-", with: "+")
        .replacingOccurrences(of: "_", with: "/")
    
    let paddingLength = (4 - base64.count % 4) % 4
    base64 += String(repeating: "=", count: paddingLength)
    
    return Data(base64Encoded: base64)
}
