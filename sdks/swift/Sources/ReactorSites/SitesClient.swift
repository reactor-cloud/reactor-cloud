import Foundation
import ReactorShared

/// Site.
public struct Site: Codable, Sendable, Identifiable {
    public let id: String
    public let name: String
    public let framework: String?
    public let createdAt: Date
    public let updatedAt: Date
    
    enum CodingKeys: String, CodingKey {
        case id, name, framework
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

/// Site deployment.
public struct Deployment: Codable, Sendable, Identifiable {
    public let id: String
    public let siteId: String
    public let status: DeploymentStatus
    public let url: String?
    public let createdAt: Date
    public let completedAt: Date?
    
    enum CodingKeys: String, CodingKey {
        case id, status, url
        case siteId = "site_id"
        case createdAt = "created_at"
        case completedAt = "completed_at"
    }
}

/// Deployment status.
public enum DeploymentStatus: String, Codable, Sendable {
    case pending
    case building
    case deploying
    case ready
    case failed
    case cancelled
}

/// Site domain.
public struct Domain: Codable, Sendable, Identifiable {
    public let id: String
    public let siteId: String
    public let domain: String
    public let verified: Bool
    public let createdAt: Date
    
    enum CodingKeys: String, CodingKey {
        case id, domain, verified
        case siteId = "site_id"
        case createdAt = "created_at"
    }
}

/// Sites client for static site deployment.
public final class SitesClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    /// Create a SitesClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    // MARK: - Sites
    
    /// List sites.
    public func list() async throws -> [Site] {
        try await request(ctx, path: "sites/v1/sites")
    }
    
    /// Get a site by ID.
    public func get(id: String) async throws -> Site {
        try await request(ctx, path: "sites/v1/sites/\(id)")
    }
    
    /// Create a new site.
    ///
    /// - Parameters:
    ///   - name: Site name
    ///   - framework: Framework name (optional)
    public func create(name: String, framework: String? = nil) async throws -> Site {
        struct CreateSiteRequest: Encodable {
            let name: String
            let framework: String?
        }
        
        return try await request(
            ctx,
            path: "sites/v1/sites",
            method: .post,
            body: CreateSiteRequest(name: name, framework: framework)
        )
    }
    
    /// Update a site.
    public func update(id: String, name: String? = nil) async throws -> Site {
        struct UpdateSiteRequest: Encodable {
            let name: String?
        }
        
        return try await request(
            ctx,
            path: "sites/v1/sites/\(id)",
            method: .patch,
            body: UpdateSiteRequest(name: name)
        )
    }
    
    /// Delete a site.
    public func delete(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "sites/v1/sites/\(id)", method: .delete)
    }
    
    // MARK: - Deployments
    
    /// List deployments for a site.
    public func listDeployments(siteId: String) async throws -> [Deployment] {
        try await request(ctx, path: "sites/v1/sites/\(siteId)/deployments")
    }
    
    /// Get a deployment by ID.
    public func getDeployment(siteId: String, deploymentId: String) async throws -> Deployment {
        try await request(ctx, path: "sites/v1/sites/\(siteId)/deployments/\(deploymentId)")
    }
    
    /// Create a new deployment.
    ///
    /// - Parameters:
    ///   - siteId: Site ID
    ///   - tarballData: Tarball of the site content
    /// - Returns: The created deployment
    public func createDeployment(siteId: String, tarballData: Data) async throws -> Deployment {
        let url = ctx.baseURL.appendingPathComponent("sites/v1/sites/\(siteId)/deployments")
        
        var headers: [String: String] = [
            "Content-Type": "application/x-tar"
        ]
        if let projectKey = ctx.projectKey {
            headers["X-Reactor-Project-Key"] = projectKey
        }
        if let tokenProvider = ctx.accessTokenProvider, let token = await tokenProvider() {
            headers["Authorization"] = "Bearer \(token)"
        }
        
        let options = HTTPRequestOptions(method: .post, headers: headers, body: tarballData)
        let response = try await ctx.httpClient.request(url, options: options)
        
        guard response.isSuccess else {
            throw errorFromResponse(status: response.status, body: response.data)
        }
        
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try decoder.decode(Deployment.self, from: response.data)
    }
    
    /// Cancel a deployment.
    public func cancelDeployment(siteId: String, deploymentId: String) async throws {
        let _: EmptyResponse = try await request(
            ctx,
            path: "sites/v1/sites/\(siteId)/deployments/\(deploymentId)/cancel",
            method: .post
        )
    }
    
    // MARK: - Domains
    
    /// List domains for a site.
    public func listDomains(siteId: String) async throws -> [Domain] {
        try await request(ctx, path: "sites/v1/sites/\(siteId)/domains")
    }
    
    /// Add a domain to a site.
    public func addDomain(siteId: String, domain: String) async throws -> Domain {
        struct AddDomainRequest: Encodable {
            let domain: String
        }
        
        return try await request(
            ctx,
            path: "sites/v1/sites/\(siteId)/domains",
            method: .post,
            body: AddDomainRequest(domain: domain)
        )
    }
    
    /// Remove a domain from a site.
    public func removeDomain(siteId: String, domainId: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "sites/v1/sites/\(siteId)/domains/\(domainId)", method: .delete)
    }
    
    /// Verify a domain.
    public func verifyDomain(siteId: String, domainId: String) async throws -> Domain {
        try await request(ctx, path: "sites/v1/sites/\(siteId)/domains/\(domainId)/verify", method: .post)
    }
}
