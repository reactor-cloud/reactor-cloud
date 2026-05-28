import Foundation
import ReactorShared

/// Options for configuring the AuthClient.
public struct AuthClientOptions: Sendable {
    /// Storage adapter for session persistence.
    public var storage: SessionStore?
    /// Storage key for the session.
    public var storageKey: String
    /// Whether to automatically refresh tokens.
    public var autoRefresh: Bool
    /// Whether to persist sessions.
    public var persistSession: Bool
    
    public init(
        storage: SessionStore? = nil,
        storageKey: String = defaultSessionKey,
        autoRefresh: Bool = true,
        persistSession: Bool = true
    ) {
        self.storage = storage
        self.storageKey = storageKey
        self.autoRefresh = autoRefresh
        self.persistSession = persistSession
    }
}

/// Parameters for signing up.
public struct SignUpParams: Sendable {
    public let email: String
    public let password: String
    public let metadata: [String: AnyCodable]?
    
    public init(email: String, password: String, metadata: [String: AnyCodable]? = nil) {
        self.email = email
        self.password = password
        self.metadata = metadata
    }
}

/// Parameters for signing in.
public struct SignInParams: Sendable {
    public let email: String
    public let password: String
    
    public init(email: String, password: String) {
        self.email = email
        self.password = password
    }
}

/// Parameters for updating user.
public struct UpdateUserParams: Encodable, Sendable {
    public let email: String?
    public let password: String?
    public let metadata: [String: AnyCodable]?
    
    public init(email: String? = nil, password: String? = nil, metadata: [String: AnyCodable]? = nil) {
        self.email = email
        self.password = password
        self.metadata = metadata
    }
}

/// Authentication client for Reactor.
///
/// Handles user authentication, session management, token refresh, and state broadcasting.
public final class AuthClient: @unchecked Sendable {
    /// The request context used for API calls.
    public let ctx: RequestContext
    private let stateManager: AuthStateManager
    private var initialized = false
    private var initTask: Task<Void, Never>?
    
    /// Organizations client.
    public let orgs: OrgsClient
    /// API keys client.
    public let apiKeys: ApiKeysClient
    
    /// Create an AuthClient.
    ///
    /// - Parameters:
    ///   - ctx: Request context for API calls
    ///   - options: Configuration options
    public init(_ ctx: RequestContext, options: AuthClientOptions = .init()) {
        self.ctx = ctx
        
        #if canImport(Security)
        let storage = options.storage ?? KeychainSessionStore()
        #else
        let storage = options.storage ?? InMemorySessionStore()
        #endif
        
        self.stateManager = AuthStateManager(
            storage: options.persistSession ? storage : nil,
            storageKey: options.storageKey,
            autoRefresh: options.autoRefresh,
            persistSession: options.persistSession
        )
        
        self.orgs = OrgsClient(ctx)
        self.apiKeys = ApiKeysClient(ctx)
        
        Task {
            await self.stateManager.setRefreshCallback { [weak self] in
                guard let self else { return nil }
                return try await self.refreshSessionInternal()
            }
        }
    }
    
    /// Initialize the auth client.
    /// Loads session from storage.
    public func initialize() async {
        guard !initialized else { return }
        
        if let initTask {
            await initTask.value
            return
        }
        
        let task = Task {
            _ = await stateManager.initialize()
            initialized = true
        }
        
        initTask = task
        await task.value
    }
    
    /// Sign up a new user.
    ///
    /// - Parameter params: Sign up parameters
    /// - Returns: The created user and session
    public func signUp(_ params: SignUpParams) async throws -> (user: User, session: Session) {
        struct SignUpRequest: Encodable {
            let email: String
            let password: String
            let metadata: [String: AnyCodable]
        }
        
        struct SignUpResponse: Decodable {
            let user: User
            let session: SessionData
            
            struct SessionData: Decodable {
                let access_token: String
                let refresh_token: String
                let expires_at: Date
            }
        }
        
        let response: SignUpResponse = try await request(
            ctx,
            path: "/auth/v1/signup",
            method: .post,
            body: SignUpRequest(
                email: params.email,
                password: params.password,
                metadata: params.metadata ?? [:]
            )
        )
        
        let session = Session(
            accessToken: response.session.access_token,
            refreshToken: response.session.refresh_token,
            expiresAt: response.session.expires_at,
            user: response.user
        )
        
        await stateManager.setSession(session, event: .signedIn)
        
        return (response.user, session)
    }
    
