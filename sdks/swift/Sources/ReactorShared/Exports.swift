/// ReactorShared - Foundation types and utilities for the Reactor Swift SDK
///
/// This module provides:
/// - HTTPClient protocol and URLSessionHTTPClient implementation
/// - ReactorError types for error handling
/// - JWT decoding utilities
/// - SessionStore protocols for session persistence
/// - RequestContext for shared request configuration
/// - Query builders for PostgREST-style URLs
///
/// ## Example
///
/// ```swift
/// import ReactorShared
///
/// let ctx = RequestContext(
///     baseURL: URL(string: "https://reactor.cloud")!,
///     projectKey: "rk_pub_..."
/// )
///
/// let user: User = try await request(ctx, path: "/auth/v1/user")
/// ```

public let ReactorSharedVersion = "0.1.0"

// Re-export all public types
// Types are exported from their respective files via public declarations
