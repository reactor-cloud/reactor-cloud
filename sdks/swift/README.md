# Reactor Swift SDK

The official Swift SDK for [Reactor](https://github.com/cdelconde/reactor), a modern backend platform.

## Requirements

- iOS 17.0+, macOS 14.0+, tvOS 17.0+, watchOS 10.0+, visionOS 1.0+
- Swift 5.9+
- Xcode 15.0+

## Installation

### Swift Package Manager

Add the following to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/cdelconde/reactor", from: "0.1.0")
]
```

Then add the dependency to your target:

```swift
.target(
    name: "YourApp",
    dependencies: [
        .product(name: "Reactor", package: "reactor")
    ]
)
```

### Xcode

1. File → Add Package Dependencies
2. Enter the repository URL
3. Select the `Reactor` product

## Quick Start

```swift
import Reactor

// Create a client
let reactor = createClient(
    url: URL(string: "https://your-reactor-instance.com")!,
    options: .init(key: "rk_pub_your_project_key")
)

// Authenticate
let (user, session) = try await reactor.auth.signIn(
    email: "user@example.com",
    password: "password"
)

// Query data (PostgREST-style)
struct Post: Codable {
    let id: Int
    let title: String
    let published: Bool
}

let posts: [Post] = try await reactor.from("posts", as: Post.self)
    .select("id, title, published")
    .eq("published", value: true)
    .order("created_at", ascending: false)
    .limit(10)
    .execute()

// Upload files
try await reactor.storage.from("avatars")
    .upload(path: "me.jpg", data: imageData)

// Invoke serverless functions
let result: OrderResult = try await reactor.functions.invoke(
    "process-order",
    body: order
)

// Track analytics
await reactor.analytics.track("purchase_completed", properties: [
    "order_id": AnyCodable(orderId),
    "total": AnyCodable(99.99)
])
```

## Modules

The SDK is modular - import only what you need:

| Module | Description |
|--------|-------------|
| `Reactor` | Umbrella module (includes all) |
| `ReactorAuth` | Authentication & user management |
| `ReactorData` | PostgREST-style database queries |
| `ReactorStorage` | File storage & uploads |
| `ReactorFunctions` | Serverless function invocation |
| `ReactorJobs` | Background job management |
| `ReactorSites` | Static site deployment |
| `ReactorRealtime` | WebSocket subscriptions |
| `ReactorAnalytics` | Product analytics |
| `ReactorShared` | Common types & utilities |

## Authentication

### Sign Up / Sign In

```swift
// Sign up
let (user, session) = try await reactor.auth.signUp(
    SignUpParams(email: "new@example.com", password: "securepassword")
)

// Sign in
let (user, session) = try await reactor.auth.signIn(
    email: "user@example.com",
    password: "password"
)

// Sign out
try await reactor.auth.signOut()
```

### Session Management

```swift
// Get current session
if let session = await reactor.getSession() {
    print("Logged in as: \(session.user.email)")
}

// Listen for auth state changes
for await (event, session) in await reactor.auth.authStateChanges() {
    switch event {
    case .signedIn:
        print("User signed in")
    case .signedOut:
        print("User signed out")
    case .tokenRefreshed:
        print("Token refreshed")
    default:
        break
    }
}
```

### Magic Links & Password Reset

```swift
// Send magic link
try await reactor.auth.signInWithMagicLink(email: "user@example.com")

// Request password reset
try await reactor.auth.requestPasswordReset(email: "user@example.com")

// Confirm password reset
try await reactor.auth.confirmPasswordReset(
    token: resetToken,
    newPassword: "newsecurepassword"
)
```

### Organizations

```swift
// Create organization
let org = try await reactor.auth.orgs.create(name: "My Team")

// List organizations
let orgs = try await reactor.auth.orgs.list()

// Invite member
try await reactor.auth.orgs.invitations.invite(
    orgId: org.id,
    email: "colleague@example.com",
    roleId: memberRoleId
)
```

## Data (PostgREST)

### Querying

```swift
struct Todo: Codable {
    let id: Int
    let title: String
    let completed: Bool
    let userId: String
}

// Select with filters
let todos: [Todo] = try await reactor.from("todos", as: Todo.self)
    .select()
    .eq("userId", value: userId)
    .eq("completed", value: false)
    .order("created_at")
    .execute()

// With pagination
let page: [Todo] = try await reactor.from("todos", as: Todo.self)
    .select("*", count: .exact)
    .range(from: 0, to: 9)
    .execute()

// Full-text search
let results: [Todo] = try await reactor.from("todos", as: Todo.self)
    .select()
    .textSearch("title", query: "important")
    .execute()
```

### Mutations

```swift
// Insert
let newTodo: [Todo] = try await reactor.from("todos", as: Todo.self)
    .insert(["title": "Buy groceries", "completed": false])
    .execute()

// Update
let updated: [Todo] = try await reactor.from("todos", as: Todo.self)
    .update(["completed": true])
    .eq("id", value: todoId)
    .execute()

// Upsert
let upserted: [Todo] = try await reactor.from("todos", as: Todo.self)
    .upsert(["id": 1, "title": "Updated title"])
    .execute()

// Delete
try await reactor.from("todos", as: Todo.self)
    .delete()
    .eq("id", value: todoId)
    .execute()
```

### Stored Procedures

```swift
struct SearchResult: Codable {
    let id: Int
    let score: Double
}

