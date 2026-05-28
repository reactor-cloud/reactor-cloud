import Foundation
import ReactorShared

/// Analytics event for batching.
public struct AnalyticsEvent: Codable, Sendable {
    public let type: String
    public let event: String?
    public let userId: String?
    public let anonymousId: String
    public let timestamp: Date
    public let properties: [String: AnyCodable]?
    public let traits: [String: AnyCodable]?
    public let context: AnalyticsContext
    
    enum CodingKeys: String, CodingKey {
        case type, event, timestamp, properties, traits, context
        case userId = "user_id"
        case anonymousId = "anonymous_id"
    }
}

/// Analytics context sent with events.
public struct AnalyticsContext: Codable, Sendable {
    public let library: LibraryInfo
    public let app: AppInfo?
    public let device: DeviceInfo?
    public let os: OSInfo?
    public let sessionId: String?
    
    enum CodingKeys: String, CodingKey {
        case library, app, device, os
        case sessionId = "session_id"
    }
}

/// Library info.
public struct LibraryInfo: Codable, Sendable {
    public let name: String
    public let version: String
}

/// App info.
public struct AppInfo: Codable, Sendable {
    public let name: String?
    public let version: String?
    public let build: String?
}

/// Device info.
public struct DeviceInfo: Codable, Sendable {
    public let manufacturer: String
    public let model: String
    public let type: String
}

/// OS info.
public struct OSInfo: Codable, Sendable {
    public let name: String
    public let version: String
}

