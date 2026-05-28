import Foundation
import ReactorShared

/// Token types that can be detected from URLs.
public enum DetectedTokenType: String, Sendable {
    case verify
    case passwordReset = "password_reset"
    case oauth
    case invite
    case pkce
}

/// Result of URL token detection.
public struct DetectedToken: Sendable {
    public let type: DetectedTokenType
    public let token: String
    public let params: [String: String]
    
    public init(type: DetectedTokenType, token: String, params: [String: String] = [:]) {
        self.type = type
        self.token = token
        self.params = params
    }
}

/// Detect session or verification tokens from a URL.
///
/// Supports:
/// - Query parameter: ?token=... (email verification, password reset)
/// - Query parameter: ?invite_token=... (invitation acceptance)
/// - Query parameter: ?code=... (PKCE OAuth)
/// - Hash fragment: #access_token=...&refresh_token=... (implicit OAuth)
///
/// - Parameter url: The URL to detect tokens from
/// - Returns: Detected token info, or nil if no token found
public func detectTokenInURL(_ url: URL) -> DetectedToken? {
    guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
        return nil
    }
    
    let queryItems = components.queryItems ?? []
    let queryParams = Dictionary(uniqueKeysWithValues: queryItems.compactMap { item in
        item.value.map { (item.name, $0) }
    })
    
    if let token = queryParams["token"] {
        let tokenType = queryParams["type"]
        if tokenType == "password_reset" {
            return DetectedToken(type: .passwordReset, token: token)
        }
        return DetectedToken(type: .verify, token: token)
    }
    
    if let inviteToken = queryParams["invite_token"] {
        return DetectedToken(type: .invite, token: inviteToken)
    }
    
    if let code = queryParams["code"] {
        var params = queryParams
        params["code"] = code
        return DetectedToken(type: .pkce, token: code, params: params)
    }
    
    if let fragment = components.fragment, !fragment.isEmpty {
        let fragmentParams = parseFragment(fragment)
        
        if let accessToken = fragmentParams["access_token"],
           let refreshToken = fragmentParams["refresh_token"] {
            return DetectedToken(
                type: .oauth,
                token: accessToken,
                params: [
                    "access_token": accessToken,
                    "refresh_token": refreshToken,
                    "expires_at": fragmentParams["expires_at"] ?? ""
                ]
            )
        }
    }
    
    return nil
}

/// Parse a URL fragment (hash) into key-value pairs.
private func parseFragment(_ fragment: String) -> [String: String] {
    var result: [String: String] = [:]
    let pairs = fragment.split(separator: "&")
    
    for pair in pairs {
        let components = pair.split(separator: "=", maxSplits: 1)
        if components.count == 2 {
            let key = String(components[0])
            let value = String(components[1]).removingPercentEncoding ?? String(components[1])
            result[key] = value
        }
    }
    
    return result
}

/// URL callback handler for deep links / Universal Links.
public struct URLCallbackHandler {
    private let authClient: AuthClient
    
    public init(authClient: AuthClient) {
        self.authClient = authClient
    }
    
    /// Handle a callback URL (deep link / Universal Link).
    ///
    /// Detects the token type and performs the appropriate action:
    /// - verify: Verifies the email
    /// - password_reset: Returns the token for use with confirmPasswordReset
    /// - oauth: Exchanges tokens and signs in
    /// - pkce: Exchanges code for tokens and signs in
    /// - invite: Returns the token for use with acceptInvitation
    ///
    /// - Parameter url: The callback URL
    /// - Returns: The resulting session (for oauth/pkce) or the detected token (for others)
    @discardableResult
    public func handleCallback(_ url: URL) async throws -> HandleCallbackResult {
        guard let detected = detectTokenInURL(url) else {
            throw ReactorError.validation(code: "invalid_callback", message: "No token found in URL", issues: [])
        }
        
        switch detected.type {
        case .verify:
            try await authClient.verifyEmail(token: detected.token)
            return .verified
            
        case .passwordReset:
            return .passwordResetToken(detected.token)
            
        case .oauth:
            guard let accessToken = detected.params["access_token"],
                  let refreshToken = detected.params["refresh_token"] else {
                throw ReactorError.validation(code: "invalid_oauth", message: "Missing OAuth tokens", issues: [])
            }
            
            let session = try await exchangeOAuthTokens(
                accessToken: accessToken,
                refreshToken: refreshToken,
                expiresAt: detected.params["expires_at"]
            )
            return .session(session)
            
        case .pkce:
            guard let code = detected.params["code"] else {
                throw ReactorError.validation(code: "invalid_pkce", message: "Missing PKCE code", issues: [])
            }
            let session = try await exchangePKCECode(code)
            return .session(session)
            
        case .invite:
            return .inviteToken(detected.token)
        }
    }
    
    private func exchangeOAuthTokens(
        accessToken: String,
        refreshToken: String,
        expiresAt: String?
    ) async throws -> Session {
        struct UserResponse: Decodable {
            let id: String
            let email: String
            let email_verified: Bool
            let metadata: [String: AnyCodable]
            let created_at: Date
        }
        
        let ctx = RequestContext(
            baseURL: authClient.ctx.baseURL,
            projectKey: authClient.ctx.projectKey,
            accessTokenProvider: { accessToken },
            httpClient: authClient.ctx.httpClient
        )
        
        let userResponse: UserResponse = try await request(ctx, path: "/auth/v1/user")
        
        let user = User(
            id: userResponse.id,
            email: userResponse.email,
            emailVerified: userResponse.email_verified,
            metadata: userResponse.metadata,
            createdAt: userResponse.created_at
        )
        
        let expiry: Date
        if let expiresAt, let date = ISO8601DateFormatter().date(from: expiresAt) {
            expiry = date
        } else {
            expiry = Date().addingTimeInterval(3600)
        }
        
        let session = Session(
            accessToken: accessToken,
            refreshToken: refreshToken,
            expiresAt: expiry,
            user: user
        )
        
        await authClient.setSession(session)
        
        return session
    }
    
    private func exchangePKCECode(_ code: String) async throws -> Session {
        struct TokenRequest: Encodable {
            let grant_type: String
            let code: String
        }
        
        struct TokenResponse: Decodable {
            let access_token: String
            let refresh_token: String
            let expires_at: Date
            let user: User
        }
        
        let response: TokenResponse = try await request(
            authClient.ctx,
            path: "/auth/v1/token",
            method: .post,
            body: TokenRequest(grant_type: "authorization_code", code: code)
        )
        
        let session = Session(
            accessToken: response.access_token,
            refreshToken: response.refresh_token,
            expiresAt: response.expires_at,
            user: response.user
        )
        
        await authClient.setSession(session)
        
        return session
    }
}

/// Result of handling a callback URL.
public enum HandleCallbackResult: Sendable {
    /// Email was verified.
    case verified
    /// Session was established (OAuth or PKCE).
    case session(Session)
    /// Password reset token was extracted (use with confirmPasswordReset).
    case passwordResetToken(String)
    /// Invitation token was extracted (use with orgs.acceptInvitation).
    case inviteToken(String)
}