    /// Sign in with email and password.
    ///
    /// - Parameter params: Sign in parameters
    /// - Returns: The user and session
    public func signIn(_ params: SignInParams) async throws -> (user: User, session: Session) {
        struct LoginRequest: Encodable {
            let email: String
            let password: String
        }
        
        struct LoginResponse: Decodable {
            let user: User
            let access_token: String
            let refresh_token: String
            let expires_at: Date
        }
        
        let response: LoginResponse = try await request(
            ctx,
            path: "/auth/v1/login",
            method: .post,
            body: LoginRequest(email: params.email, password: params.password)
        )
        
        let session = Session(
            accessToken: response.access_token,
            refreshToken: response.refresh_token,
            expiresAt: response.expires_at,
            user: response.user
        )
        
        await stateManager.setSession(session, event: .signedIn)
        
        return (response.user, session)
    }
    
    /// Sign in with email and password (convenience).
    public func signIn(email: String, password: String) async throws -> (user: User, session: Session) {
        try await signIn(SignInParams(email: email, password: password))
    }
    
    /// Sign out the current user.
    public func signOut() async throws {
        let session = await stateManager.getSession()
        
        if let session {
            struct LogoutRequest: Encodable {
                let refresh_token: String
            }
            
            _ = try? await request(
                ctx,
                path: "/auth/v1/logout",
                method: .post,
                body: LogoutRequest(refresh_token: session.refreshToken),
                responseType: EmptyResponse.self
            )
        }
        
        await stateManager.clearSession()
    }
    
    /// Get the current session.
    public func getSession() async -> Session? {
        await initialize()
        return await stateManager.getSession()
    }
    
    /// Get the current user.
    public func getUser() async throws -> User? {
        await initialize()
        
        guard await stateManager.getSession() != nil else {
            return nil
        }
        
        if let cachedUser = await stateManager.getUser() {
            return cachedUser
        }
        
        return try await request(ctx, path: "/auth/v1/user")
    }
    
    /// Get the current user (convenience).
    public var currentUser: User? {
        get async {
            await stateManager.getUser()
        }
    }
    
    /// Update the current user's profile.
    ///
    /// - Parameter params: Update parameters
    /// - Returns: The updated user
    public func updateUser(_ params: UpdateUserParams) async throws -> User {
        let user: User = try await request(ctx, path: "/auth/v1/user", method: .patch, body: params)
        
        if let session = await stateManager.getSession() {
            let updatedSession = Session(
                accessToken: session.accessToken,
                refreshToken: session.refreshToken,
                expiresAt: session.expiresAt,
                user: user
            )
            await stateManager.setSession(updatedSession, event: .userUpdated)
        }
        
        return user
    }
    
    /// Verify email with a token.
    ///
    /// - Parameter token: Verification token
    public func verifyEmail(token: String) async throws {
        struct VerifyResponse: Decodable {
            let verified: Bool
        }
        
        let _: VerifyResponse = try await request(
            ctx,
            path: "/auth/v1/verify?token=\(token.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? token)"
        )
        
        if let session = await stateManager.getSession() {
            let user: User = try await request(ctx, path: "/auth/v1/user")
            let updatedSession = Session(
                accessToken: session.accessToken,
                refreshToken: session.refreshToken,
                expiresAt: session.expiresAt,
                user: user
            )
            await stateManager.setSession(updatedSession, event: .userUpdated)
        }
    }
    
    /// Resend email verification.
    ///
    /// - Parameter email: Email address
    public func resendVerification(email: String) async throws {
        struct ResendRequest: Encodable {
            let email: String
        }
        
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/resend", method: .post, body: ResendRequest(email: email))
    }
    
    /// Request a password reset email.
    ///
    /// - Parameter email: Email address
    public func requestPasswordReset(email: String) async throws {
        struct ResetRequest: Encodable {
            let email: String
        }
        
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/password-reset/request", method: .post, body: ResetRequest(email: email))
    }
    
    /// Confirm password reset with token and new password.
    ///
    /// - Parameters:
    ///   - token: Reset token
    ///   - newPassword: New password
    public func confirmPasswordReset(token: String, newPassword: String) async throws {
        struct ConfirmRequest: Encodable {
            let token: String
            let password: String
        }
        
        let _: EmptyResponse = try await request(
            ctx,
            path: "/auth/v1/password-reset/confirm",
            method: .post,
            body: ConfirmRequest(token: token, password: newPassword)
        )
    }
    
