/// Reactor - The official Reactor Swift SDK
///
/// This is the umbrella module that composes all capability packages
/// and provides a unified client interface.
///
/// ## Usage
///
/// ```swift
/// import Reactor
///
/// let reactor = createClient(
///     url: URL(string: "https://reactor.cloud")!,
///     options: .init(key: "rk_pub_...")
/// )
///
/// // Auth
/// let session = try await reactor.auth.signIn(email: email, password: password)
///
/// // Data (PostgREST-style)
/// let posts: [Post] = try await reactor.from("posts", as: Post.self)
///     .select()
///     .eq("published", value: true)
///     .execute()
///
/// // Storage
/// try await reactor.storage.from("avatars").upload(path: "me.jpg", data: imageData)
///
/// // Functions
/// let result: OrderResult = try await reactor.functions.invoke("process-order", body: order)
///
/// // Jobs
/// let run = try await reactor.jobs.trigger("send-email", payload: emailPayload)
/// ```

@_exported import ReactorShared
@_exported import ReactorAuth
@_exported import ReactorData
@_exported import ReactorStorage
@_exported import ReactorFunctions
@_exported import ReactorJobs
@_exported import ReactorSites
@_exported import ReactorRealtime
@_exported import ReactorAnalytics
@_exported import ReactorAI

public let ReactorVersion = "0.1.0"
