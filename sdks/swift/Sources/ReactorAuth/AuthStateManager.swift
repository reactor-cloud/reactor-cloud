import Foundation
import ReactorShared

/// Refresh margin - schedule refresh 60 seconds before expiration.
private let refreshMarginSeconds = 60

/// Manages auth state including session storage, token refresh, and state change broadcasts.
public actor AuthStateManager {
    private var session: Session?
    private var continuations: [UUID: AsyncStream<(AuthStateEvent, Session?)>.Continuation] = [:]
    private var refreshTask: Task<Void, Never>?
    private var refreshInProgress: Task<Session?, Error>?
    
    private let storage: SessionStore?
    private let storageKey: String
    private let autoRefresh: Bool
    private let persistSession: Bool
    private var onRefresh: (@Sendable () async throws -> Session?)?
    
    /// Create an AuthStateManager.
    ///
    /// - Parameters:
    ///   - storage: Storage adapter for session persistence
    ///   - storageKey: Key to use in storage (default: "reactor.session")
    ///   - autoRefresh: Whether to automatically refresh tokens before expiry
    ///   - persistSession: Whether to persist sessions to storage
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
    
    /// Initialize state from storage.
    ///
    /// - Returns: The restored session, or nil if no valid session exists
    public func initialize() async -> Session? {
        guard persistSession, let storage else { return nil }
        
        do {
            guard let stored = await storage.getItem(storageKey) else {
                return nil
            }
            
            guard let data = stored.data(using: .utf8) else {
                return nil
            }
            
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let session = try decoder.decode(Session.self, from: data)
            
            if isJWTExpired(session.accessToken) {
                if let onRefresh {
                    do {
                        if let refreshed = try await onRefresh() {
                            await setSession(refreshed, event: .initialSession)
                            return refreshed
                        }
                    } catch {
                        // Refresh failed, clear session
                    }
                }
                await clearSession()
                return nil
            }
            
            await setSession(session, event: .initialSession)
            return session
        } catch {
            return nil
        }
    }
    
    /// Get the current session.
    public func getSession() -> Session? {
        session
    }
    
    /// Get the current user.
    public func getUser() -> User? {
        session?.user
    }
    
    /// Get the current access token.
    public func getAccessToken() -> String? {
        session?.accessToken
    }
    
    /// Set the current session.
    ///
    /// - Parameters:
    ///   - session: The session to set, or nil to clear
    ///   - event: The event type to broadcast
    public func setSession(_ session: Session?, event: AuthStateEvent) async {
        self.session = session
        
        if persistSession, let storage {
            if let session {
                do {
                    let encoder = JSONEncoder()
                    encoder.dateEncodingStrategy = .iso8601
                    let data = try encoder.encode(session)
                    if let string = String(data: data, encoding: .utf8) {
                        await storage.setItem(storageKey, value: string)
                    }
                } catch {
                    // Ignore encoding errors
                }
            } else {
                await storage.removeItem(storageKey)
            }
        }
        
        if let session, autoRefresh {
            scheduleRefresh(accessToken: session.accessToken)
        } else {
            cancelRefresh()
        }
        
        notifyListeners(event: event, session: session)
    }
    
    /// Clear the current session.
    public func clearSession() async {
        await setSession(nil, event: .signedOut)
    }
    
    /// Set the refresh callback.
    ///
    /// - Parameter onRefresh: Async function that returns a refreshed session
    public func setRefreshCallback(_ onRefresh: @escaping @Sendable () async throws -> Session?) {
        self.onRefresh = onRefresh
        
        if let session, autoRefresh {
            scheduleRefresh(accessToken: session.accessToken)
        }
    }
    
    /// Manually trigger a refresh.
    ///
    /// - Returns: The refreshed session, or nil if refresh failed
    public func refresh() async -> Session? {
        guard let onRefresh else { return nil }
        
        if let refreshInProgress {
            return try? await refreshInProgress.value
        }
        
        let task = Task<Session?, Error> { [weak self] in
            guard let self else { return nil }
            
            do {
                let session = try await onRefresh()
                if let session {
                    await self.setSession(session, event: .tokenRefreshed)
                } else {
                    await self.clearSession()
                }
                return session
            } catch {
                await self.clearSession()
                throw error
            }
        }
        
        refreshInProgress = task
        
        defer { refreshInProgress = nil }
        
        return try? await task.value
    }
    
    /// Create an async stream of auth state changes.
    ///
    /// - Returns: An async stream that emits `(event, session)` tuples
    public func authStateChanges() -> AsyncStream<(AuthStateEvent, Session?)> {
        let id = UUID()
        
        return AsyncStream { continuation in
            continuation.onTermination = { [weak self] _ in
                Task { [weak self] in
                    await self?.removeContinuation(id: id)
                }
            }
            
            Task { [weak self] in
                await self?.addContinuation(id: id, continuation: continuation)
                
                if let session = await self?.session {
                    continuation.yield((.initialSession, session))
                }
            }
        }
    }
    
    /// Subscribe to auth state changes with a callback (legacy API).
    ///
    /// - Parameter callback: Called when auth state changes
    /// - Returns: A subscription handle with an `unsubscribe` method
    public nonisolated func onAuthStateChange(
        _ callback: @escaping @Sendable (AuthStateEvent, Session?) -> Void
    ) -> AuthStateSubscription {
        let stream = Task {
            await authStateChanges()
        }
        
        let task = Task {
            let authStream = await stream.value
            for await (event, session) in authStream {
                callback(event, session)
            }
        }
        
        return AuthStateSubscription {
            task.cancel()
        }
    }
    
    /// Cleanup resources.
    public func destroy() {
        cancelRefresh()
        for continuation in continuations.values {
            continuation.finish()
        }
        continuations.removeAll()
    }
    
    // MARK: - Private
    
    private func addContinuation(id: UUID, continuation: AsyncStream<(AuthStateEvent, Session?)>.Continuation) {
        continuations[id] = continuation
    }
    
    private func removeContinuation(id: UUID) {
        continuations[id]?.finish()
        continuations[id] = nil
    }
    
    private func notifyListeners(event: AuthStateEvent, session: Session?) {
        for continuation in continuations.values {
            continuation.yield((event, session))
        }
    }
    
    private func scheduleRefresh(accessToken: String) {
        cancelRefresh()
        
        guard let remaining = getJWTTimeRemaining(accessToken) else {
            Task { await refresh() }
            return
        }
        
        if remaining <= 0 {
            Task { await refresh() }
            return
        }
        
        let delay = max(remaining - refreshMarginSeconds, 0)
        
        refreshTask = Task { [weak self] in
            do {
                try await Task.sleep(nanoseconds: UInt64(delay) * 1_000_000_000)
                _ = await self?.refresh()
            } catch {
                // Task was cancelled
            }
        }
    }
    
    private func cancelRefresh() {
        refreshTask?.cancel()
        refreshTask = nil
    }
}

/// Subscription handle for auth state changes.
public struct AuthStateSubscription: Sendable {
    private let _unsubscribe: @Sendable () -> Void
    
    init(_ unsubscribe: @escaping @Sendable () -> Void) {
        self._unsubscribe = unsubscribe
    }
    
    /// Unsubscribe from auth state changes.
    public func unsubscribe() {
        _unsubscribe()
    }
}
