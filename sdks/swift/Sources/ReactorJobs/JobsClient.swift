import Foundation
import ReactorShared

/// Job run status.
public enum JobRunStatus: String, Codable, Sendable {
    case pending
    case running
    case completed
    case failed
    case cancelled
}

/// Job run.
public struct JobRun: Codable, Sendable, Identifiable {
    public let id: String
    public let jobName: String
    public let status: JobRunStatus
    public let payload: [String: AnyCodable]?
    public let result: [String: AnyCodable]?
    public let error: String?
    public let startedAt: Date?
    public let completedAt: Date?
    public let createdAt: Date
    
    enum CodingKeys: String, CodingKey {
        case id, status, payload, result, error
        case jobName = "job_name"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case createdAt = "created_at"
    }
}

/// Job trigger.
public struct JobTrigger: Codable, Sendable, Identifiable {
    public let id: String
    public let jobName: String
    public let schedule: String
    public let enabled: Bool
    public let createdAt: Date
    
    enum CodingKeys: String, CodingKey {
        case id, schedule, enabled
        case jobName = "job_name"
        case createdAt = "created_at"
    }
}

/// Dead letter queue item.
public struct DLQItem: Codable, Sendable, Identifiable {
    public let id: String
    public let jobName: String
    public let payload: [String: AnyCodable]?
    public let error: String
    public let failedAt: Date
    
    enum CodingKeys: String, CodingKey {
        case id, payload, error
        case jobName = "job_name"
        case failedAt = "failed_at"
    }
}

/// Jobs client for background job management.
public final class JobsClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    /// Runs sub-client for job run operations.
    public let runs: RunsClient
    
    /// DLQ sub-client for dead letter queue operations.
    public let dlq: DLQClient
    
    /// Triggers sub-client for scheduled job triggers.
    public let triggers: TriggersClient
    
    /// Create a JobsClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
        self.runs = RunsClient(ctx)
        self.dlq = DLQClient(ctx)
        self.triggers = TriggersClient(ctx)
    }
    
    /// Trigger a job.
    ///
    /// - Parameters:
    ///   - name: Job name
    ///   - payload: Job payload
    /// - Returns: The created job run
    public func trigger<Payload: Encodable>(_ name: String, payload: Payload) async throws -> JobRun {
        try await request(
            ctx,
            path: "jobs/v1/trigger/\(name)",
            method: .post,
            body: payload
        )
    }
    
    /// Trigger a job with no payload.
    public func trigger(_ name: String) async throws -> JobRun {
        struct EmptyPayload: Encodable {}
        return try await trigger(name, payload: EmptyPayload())
    }
}

/// Client for job run operations.
public final class RunsClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Get a job run by ID.
    public func get(id: String) async throws -> JobRun {
        try await request(ctx, path: "jobs/v1/runs/\(id)")
    }
    
    /// List job runs.
    ///
    /// - Parameters:
    ///   - jobName: Filter by job name (optional)
    ///   - status: Filter by status (optional)
    ///   - limit: Maximum number of results
    public func list(jobName: String? = nil, status: JobRunStatus? = nil, limit: Int = 50) async throws -> [JobRun] {
        var path = "jobs/v1/runs?limit=\(limit)"
        if let jobName {
            path += "&job_name=\(jobName)"
        }
        if let status {
            path += "&status=\(status.rawValue)"
        }
        return try await request(ctx, path: path)
    }
    
    /// Cancel a job run.
    public func cancel(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "jobs/v1/runs/\(id)/cancel", method: .post)
    }
    
    /// Wait for a job run to complete with exponential backoff.
    ///
    /// - Parameters:
    ///   - id: Job run ID
    ///   - maxWait: Maximum wait time in seconds
    /// - Returns: The completed job run
    public func wait(id: String, maxWait: TimeInterval = 60) async throws -> JobRun {
        let startTime = Date()
        var delay: TimeInterval = 0.5
        
        while true {
            let run = try await get(id: id)
            
            switch run.status {
            case .completed, .failed, .cancelled:
                return run
            case .pending, .running:
                if Date().timeIntervalSince(startTime) > maxWait {
                    throw ReactorError.timeout
                }
                try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                delay = min(delay * 2, 5)
            }
        }
    }
}

/// Client for dead letter queue operations.
public final class DLQClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// List items in the dead letter queue.
    ///
    /// - Parameter jobName: Filter by job name (optional)
    public func list(jobName: String? = nil) async throws -> [DLQItem] {
        var path = "jobs/v1/dlq"
        if let jobName {
            path += "?job_name=\(jobName)"
        }
        return try await request(ctx, path: path)
    }
    
    /// Replay a dead letter queue item.
    ///
    /// - Parameter id: DLQ item ID
    /// - Returns: The new job run
    public func replay(id: String) async throws -> JobRun {
        try await request(ctx, path: "jobs/v1/dlq/\(id)/replay", method: .post)
    }
    
    /// Delete a dead letter queue item.
    public func delete(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "jobs/v1/dlq/\(id)", method: .delete)
    }
}

/// Client for job trigger operations.
public final class TriggersClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Create a scheduled trigger.
    ///
    /// - Parameters:
    ///   - jobName: Job name
    ///   - schedule: Cron expression
    /// - Returns: The created trigger
    public func create(jobName: String, schedule: String) async throws -> JobTrigger {
        struct CreateTriggerRequest: Encodable {
            let job_name: String
            let schedule: String
        }
        
        return try await request(
            ctx,
            path: "jobs/v1/triggers",
            method: .post,
            body: CreateTriggerRequest(job_name: jobName, schedule: schedule)
        )
    }
    
    /// List triggers.
    public func list() async throws -> [JobTrigger] {
        try await request(ctx, path: "jobs/v1/triggers")
    }
    
    /// Delete a trigger.
    public func delete(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "jobs/v1/triggers/\(id)", method: .delete)
    }
}
