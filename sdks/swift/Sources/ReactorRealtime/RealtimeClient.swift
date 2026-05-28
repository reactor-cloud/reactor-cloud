import Foundation
import ReactorShared

/// Realtime event types.
public enum RealtimeEvent: String, Sendable {
    case insert = "INSERT"
    case update = "UPDATE"
    case delete = "DELETE"
    case all = "*"
}

/// Postgres changes filter.
public struct PostgresChangesFilter: Sendable {
    public let event: RealtimeEvent
    public let schema: String
    public let table: String
    public let filter: String?
    
    public init(
        event: RealtimeEvent = .all,
        schema: String = "public",
        table: String,
        filter: String? = nil
    ) {
        self.event = event
        self.schema = schema
        self.table = table
        self.filter = filter
    }
}

/// Postgres change payload.
public struct PostgresChangePayload<T: Decodable & Sendable>: Sendable {
    public let eventType: RealtimeEvent
    public let old: T?
    public let new: T?
    public let schema: String
    public let table: String
    
    public init(eventType: RealtimeEvent, old: T?, new: T?, schema: String, table: String) {
        self.eventType = eventType
        self.old = old
        self.new = new
        self.schema = schema
        self.table = table
    }
}

/// Realtime channel subscription.
public struct RealtimeSubscription: Sendable {
    private let _unsubscribe: @Sendable () async -> Void
    
    init(_ unsubscribe: @escaping @Sendable () async -> Void) {
        self._unsubscribe = unsubscribe
    }
    
    /// Unsubscribe from the channel.
    public func unsubscribe() async {
        await _unsubscribe()
    }
}

/// Realtime channel for subscriptions.
///
/// Note: This is a type-stub. Full WebSocket implementation pending `reactor-realtime` server availability.
public actor RealtimeChannel {
    private let name: String
    private let client: RealtimeClient
    private var subscriptions: [UUID: AnyObject] = [:]
    
    init(name: String, client: RealtimeClient) {
        self.name = name
        self.client = client
    }
    
    /// Subscribe to Postgres changes.
    ///
    /// - Parameters:
    ///   - filter: Changes filter
    ///   - type: Row type to decode
    ///   - callback: Called when changes occur
    /// - Returns: Subscription handle
    ///
    /// Note: Implementation pending server availability.
    public func on<T: Decodable & Sendable>(
        _ filter: PostgresChangesFilter,
        as type: T.Type,
        callback: @escaping @Sendable (PostgresChangePayload<T>) -> Void
    ) -> RealtimeSubscription {
        let id = UUID()
        
        return RealtimeSubscription { [weak self] in
            await self?.removeSubscription(id: id)
        }
    }
    
    /// Subscribe to all Postgres changes on a table.
    public func onPostgresChanges<T: Decodable & Sendable>(
        table: String,
        schema: String = "public",
        as type: T.Type,
        callback: @escaping @Sendable (PostgresChangePayload<T>) -> Void
    ) -> RealtimeSubscription {
        on(PostgresChangesFilter(schema: schema, table: table), as: type, callback: callback)
    }
    
    /// Subscribe the channel (start receiving events).
    ///
    /// Note: Implementation pending server availability.
    public func subscribe() async throws {
        // TODO: Implement WebSocket connection when reactor-realtime server is ready
    }
    
    /// Unsubscribe the channel.
    public func unsubscribe() async {
        subscriptions.removeAll()
        // TODO: Close WebSocket connection
    }
    
    private func removeSubscription(id: UUID) {
        subscriptions[id] = nil
    }
}

/// Realtime client for real-time subscriptions.
///
/// Note: This is a type-stub. Full WebSocket implementation pending `reactor-realtime` server availability.
/// The API matches the JS SDK so client code will work once the server is ready.
public final class RealtimeClient: @unchecked Sendable {
    private let ctx: RequestContext
    private var channels: [String: RealtimeChannel] = [:]
    
    /// Create a RealtimeClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Get or create a channel.
    ///
    /// - Parameter name: Channel name
    /// - Returns: The channel
    public func channel(_ name: String) -> RealtimeChannel {
        if let existing = channels[name] {
            return existing
        }
        
        let channel = RealtimeChannel(name: name, client: self)
        channels[name] = channel
        return channel
    }
    
    /// Remove a channel.
    ///
    /// - Parameter name: Channel name
    public func removeChannel(_ name: String) async {
        if let channel = channels[name] {
            await channel.unsubscribe()
            channels[name] = nil
        }
    }
    
    /// Remove all channels.
    public func removeAllChannels() async {
        for channel in channels.values {
            await channel.unsubscribe()
        }
        channels.removeAll()
    }
}
