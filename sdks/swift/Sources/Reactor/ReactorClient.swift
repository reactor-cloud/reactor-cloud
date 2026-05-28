import Foundation
import ReactorShared
import ReactorAuth
import ReactorData
import ReactorStorage
import ReactorFunctions
import ReactorJobs
import ReactorSites
import ReactorRealtime
import ReactorAnalytics
import ReactorAI

/// Options for creating a Reactor client.
public struct ReactorClientOptions: Sendable {
    /// Project key (public anon key)
    public let key: String
    
    /// Session store for persisting auth sessions
    public let sessionStore: SessionStore?
    
    /// Custom HTTP client (defaults to URLSession)
    public let httpClient: HTTPClient?
    
    /// Default request timeout
    public let timeout: TimeInterval?
    
    /// Default retry count
    public let retries: Int?
    
    /// Auto-refresh auth token
    public let autoRefreshToken: Bool
    
    /// Auto-identify user in analytics on auth state change
    public let autoIdentifyUser: Bool
    
    /// Analytics enabled
    public let analyticsEnabled: Bool
    
    public init(
        key: String,
        sessionStore: SessionStore? = nil,
        httpClient: HTTPClient? = nil,
        timeout: TimeInterval? = nil,
        retries: Int? = nil,
        autoRefreshToken: Bool = true,
        autoIdentifyUser: Bool = true,
        analyticsEnabled: Bool = true
    ) {
        self.key = key
        self.sessionStore = sessionStore
        self.httpClient = httpClient
        self.timeout = timeout
        self.retries = retries
        self.autoRefreshToken = autoRefreshToken
        self.autoIdentifyUser = autoIdentifyUser
        self.analyticsEnabled = analyticsEnabled
    }
}

/// The unified Reactor client providing access to all services.
public final class ReactorClient: @unchecked Sendable {
    /// The base URL of the Reactor instance
    public let url: URL
    
    /// Auth client for user authentication and session management
    public let auth: AuthClient
    
    /// Data client for PostgREST-style database queries
    public let data: DataClient
    
    /// Storage client for file management
    public let storage: StorageClient
    
    /// Functions client for serverless function invocation
    public let functions: FunctionsClient
    
    /// Jobs client for background job management
    public let jobs: JobsClient
    
    /// Sites client for static site deployment
    public let sites: SitesClient
    
    /// Realtime client for WebSocket subscriptions
    public let realtime: RealtimeClient
    
    /// Analytics client for event tracking
    public let analytics: AnalyticsClient
    
    /// AI client for chat completions and embeddings
    public let ai: AIClient
    
    private let options: ReactorClientOptions
    private var authStateSubscription: AuthStateSubscription?
    
    init(url: URL, options: ReactorClientOptions) {
        self.url = url
        self.options = options
        
        let httpClient = options.httpClient ?? URLSessionHTTPClient()
        
        // Create base context without auth
        let baseCtx = RequestContext(
            baseURL: url,
            projectKey: options.key,
            accessTokenProvider: nil,
            httpClient: httpClient,
            defaultTimeout: options.timeout ?? 30,
            defaultRetries: options.retries ?? 0
        )
        
        // Create auth client options
        let authOptions = AuthClientOptions(
            storage: options.sessionStore,
            autoRefresh: options.autoRefreshToken,
            persistSession: true
        )
        
        // Create auth client first
        let authClient = AuthClient(baseCtx, options: authOptions)
        self.auth = authClient
        
        // Create context with auth token provider
        let authedCtx = RequestContext(
            baseURL: url,
            projectKey: options.key,
            accessTokenProvider: { [weak authClient] in
                await authClient?.getSession()?.accessToken
            },
            httpClient: httpClient,
            defaultTimeout: options.timeout ?? 30,
            defaultRetries: options.retries ?? 0
        )
        
        // Create all other clients with auth context
        self.data = DataClient(authedCtx)
        self.storage = StorageClient(authedCtx)
        self.functions = FunctionsClient(authedCtx)
        self.jobs = JobsClient(authedCtx)
        self.sites = SitesClient(authedCtx)
        self.realtime = RealtimeClient(authedCtx)
        
        let analyticsClient = AnalyticsClient(authedCtx, enabled: options.analyticsEnabled)
        self.analytics = analyticsClient
        self.ai = AIClient(authedCtx)
        
        // Start analytics periodic flush in background
        if options.analyticsEnabled {
            Task {
                await analyticsClient.startPeriodicFlush()
            }
        }
        
        // Wire auto-identify on auth state change
        if options.autoIdentifyUser && options.analyticsEnabled {
            self.authStateSubscription = authClient.onAuthStateChange { [weak self] (event: AuthStateEvent, session: Session?) in
                Task { [weak self] in
                    guard let self = self else { return }
                    
                    switch event {
                    case .signedIn, .tokenRefreshed, .userUpdated, .initialSession:
                        if let user = session?.user {
                            await self.analytics.identify(
                                userId: user.id,
                                traits: [
                                    "email": AnyCodable(user.email),
                                    "created_at": AnyCodable(user.createdAt.description)
                                ]
                            )
                        }
                    case .signedOut:
                        await self.analytics.reset()
                    }
                }
            }
        }
    }
    
    deinit {
        authStateSubscription?.unsubscribe()
    }
    
    // MARK: - Convenience Methods
    
    /// Start a query builder for a table.
    ///
    /// Convenience method equivalent to `data.from(table, as: type)`.
    public func from<Row: Codable>(_ table: String, as type: Row.Type) -> QueryBuilder<Row> {
        data.from(table, as: type)
    }
    
    /// Get the current user session.
    public func getSession() async -> Session? {
        await auth.getSession()
    }
    
    /// Get the current user.
    public func getUser() async -> User? {
        try? await auth.getUser()
    }
}

/// Create a Reactor client.
///
/// - Parameters:
///   - url: Base URL of the Reactor instance
///   - options: Client options
/// - Returns: Configured ReactorClient
///
/// ## Example
///
/// ```swift
/// let reactor = createClient(
///     url: URL(string: "https://reactor.cloud")!,
///     options: .init(key: "rk_pub_your_key")
/// )
/// ```
public func createClient(url: URL, options: ReactorClientOptions) -> ReactorClient {
    ReactorClient(url: url, options: options)
}
