import Foundation
import ReactorShared

/// Options for function invocation.
public struct InvokeOptions: Sendable {
    public var headers: [String: String]
    public var timeout: TimeInterval
    
    public init(headers: [String: String] = [:], timeout: TimeInterval = 30) {
        self.headers = headers
        self.timeout = timeout
    }
}

/// Functions client for serverless function invocation.
public final class FunctionsClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    /// Create a FunctionsClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Invoke a function with typed request and response.
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - body: Request body
    ///   - options: Invoke options
    /// - Returns: Function response
    public func invoke<Request: Encodable, Response: Decodable>(
        _ name: String,
        body: Request,
        options: InvokeOptions = .init()
    ) async throws -> Response {
        var headers = options.headers
        headers["Content-Type"] = "application/json"
        
        return try await request(
            ctx,
            path: "functions/v1/\(name)",
            method: .post,
            body: body,
            headers: headers
        )
    }
    
    /// Invoke a function without a request body.
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - options: Invoke options
    /// - Returns: Function response
    public func invoke<Response: Decodable>(
        _ name: String,
        options: InvokeOptions = .init()
    ) async throws -> Response {
        try await request(
            ctx,
            path: "functions/v1/\(name)",
            method: .post,
            headers: options.headers
        )
    }
    
    /// Invoke a function and return raw data.
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - body: Request body (optional)
    ///   - options: Invoke options
    /// - Returns: Raw response data
    public func invokeRaw(
        _ name: String,
        body: Data? = nil,
        options: InvokeOptions = .init()
    ) async throws -> Data {
        let url = ctx.baseURL.appendingPathComponent("functions/v1/\(name)")
        
        var headers = options.headers
        headers["Content-Type"] = "application/json"
        if let projectKey = ctx.projectKey {
            headers["X-Reactor-Project-Key"] = projectKey
        }
        if let tokenProvider = ctx.accessTokenProvider, let token = await tokenProvider() {
            headers["Authorization"] = "Bearer \(token)"
        }
        
        let httpOptions = HTTPRequestOptions(method: .post, headers: headers, body: body, timeout: options.timeout)
        let response = try await ctx.httpClient.request(url, options: httpOptions)
        
        guard response.isSuccess else {
            throw errorFromResponse(status: response.status, body: response.data)
        }
        
        return response.data
    }
    
    /// Invoke a function with SSE streaming response.
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - body: Request body (optional)
    ///   - options: Invoke options
    /// - Returns: Async stream of SSE events
    public func invokeStream(
        _ name: String,
        body: Data? = nil,
        options: InvokeOptions = .init()
    ) -> AsyncThrowingStream<SSEEvent, Error> {
        let url = ctx.baseURL.appendingPathComponent("functions/v1/\(name)")
        
        var headers = options.headers
        headers["Accept"] = "text/event-stream"
        if let projectKey = ctx.projectKey {
            headers["X-Reactor-Project-Key"] = projectKey
        }
        
        let httpOptions = HTTPRequestOptions(method: .post, headers: headers, body: body, timeout: options.timeout)
        let dataStream = ctx.httpClient.stream(url, options: httpOptions)
        
        return AsyncThrowingStream { continuation in
            Task {
                var buffer = Data()
                
                do {
                    for try await chunk in dataStream {
                        buffer.append(chunk)
                        
                        while let lineEnd = buffer.firstIndex(of: UInt8(ascii: "\n")) {
                            let lineData = buffer[..<lineEnd]
                            buffer = buffer[(buffer.index(after: lineEnd))...]
                            
                            if let line = String(data: Data(lineData), encoding: .utf8) {
                                if let event = parseSSELine(line) {
                                    continuation.yield(event)
                                }
                            }
                        }
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }
}

/// Server-Sent Event.
public struct SSEEvent: Sendable {
    public let event: String?
    public let data: String
    public let id: String?
    
    public init(event: String? = nil, data: String, id: String? = nil) {
        self.event = event
        self.data = data
        self.id = id
    }
}

private func parseSSELine(_ line: String) -> SSEEvent? {
    guard line.hasPrefix("data:") else { return nil }
    let data = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
    return SSEEvent(data: data)
}
