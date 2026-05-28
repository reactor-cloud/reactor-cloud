import Foundation

/// HTTP method.
public enum HTTPMethod: String, Sendable {
    case get = "GET"
    case post = "POST"
    case put = "PUT"
    case patch = "PATCH"
    case delete = "DELETE"
}

/// HTTP request options.
public struct HTTPRequestOptions: Sendable {
    public var method: HTTPMethod
    public var headers: [String: String]
    public var body: Data?
    public var timeout: TimeInterval
    
    public init(
        method: HTTPMethod = .get,
        headers: [String: String] = [:],
        body: Data? = nil,
        timeout: TimeInterval = 30
    ) {
        self.method = method
        self.headers = headers
        self.body = body
        self.timeout = timeout
    }
}

/// HTTP response.
public struct HTTPResponse: Sendable {
    public let status: Int
    public let headers: [String: String]
    public let data: Data
    
    public init(status: Int, headers: [String: String], data: Data) {
        self.status = status
        self.headers = headers
        self.data = data
    }
    
    public var isSuccess: Bool {
        (200..<300).contains(status)
    }
}

/// Protocol for HTTP transport.
public protocol HTTPClient: Sendable {
    func request(_ url: URL, options: HTTPRequestOptions) async throws -> HTTPResponse
    func stream(_ url: URL, options: HTTPRequestOptions) -> AsyncThrowingStream<Data, Error>
}

/// URLSession-based HTTP client implementation.
public final class URLSessionHTTPClient: HTTPClient, @unchecked Sendable {
    private let session: URLSession
    
    public init(session: URLSession = .shared) {
        self.session = session
    }
    
    public func request(_ url: URL, options: HTTPRequestOptions) async throws -> HTTPResponse {
        var request = URLRequest(url: url)
        request.httpMethod = options.method.rawValue
        request.timeoutInterval = options.timeout
        
        for (key, value) in options.headers {
            request.setValue(value, forHTTPHeaderField: key)
        }
        
        if let body = options.body {
            request.httpBody = body
        }
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw ReactorError.network(message: "Invalid response type", underlyingError: nil)
        }
        
        var headers: [String: String] = [:]
        for (key, value) in httpResponse.allHeaderFields {
            if let key = key as? String, let value = value as? String {
                headers[key] = value
            }
        }
        
        return HTTPResponse(
            status: httpResponse.statusCode,
            headers: headers,
            data: data
        )
    }
    
    public func stream(_ url: URL, options: HTTPRequestOptions) -> AsyncThrowingStream<Data, Error> {
        AsyncThrowingStream { continuation in
            Task {
                var request = URLRequest(url: url)
                request.httpMethod = options.method.rawValue
                request.timeoutInterval = options.timeout
                
                for (key, value) in options.headers {
                    request.setValue(value, forHTTPHeaderField: key)
                }
                
                if let body = options.body {
                    request.httpBody = body
                }
                
                do {
                    let (bytes, response) = try await session.bytes(for: request)
                    
                    guard let httpResponse = response as? HTTPURLResponse else {
                        continuation.finish(throwing: ReactorError.network(message: "Invalid response type", underlyingError: nil))
                        return
                    }
                    
                    guard httpResponse.statusCode >= 200 && httpResponse.statusCode < 300 else {
                        var data = Data()
                        for try await byte in bytes {
                            data.append(byte)
                        }
                        let error = errorFromResponse(status: httpResponse.statusCode, body: data)
                        continuation.finish(throwing: error)
                        return
                    }
                    
                    for try await byte in bytes {
                        continuation.yield(Data([byte]))
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish(throwing: ReactorError.cancelled)
                } catch let error as URLError where error.code == .timedOut {
                    continuation.finish(throwing: ReactorError.timeout)
                } catch let error as URLError where error.code == .cancelled {
                    continuation.finish(throwing: ReactorError.cancelled)
                } catch {
                    continuation.finish(throwing: ReactorError.network(
                        message: error.localizedDescription,
                        underlyingError: String(describing: error)
                    ))
                }
            }
        }
    }
}