/// Analytics client for product analytics.
///
/// Manual-only API - no auto-capture. Supports:
/// - track: Custom events
/// - identify: User identification
/// - screen: Screen views (mobile)
/// - page: Page views
/// - alias: User aliasing
/// - reset: Clear user identity
/// - flush: Force send batched events
/// - optOut/optIn: Consent management
public actor AnalyticsClient {
    private let ctx: RequestContext
    private let enabled: Bool
    
    private var userId: String?
    private var anonymousId: String
    private var sessionId: String
    private var traits: [String: AnyCodable] = [:]
    private var optedOut: Bool = false
    
    private var eventQueue: [AnalyticsEvent] = []
    private let maxQueueSize: Int = 20
    private let flushInterval: TimeInterval = 30
    private var flushTask: Task<Void, Never>?
    
    private let keychainKey = "reactor_anonymous_id"
    
    /// Create an AnalyticsClient.
    ///
    /// - Parameters:
    ///   - ctx: Request context for API calls
    ///   - enabled: Whether analytics is enabled
    public init(_ ctx: RequestContext, enabled: Bool = true) {
        self.ctx = ctx
        self.enabled = enabled
        
        // Load or generate anonymous ID
        if let storedId = Self.loadAnonymousId() {
            self.anonymousId = storedId
        } else {
            let newId = UUID().uuidString
            self.anonymousId = newId
            Self.saveAnonymousId(newId)
        }
        
        // Generate session ID
        self.sessionId = UUID().uuidString
    }
    
    /// Start the periodic flush task.
    /// Call this after initialization to begin background flushing.
    public func startPeriodicFlush() {
        guard enabled, flushTask == nil else { return }
        
        flushTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: UInt64(30 * 1_000_000_000))
                await self?.flush()
            }
        }
    }
    
    deinit {
        flushTask?.cancel()
    }
    
    // MARK: - Public API
    
    /// Track a custom event.
    ///
    /// - Parameters:
    ///   - event: Event name
    ///   - properties: Event properties
    public func track(_ event: String, properties: [String: AnyCodable]? = nil) async {
        guard enabled && !optedOut else { return }
        
        let analyticsEvent = AnalyticsEvent(
            type: "track",
            event: event,
            userId: userId,
            anonymousId: anonymousId,
            timestamp: Date(),
            properties: properties,
            traits: nil,
            context: buildContext()
        )
        
        await enqueue(analyticsEvent)
    }
    
    /// Identify a user.
    ///
    /// - Parameters:
    ///   - userId: User ID
    ///   - traits: User traits
    public func identify(userId: String, traits: [String: AnyCodable]? = nil) async {
        guard enabled && !optedOut else { return }
        
        self.userId = userId
        if let traits = traits {
            self.traits.merge(traits) { _, new in new }
        }
        
        // Queue the $identify event (matches JS SDK)
        let event = AnalyticsEvent(
            type: "identify",
            event: "$identify",
            userId: userId,
            anonymousId: anonymousId,
            timestamp: Date(),
            properties: nil,
            traits: self.traits,
            context: buildContext()
        )
        
        await enqueue(event)
        
        // Also send to dedicated identify endpoint (matches JS SDK)
        await sendIdentify(userId: userId, traits: self.traits)
    }
    
    /// Send identify to dedicated endpoint.
    private func sendIdentify(userId: String, traits: [String: AnyCodable]) async {
        struct IdentifyRequest: Encodable {
            let anonymous_id: String
            let user_id: String
            let traits: [String: AnyCodable]
        }
        
        do {
            let _: EmptyResponse = try await request(
                ctx,
                path: "analytics/v1/identify",
                method: .post,
                body: IdentifyRequest(
                    anonymous_id: anonymousId,
                    user_id: userId,
                    traits: traits
                )
            )
        } catch {
            // Ignore errors
        }
    }
    
    /// Track a screen view (mobile).
    ///
    /// - Parameters:
    ///   - name: Screen name
    ///   - properties: Screen properties
    public func screen(_ name: String, properties: [String: AnyCodable]? = nil) async {
        guard enabled && !optedOut else { return }
        
        var props = properties ?? [:]
        props["name"] = AnyCodable(name)
        
        let event = AnalyticsEvent(
            type: "screen",
            event: name,
            userId: userId,
            anonymousId: anonymousId,
            timestamp: Date(),
            properties: props,
            traits: nil,
            context: buildContext()
        )
        
        await enqueue(event)
    }
    
    /// Track a page view.
    ///
    /// - Parameters:
    ///   - name: Page name
    ///   - properties: Page properties
    public func page(_ name: String, properties: [String: AnyCodable]? = nil) async {
        guard enabled && !optedOut else { return }
        
        var props = properties ?? [:]
        props["name"] = AnyCodable(name)
        
        let event = AnalyticsEvent(
            type: "page",
            event: name,
            userId: userId,
            anonymousId: anonymousId,
            timestamp: Date(),
            properties: props,
            traits: nil,
            context: buildContext()
        )
        
        await enqueue(event)
    }
    
    /// Alias a user ID.
    ///
    /// - Parameters:
    ///   - newId: New user ID
    ///   - previousId: Previous user ID (defaults to anonymous ID)
    public func alias(newId: String, previousId: String? = nil) async {
        guard enabled && !optedOut else { return }
        
        let prev = previousId ?? anonymousId
        
        // Queue the $alias event (matches JS SDK)
        let event = AnalyticsEvent(
            type: "alias",
            event: "$alias",
            userId: newId,
            anonymousId: prev,
            timestamp: Date(),
            properties: ["previousId": AnyCodable(prev), "userId": AnyCodable(newId)],
            traits: nil,
            context: buildContext()
        )
        
        await enqueue(event)
        
        // Also send to dedicated alias endpoint (matches JS SDK)
        await sendAlias(previousId: prev, userId: newId)
    }
    
    /// Send alias to dedicated endpoint.
    private func sendAlias(previousId: String, userId: String) async {
        struct AliasRequest: Encodable {
            let anonymous_id: String
            let user_id: String
        }
        
        do {
            let _: EmptyResponse = try await request(
                ctx,
                path: "analytics/v1/alias",
                method: .post,
                body: AliasRequest(
                    anonymous_id: previousId,
                    user_id: userId
                )
            )
        } catch {
            // Ignore errors
        }
    }
    
    /// Reset the user identity.
    public func reset() async {
        userId = nil
        traits = [:]
        anonymousId = UUID().uuidString
        sessionId = UUID().uuidString
        Self.saveAnonymousId(anonymousId)
    }
    
    /// Flush the event queue immediately.
    public func flush() async {
        guard !eventQueue.isEmpty else { return }
        
        let events = eventQueue
        eventQueue = []
        
        do {
            try await sendBatch(events)
        } catch {
            // Re-queue events on failure (with limit)
            if eventQueue.count < maxQueueSize * 2 {
                eventQueue = events + eventQueue
            }
        }
    }
    
    /// Opt out of analytics.
    public func optOut() async {
        optedOut = true
        eventQueue = []
        
        // Match JS SDK endpoint path exactly: /consent/opt-out
        do {
            let _: EmptyResponse = try await request(
                ctx,
                path: "analytics/v1/consent/opt-out",
                method: .post,
                body: ["anonymous_id": anonymousId]
            )
        } catch {
            // Ignore errors
        }
    }
    
    /// Opt back into analytics.
    public func optIn() async {
        optedOut = false
        
        // Match JS SDK endpoint path exactly: /consent/opt-in
        do {
            let _: EmptyResponse = try await request(
                ctx,
                path: "analytics/v1/consent/opt-in",
                method: .post,
                body: ["anonymous_id": anonymousId]
            )
        } catch {
            // Ignore errors
        }
    }
    
    /// Check if user has opted out.
    public func isOptedOut() -> Bool {
        optedOut
    }
    
    // MARK: - Private
    
    private func enqueue(_ event: AnalyticsEvent) async {
        eventQueue.append(event)
        
        if eventQueue.count >= maxQueueSize {
            await flush()
        }
    }
    
    private func sendBatch(_ events: [AnalyticsEvent]) async throws {
        // Match JS SDK wire format exactly
        struct BatchEventPayload: Encodable {
            let event: String
            let anonymous_id: String
            let user_id: String?
            let session_id: String?
            let timestamp: String
            let properties: [String: AnyCodable]?
            let context: AnalyticsContext
        }
        
        struct BatchRequest: Encodable {
            let events: [BatchEventPayload]
        }
        
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        let payloads = events.map { e in
            BatchEventPayload(
                event: e.event ?? e.type,
                anonymous_id: e.anonymousId,
                user_id: e.userId,
                session_id: e.context.sessionId,
                timestamp: formatter.string(from: e.timestamp),
                properties: e.properties ?? e.traits,
                context: e.context
            )
        }
        
        let _: EmptyResponse = try await request(
            ctx,
            path: "analytics/v1/batch",
            method: .post,
            body: BatchRequest(events: payloads)
        )
    }
    
    private func buildContext() -> AnalyticsContext {
        AnalyticsContext(
            library: LibraryInfo(name: "reactor-swift", version: ReactorAnalyticsVersion),
            app: buildAppInfo(),
            device: buildDeviceInfo(),
            os: buildOSInfo(),
            sessionId: sessionId
        )
    }
    
    private func buildAppInfo() -> AppInfo? {
        #if os(iOS) || os(macOS) || os(tvOS) || os(watchOS) || os(visionOS)
        let bundle = Bundle.main
        return AppInfo(
            name: bundle.object(forInfoDictionaryKey: "CFBundleName") as? String,
            version: bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String,
            build: bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String
        )
        #else
        return nil
        #endif
    }
    
    private func buildDeviceInfo() -> DeviceInfo? {
        #if os(iOS)
        return DeviceInfo(manufacturer: "Apple", model: UIDevice.current.model, type: "mobile")
        #elseif os(macOS)
        return DeviceInfo(manufacturer: "Apple", model: "Mac", type: "desktop")
        #elseif os(tvOS)
        return DeviceInfo(manufacturer: "Apple", model: "Apple TV", type: "tv")
        #elseif os(watchOS)
        return DeviceInfo(manufacturer: "Apple", model: "Apple Watch", type: "watch")
        #elseif os(visionOS)
        return DeviceInfo(manufacturer: "Apple", model: "Apple Vision Pro", type: "headset")
        #else
        return nil
        #endif
    }
    
    private func buildOSInfo() -> OSInfo? {
        #if os(iOS)
        return OSInfo(name: "iOS", version: UIDevice.current.systemVersion)
        #elseif os(macOS)
        let version = ProcessInfo.processInfo.operatingSystemVersion
        return OSInfo(name: "macOS", version: "\(version.majorVersion).\(version.minorVersion).\(version.patchVersion)")
        #elseif os(tvOS)
        return OSInfo(name: "tvOS", version: UIDevice.current.systemVersion)
        #elseif os(watchOS)
        return OSInfo(name: "watchOS", version: "unknown")
        #elseif os(visionOS)
        return OSInfo(name: "visionOS", version: "unknown")
        #else
        return nil
        #endif
    }
    
    // MARK: - Keychain Storage
    
    private static func loadAnonymousId() -> String? {
        #if os(iOS) || os(macOS) || os(tvOS) || os(watchOS) || os(visionOS)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.reactor.analytics",
            kSecAttrAccount as String: "anonymous_id",
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        
        guard status == errSecSuccess,
              let data = result as? Data,
              let id = String(data: data, encoding: .utf8) else {
            return nil
        }
        
        return id
        #else
        return nil
        #endif
    }
    
    private static func saveAnonymousId(_ id: String) {
        #if os(iOS) || os(macOS) || os(tvOS) || os(watchOS) || os(visionOS)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.reactor.analytics",
            kSecAttrAccount as String: "anonymous_id"
        ]
        
        SecItemDelete(query as CFDictionary)
        
        guard let data = id.data(using: .utf8) else { return }
        
        let addQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.reactor.analytics",
            kSecAttrAccount as String: "anonymous_id",
            kSecValueData as String: data
        ]
        
        SecItemAdd(addQuery as CFDictionary, nil)
        #endif
    }
}
