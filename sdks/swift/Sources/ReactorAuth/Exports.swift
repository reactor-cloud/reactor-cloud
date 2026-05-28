/// ReactorAuth - Authentication and user management for the Reactor Swift SDK
///
/// This module provides:
/// - AuthClient for user authentication (signUp, signIn, signOut, etc.)
/// - AuthStateManager for session state and refresh token management
/// - OrgsClient for organization management
/// - ApiKeysClient for API key management
/// - URLDetect for deep link / callback URL parsing
///
/// ## Example
///
/// ```swift
/// import ReactorAuth
///
/// let ctx = RequestContext(baseURL: URL(string: "https://reactor.cloud")!)
/// let auth = AuthClient(ctx)
///
/// // Sign in
/// let (user, session) = try await auth.signIn(email: "user@example.com", password: "...")
///
/// // Listen for state changes
/// for await (event, session) in await auth.authStateChanges() {
///     print("Auth event: \(event)")
/// }
/// ```

@_exported import ReactorShared

public let ReactorAuthVersion = "0.1.0"
