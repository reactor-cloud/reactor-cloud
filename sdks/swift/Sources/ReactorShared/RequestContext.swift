import Foundation

/// Request context shared across all SDK operations.
public struct RequestContext: Sendable {
    /// Base URL for API requests.
    public let baseURL: URL
    /// Project key (anon key).
    public let projectKey: String?
    /// Provider for current access token.
    public let accessTokenProvider: (@Sendable () async -> String?)?
    /// HTTP client for making requests.
    public let httpClient: HTTPClient
    /// Default headers to include in all requests.
    public let defaultHeaders: [String: String]
    /// Default timeout for requests.
    public let defaultTimeout: TimeInterval
    /// Default number of retries on 5xx/network errors.
    public let defaultRetries: Int
    
    public init(
        baseURL: URL,
        projectKey: String? = nil,
        accessTokenProvider: (@Sendable () async -> String?)? = nil,
        httpClient: HTTPClient = URLSessionHTTPClient(),
        defaultHeaders: [String: String] = [:],
        defaultTimeout: TimeInterval = 30,
        defaultRetries: Int = 3
    ) {
        self.baseURL = baseURL
        self.projectKey = projectKey
        self.accessTokenProvider = accessTokenProvider
        self.httpClient = httpClient
        self.defaultHeaders = defaultHeaders
        self.defaultTimeout = defaultTimeout
        self.defaultRetries = defaultRetries
    }
    
    /// Create a new context with an access token provider.
    public func with(accessTokenProvider: @escaping @Sendable () async -> String?) -> RequestContext {
        RequestContext(
            baseURL: baseURL,
            projectKey: projectKey,
            accessTokenProvider: accessTokenProvider,
            httpClient: httpClient,
            defaultHeaders: defaultHeaders,
            defaultTimeout: defaultTimeout,
            defaultRetries: defaultRetries
        )
    }
}

// MARK: - Request Helpers

/// Perform an HTTP request with SDK conventions.
public func request<T: Decodable>(
    _ ctx: RequestContext,
    path: String,
    method: HTTPMethod = .get,
    body: (any Encodable)? = nil,
    headers: [String: String] = [:],
    responseType: T.Type = T.self
) async throws -> T {
    let url = ctx.baseURL.appendingPathComponent(path)
    
    var allHeaders = [
        "Content-Type": "application/json",
        "Accept": "application/json",
        "X-Reactor-Client": "swift/\(ReactorSharedVersion)"
    ]
    
    allHeaders.merge(ctx.defaultHeaders) { _, new in new }
    allHeaders.merge(headers) { _, new in new }
    
    if let projectKey = ctx.projectKey {
        allHeaders["X-Reactor-Project-Key"] = projectKey
    }
    
    if let tokenProvider = ctx.accessTokenProvider {
        if let token = await tokenProvider() {
            allHeaders["Authorization"] = "Bearer \(token)"
        }
    }
    
    var bodyData: Data?
    if let body {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        bodyData = try encoder.encode(AnyEncodable(body))
    }
    
    let options = HTTPRequestOptions(
        method: method,
        headers: allHeaders,
        body: bodyData,
        timeout: ctx.defaultTimeout
    )
    
    let response = try await performWithRetry(ctx: ctx, url: url, options: options)
    
    guard response.isSuccess else {
        throw errorFromResponse(status: response.status, body: response.data)
    }
    
    if response.data.isEmpty {
        if let emptyResponse = EmptyResponse() as? T {
            return emptyResponse
        }
    }
    
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .custom { decoder in
        let container = try decoder.singleValueContainer()
        let string = try container.decode(String.self)
        
        let formatters = [
            ISO8601DateFormatter(),
            { () -> ISO8601DateFormatter in
                let f = ISO8601DateFormatter()
                f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
                return f
            }()
        ]
        
        for formatter in formatters {
            if let date = formatter.date(from: string) {
                return date
            }
        }
        
        throw DecodingError.dataCorruptedError(in: container, debugDescription: "Unable to parse date: \(string)")
    }
    
    return try decoder.decode(T.self, from: response.data)
}

/// Perform a request without expecting a response body.
public func requestVoid(
    _ ctx: RequestContext,
    path: String,
    method: HTTPMethod = .get,
    body: (any Encodable)? = nil,
    headers: [String: String] = [:]
) async throws {
    let _: EmptyResponse = try await request(ctx, path: path, method: method, body: body, headers: headers)
}

/// Empty response type for void operations.
public struct EmptyResponse: Decodable, Sendable {
    public init() {}
}

private func performWithRetry(
    ctx: RequestContext,
    url: URL,
    options: HTTPRequestOptions
) async throws -> HTTPResponse {
    var lastError: Error?
    
    for attempt in 0...ctx.defaultRetries {
        do {
            let response = try await ctx.httpClient.request(url, options: options)
            
            if response.isSuccess || !isRetryable(status: response.status) {
                return response
            }
            
            if attempt < ctx.defaultRetries {
                let delay = getBackoffDelay(attempt: attempt)
                try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            }
            
            lastError = errorFromResponse(status: response.status, body: response.data)
        } catch is CancellationError {
            throw ReactorError.cancelled
        } catch let error as ReactorError {
            throw error
        } catch let error as URLError where error.code == .timedOut {
            if attempt == ctx.defaultRetries {
                throw ReactorError.timeout
            }
            lastError = ReactorError.timeout
        } catch let error as URLError where error.code == .cancelled {
            throw ReactorError.cancelled
        } catch {
            if attempt == ctx.defaultRetries {
                throw ReactorError.network(message: error.localizedDescription, underlyingError: String(describing: error))
            }
            lastError = error
            let delay = getBackoffDelay(attempt: attempt)
            try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
        }
    }
    
    throw lastError ?? ReactorError.network(message: "Request failed", underlyingError: nil)
}

private func isRetryable(status: Int) -> Bool {
    status >= 500 || status == 429
}

private func getBackoffDelay(attempt: Int, baseMs: Double = 1000, maxMs: Double = 10000) -> TimeInterval {
    let delay = min(baseMs * pow(2.0, Double(attempt)), maxMs)
    let jitter = delay * (0.75 + Double.random(in: 0..<0.5))
    return jitter / 1000.0
}

/// Type-erased Encodable wrapper.
private struct AnyEncodable: Encodable {
    private let _encode: (Encoder) throws -> Void
    
    init(_ wrapped: any Encodable) {
        _encode = wrapped.encode(to:)
    }
    
    func encode(to encoder: Encoder) throws {
        try _encode(encoder)
    }
}
