import Foundation

/// Base error type for all Reactor SDK errors.
public enum ReactorError: Error, Sendable, Equatable {
    /// Authentication/authorization errors (401).
    case auth(AuthError)
    /// Forbidden error (403).
    case forbidden(code: String, message: String, hint: String?)
    /// Validation errors (400, 422).
    case validation(code: String, message: String, issues: [ValidationIssue])
    /// Resource not found (404).
    case notFound(code: String, message: String, hint: String?)
    /// Conflict error (409).
    case conflict(code: String, message: String, hint: String?)
    /// Rate limit exceeded (429).
    case rateLimit(code: String, message: String, retryAfter: TimeInterval?)
    /// Server error (5xx).
    case server(code: String, message: String, status: Int)
    /// Network/connection error.
    case network(message: String, underlyingError: String?)
    /// Request was cancelled.
    case cancelled
    /// Request timed out.
    case timeout
    /// Unknown error.
    case unknown(code: String, message: String, status: Int)
}

/// Authentication-specific errors.
public enum AuthError: Error, Sendable, Equatable {
    case invalidCredentials(message: String)
    case sessionExpired(message: String)
    case invalidToken(message: String)
    case userNotFound(message: String)
    case emailNotConfirmed(message: String)
    case weakPassword(message: String)
    case emailAlreadyExists(message: String)
    case other(code: String, message: String, hint: String?)
}

/// Validation issue for a specific field.
public struct ValidationIssue: Sendable, Equatable, Codable {
    public let field: String
    public let messages: [String]
    
    public init(field: String, messages: [String]) {
        self.field = field
        self.messages = messages
    }
}

/// Server error response envelope.
public struct ErrorEnvelope: Decodable, Sendable {
    public let error: ErrorBody
    
    public struct ErrorBody: Decodable, Sendable {
        public let code: String
        public let message: String
        public let status: Int?
        public let hint: String?
        public let fields: [String: [String]]?
    }
}

extension ReactorError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .auth(let authError):
            return authError.localizedDescription
        case .forbidden(_, let message, _):
            return message
        case .validation(_, let message, _):
            return message
        case .notFound(_, let message, _):
            return message
        case .conflict(_, let message, _):
            return message
        case .rateLimit(_, let message, _):
            return message
        case .server(_, let message, _):
            return message
        case .network(let message, _):
            return message
        case .cancelled:
            return "Request was cancelled"
        case .timeout:
            return "Request timed out"
        case .unknown(_, let message, _):
            return message
        }
    }
}

extension AuthError: LocalizedError {
    public var localizedDescription: String {
        switch self {
        case .invalidCredentials(let message):
            return message
        case .sessionExpired(let message):
            return message
        case .invalidToken(let message):
            return message
        case .userNotFound(let message):
            return message
        case .emailNotConfirmed(let message):
            return message
        case .weakPassword(let message):
            return message
        case .emailAlreadyExists(let message):
            return message
        case .other(_, let message, _):
            return message
        }
    }
}

/// Create an appropriate error from an HTTP response status and body.
public func errorFromResponse(status: Int, body: Data) -> ReactorError {
    let decoder = JSONDecoder()
    
    var code = "unknown"
    var message = "An unknown error occurred"
    var hint: String?
    var fields: [String: [String]]?
    
    if let envelope = try? decoder.decode(ErrorEnvelope.self, from: body) {
        code = envelope.error.code
        message = envelope.error.message
        hint = envelope.error.hint
        fields = envelope.error.fields
    } else if let text = String(data: body, encoding: .utf8), !text.isEmpty {
        message = text
    }
    
    switch status {
    case 400, 422:
        let issues = (fields ?? [:]).map { ValidationIssue(field: $0.key, messages: $0.value) }
        return .validation(code: code, message: message, issues: issues)
    case 401:
        return .auth(authErrorFromCode(code, message: message, hint: hint))
    case 403:
        return .forbidden(code: code, message: message, hint: hint)
    case 404:
        return .notFound(code: code, message: message, hint: hint)
    case 409:
        return .conflict(code: code, message: message, hint: hint)
    case 429:
        return .rateLimit(code: code, message: message, retryAfter: nil)
    default:
        if status >= 500 {
            return .server(code: code, message: message, status: status)
        }
        return .unknown(code: code, message: message, status: status)
    }
}

private func authErrorFromCode(_ code: String, message: String, hint: String?) -> AuthError {
    switch code {
    case "invalid_credentials":
        return .invalidCredentials(message: message)
    case "session_expired":
        return .sessionExpired(message: message)
    case "invalid_token":
        return .invalidToken(message: message)
    case "user_not_found":
        return .userNotFound(message: message)
    case "email_not_confirmed":
        return .emailNotConfirmed(message: message)
    case "weak_password":
        return .weakPassword(message: message)
    case "email_already_exists":
        return .emailAlreadyExists(message: message)
    default:
        return .other(code: code, message: message, hint: hint)
    }
}