let results: [SearchResult] = try await reactor.data.rpc(
    "search_todos",
    args: ["query": "important", "limit": 10]
)
```

## Storage

### File Operations

```swift
let bucket = reactor.storage.from("documents")

// Upload
let result = try await bucket.upload(
    path: "reports/2024/q1.pdf",
    data: pdfData,
    options: .init(contentType: "application/pdf")
)

// Download
let data = try await bucket.download(path: "reports/2024/q1.pdf")

// Stream download (large files)
for try await chunk in try bucket.downloadStream(path: "large-file.zip") {
    // Process chunk
}

// List files
let files = try await bucket.list(prefix: "reports/2024")

// Get public URL
let url = bucket.getPublicUrl(path: "public/image.jpg")

// Create signed URL
let signedUrl = try await bucket.createSignedUrl(
    path: "private/document.pdf",
    expiresIn: 3600
)

// Delete
try await bucket.remove(paths: ["reports/old.pdf"])
```

### Bucket Administration

```swift
// List buckets
let buckets = try await reactor.storage.listBuckets()

// Create bucket
let bucket = try await reactor.storage.createBucket(
    id: "user-uploads",
    options: .init(public: false)
)

// Delete bucket
try await reactor.storage.deleteBucket(id: "old-bucket")
```

## Functions

```swift
// Typed invoke
struct OrderRequest: Codable {
    let items: [String]
    let total: Double
}

struct OrderResponse: Codable {
    let orderId: String
    let status: String
}

let response: OrderResponse = try await reactor.functions.invoke(
    "process-order",
    body: OrderRequest(items: ["item1", "item2"], total: 99.99)
)

// Raw invoke
let data = try await reactor.functions.invokeRaw("health-check")

// Streaming (SSE)
for try await event in try reactor.functions.invokeStream("long-running-task") {
    switch event {
    case .data(let content):
        print("Progress: \(content)")
    case .event(let name, let data):
        print("Event \(name): \(data ?? "")")
    }
}
```

## Jobs

```swift
// Trigger a job
let run = try await reactor.jobs.trigger("send-newsletter", payload: [
    "templateId": "welcome",
    "recipients": ["user@example.com"]
])

// Wait for completion
let completedRun = try await reactor.jobs.runs.wait(id: run.id, maxWait: 60)

// List job runs
let runs = try await reactor.jobs.runs.list(jobName: "send-newsletter", status: .completed)

// Cancel a run
try await reactor.jobs.runs.cancel(id: runId)
```

## Analytics

```swift
// Track events
await reactor.analytics.track("button_clicked", properties: [
    "button_id": AnyCodable("signup"),
    "page": AnyCodable("/home")
])

// Screen views (mobile)
await reactor.analytics.screen("Home Screen")

// Page views
await reactor.analytics.page("Dashboard")

// Identify user (auto-called on auth state change if enabled)
await reactor.analytics.identify(userId: user.id, traits: [
    "plan": AnyCodable("pro"),
    "company": AnyCodable("Acme Inc")
])

// Consent management
await reactor.analytics.optOut()  // GDPR compliance
await reactor.analytics.optIn()

// Manual flush
await reactor.analytics.flush()
```

## Realtime (Coming Soon)

The Realtime module provides type stubs for WebSocket subscriptions. Full implementation pending `reactor-realtime` server availability.

```swift
let channel = reactor.realtime.channel("todos")

// Subscribe to changes
let subscription = channel.onPostgresChanges(
    table: "todos",
    as: Todo.self
) { change in
    switch change.eventType {
    case .insert:
        print("New todo: \(change.new?.title ?? "")")
    case .update:
        print("Updated todo: \(change.new?.title ?? "")")
    case .delete:
        print("Deleted todo ID: \(change.old?.id ?? 0)")
    default:
        break
    }
}

// Start receiving events
try await channel.subscribe()

// Unsubscribe
await subscription.unsubscribe()
```

## Error Handling

The SDK uses a unified `ReactorError` type:

```swift
do {
    let user = try await reactor.auth.signIn(email: email, password: password)
} catch let error as ReactorError {
    switch error {
    case .auth(let authError):
        switch authError {
        case .invalidCredentials:
            print("Wrong email or password")
        case .userNotFound:
            print("User doesn't exist")
        default:
            print("Auth error: \(authError)")
        }
    case .network(let message):
        print("Network error: \(message)")
    case .server(let status, let message, _):
        print("Server error \(status): \(message)")
    default:
        print("Error: \(error)")
    }
}
```

## Configuration

```swift
let reactor = createClient(
    url: URL(string: "https://your-reactor-instance.com")!,
    options: .init(
        key: "rk_pub_your_project_key",
        
        // Session persistence (default: Keychain on Apple platforms)
        sessionStore: KeychainSessionStore(),
        
        // Auto-refresh tokens (default: true)
        autoRefreshToken: true,
        
        // Auto-identify in analytics on auth change (default: true)
        autoIdentifyUser: true,
        
        // Enable analytics (default: true)
        analyticsEnabled: true,
        
        // Request timeout (default: 30s)
        timeout: 30,
        
        // Retry count (default: 0)
        retries: 2
    )
)
```

## Contributing

See the main [Reactor repository](https://github.com/cdelconde/reactor) for contribution guidelines.

## License

MIT License - see [LICENSE](../../LICENSE) for details.