    /// Send a magic link to the email address.
    ///
    /// - Parameters:
    ///   - email: Email address
    ///   - redirectTo: Optional redirect URL after sign-in
    public func signInWithMagicLink(email: String, redirectTo: URL? = nil) async throws {
        struct MagicLinkRequest: Encodable {
            let email: String
            let redirect_to: String?
        }
        
        let _: EmptyResponse = try await request(
            ctx,
            path: "/auth/v1/magiclink",
            method: .post,
            body: MagicLinkRequest(email: email, redirect_to: redirectTo?.absoluteString)
        )
    }
    
    /// Verify an OTP (magic link token).
    ///
    /// - Parameters:
    ///   - email: Email address
    ///   - token: OTP token
    /// - Returns: The session
    public func verifyOtp(email: String, token: String) async throws -> Session {
        struct VerifyOtpRequest: Encodable {
            let email: String
            let token: String
        }
        
        struct VerifyOtpResponse: Decodable {
            let user: User
            let access_token: String
            let refresh_token: String
            let expires_at: Date
        }
        
        let response: VerifyOtpResponse = try await request(
            ctx,
            path: "/auth/v1/verify-otp",
            method: .post,
            body: VerifyOtpRequest(email: email, token: token)
        )
        
        let session = Session(
            accessToken: response.access_token,
            refreshToken: response.refresh_token,
            expiresAt: response.expires_at,
            user: response.user
        )
        
        await stateManager.setSession(session, event: .signedIn)
        
        return session
    }
    
    /// Manually refresh the session.
    ///
    /// - Returns: The refreshed session
    public func refreshSession() async throws -> Session {
        guard let session = await stateManager.refresh() else {
            throw ReactorError.auth(.sessionExpired(message: "Failed to refresh session"))
        }
        return session
    }
    
    private func refreshSessionInternal() async throws -> Session? {
        guard let currentSession = await stateManager.getSession() else {
            return nil
        }
        
        struct TokenRequest: Encodable {
            let grant_type: String
            let refresh_token: String
        }
        
        struct TokenResponse: Decodable {
            let access_token: String
            let refresh_token: String
            let expires_at: Date
        }
        
        let response: TokenResponse = try await request(
            ctx,
            path: "/auth/v1/token",
            method: .post,
            body: TokenRequest(grant_type: "refresh_token", refresh_token: currentSession.refreshToken)
        )
        
        let authenticatedCtx = RequestContext(
            baseURL: ctx.baseURL,
            projectKey: ctx.projectKey,
            accessTokenProvider: { response.access_token },
            httpClient: ctx.httpClient,
            defaultHeaders: ctx.defaultHeaders
        )
        
        let user: User = try await request(authenticatedCtx, path: "/auth/v1/user")
        
        return Session(
            accessToken: response.access_token,
            refreshToken: response.refresh_token,
            expiresAt: response.expires_at,
            user: user
        )
    }
    
    /// Set the session manually (for SSR scenarios).
    ///
    /// - Parameter session: The session to set
    public func setSession(_ session: Session) async {
        await stateManager.setSession(session, event: .signedIn)
    }
    
    /// Subscribe to auth state changes.
    ///
    /// - Returns: An async stream of (event, session) tuples
    public func authStateChanges() async -> AsyncStream<(AuthStateEvent, Session?)> {
        await stateManager.authStateChanges()
    }
    
    /// Subscribe to auth state changes with a callback (legacy API).
    ///
    /// - Parameter callback: Called when auth state changes
    /// - Returns: A subscription handle
    public func onAuthStateChange(
        _ callback: @escaping @Sendable (AuthStateEvent, Session?) -> Void
    ) -> AuthStateSubscription {
        stateManager.onAuthStateChange(callback)
    }
    
    /// Get the access token for making authenticated requests.
    public func getAccessToken() async -> String? {
        await stateManager.getAccessToken()
    }
    
    /// Delete the current user's account.
    public func deleteUser() async throws {
        let _: EmptyResponse = try await request(ctx, path: "/auth/v1/user", method: .delete)
        await stateManager.clearSession()
    }
}
