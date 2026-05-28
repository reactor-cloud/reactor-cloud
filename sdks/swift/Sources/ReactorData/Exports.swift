/// ReactorData - PostgREST-style database operations for the Reactor Swift SDK
///
/// This module provides:
/// - DataClient for database operations
/// - QueryBuilder for fluent query construction
/// - Full PostgREST filter/modifier operator support
/// - RPC (stored procedure) calls
///
/// ## Example
///
/// ```swift
/// import ReactorData
///
/// struct Post: Codable, Sendable {
///     let id: String
///     let title: String
///     let published: Bool
/// }
///
/// let ctx = RequestContext(baseURL: URL(string: "https://reactor.cloud")!)
/// let data = DataClient(ctx)
///
/// // Select with filters
/// let posts = try await data.from("posts", as: Post.self)
///     .select()
///     .eq("published", value: true)
///     .order("created_at", ascending: false)
///     .limit(10)
///     .execute()
///
/// // Insert
/// let newPost = try await data.from("posts", as: Post.self)
///     .insert(["title": "Hello", "published": true])
///     .select()
///     .single()
///     .execute()
///
/// // RPC
/// let result: [Post] = try await data.rpc("search_posts", args: ["query": "hello"])
/// ```

@_exported import ReactorShared

public let ReactorDataVersion = "0.1.0"
