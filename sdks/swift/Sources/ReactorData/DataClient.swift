import Foundation
import ReactorShared

/// Data client for PostgREST-style database operations.
public final class DataClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    /// Create a DataClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Start a query on a table.
    ///
    /// - Parameters:
    ///   - table: Table name
    ///   - type: Row type to decode as
    /// - Returns: A query builder for the table
    ///
    /// ## Example
    ///
    /// ```swift
    /// struct Post: Codable, Sendable {
    ///     let id: String
    ///     let title: String
    ///     let published: Bool
    /// }
    ///
    /// let posts = try await client.from("posts", as: Post.self)
    ///     .select()
    ///     .eq("published", value: true)
    ///     .order("created_at", ascending: false)
    ///     .limit(10)
    ///     .execute()
    /// ```
    public func from<Row: Codable & Sendable>(_ table: String, as type: Row.Type) -> QueryBuilder<Row> {
        QueryBuilder<Row>(ctx: ctx, table: table)
    }
    
    /// Call a stored procedure (RPC).
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - args: Function arguments
    ///   - type: Return type
    /// - Returns: The function result
    ///
    /// ## Example
    ///
    /// ```swift
    /// struct SearchArgs: Encodable {
    ///     let query: String
    ///     let limit: Int
    /// }
    ///
    /// let results: [Post] = try await client.rpc("search_posts", args: SearchArgs(query: "hello", limit: 10))
    /// ```
    public func rpc<Args: Encodable, Result: Decodable>(
        _ name: String,
        args: Args,
        as type: Result.Type = Result.self
    ) async throws -> Result {
        try await request(
            ctx,
            path: "/data/v1/rpc/\(name)",
            method: .post,
            body: args
        )
    }
    
    /// Call a stored procedure with no arguments.
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - type: Return type
    /// - Returns: The function result
    public func rpc<Result: Decodable>(
        _ name: String,
        as type: Result.Type = Result.self
    ) async throws -> Result {
        try await request(
            ctx,
            path: "/data/v1/rpc/\(name)",
            method: .post,
            body: EmptyBody()
        )
    }
    
    /// Call a stored procedure that returns void.
    ///
    /// - Parameters:
    ///   - name: Function name
    ///   - args: Function arguments
    public func rpc<Args: Encodable>(
        _ name: String,
        args: Args
    ) async throws {
        let _: EmptyResponse = try await request(
            ctx,
            path: "/data/v1/rpc/\(name)",
            method: .post,
            body: args
        )
    }
}

private struct EmptyBody: Encodable {}
