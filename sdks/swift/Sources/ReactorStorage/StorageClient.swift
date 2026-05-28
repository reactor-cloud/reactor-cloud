import Foundation
import ReactorShared

/// File object in storage.
public struct FileObject: Codable, Sendable, Identifiable {
    public let id: String
    public let name: String
    public let bucketId: String
    public let owner: String?
    public let createdAt: Date
    public let updatedAt: Date
    public let metadata: [String: AnyCodable]?
    
    enum CodingKeys: String, CodingKey {
        case id, name, owner, metadata
        case bucketId = "bucket_id"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

/// Storage bucket.
public struct Bucket: Codable, Sendable, Identifiable {
    public let id: String
    public let name: String
    public let isPublic: Bool
    public let createdAt: Date
    public let updatedAt: Date
    
    enum CodingKeys: String, CodingKey {
        case id, name
        case isPublic = "public"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

/// Upload options.
public struct UploadOptions: Sendable {
    public var contentType: String?
    public var cacheControl: String?
    public var upsert: Bool
    public var metadata: [String: AnyCodable]?
    
    public init(
        contentType: String? = nil,
        cacheControl: String? = nil,
        upsert: Bool = false,
        metadata: [String: AnyCodable]? = nil
    ) {
        self.contentType = contentType
        self.cacheControl = cacheControl
        self.upsert = upsert
        self.metadata = metadata
    }
}

/// List options.
public struct ListOptions: Sendable {
    public var limit: Int?
    public var offset: Int?
    public var sortBy: (column: String, order: SortOrder)?
    public var search: String?
    
    public enum SortOrder: String, Sendable {
        case asc, desc
    }
    
    public init(
        limit: Int? = nil,
        offset: Int? = nil,
        sortBy: (column: String, order: SortOrder)? = nil,
        search: String? = nil
    ) {
        self.limit = limit
        self.offset = offset
        self.sortBy = sortBy
        self.search = search
    }
}

/// Signed URL options.
public struct SignedUrlOptions: Sendable {
    public var download: Bool
    public var downloadFilename: String?
    public var transform: ImageTransform?
    
    public struct ImageTransform: Sendable {
        public var width: Int?
        public var height: Int?
        public var quality: Int?
        
        public init(width: Int? = nil, height: Int? = nil, quality: Int? = nil) {
            self.width = width
            self.height = height
            self.quality = quality
        }
    }
    
    public init(download: Bool = false, downloadFilename: String? = nil, transform: ImageTransform? = nil) {
        self.download = download
        self.downloadFilename = downloadFilename
        self.transform = transform
    }
}

/// Client for operations on a specific bucket.
public final class BucketClient: @unchecked Sendable {
    private let ctx: RequestContext
    private let bucketId: String
    
    init(_ ctx: RequestContext, bucketId: String) {
        self.ctx = ctx
        self.bucketId = bucketId
    }
    
    /// Upload a file to the bucket.
    ///
    /// - Parameters:
    ///   - path: Path within the bucket
    ///   - data: File data
    ///   - options: Upload options
    /// - Returns: Upload result with path and ID
    public func upload(path: String, data: Data, options: UploadOptions = .init()) async throws -> UploadResult {
        let boundary = UUID().uuidString
        var body = Data()
        
        body.append("--\(boundary)\r\n".data(using: .utf8)!)
        
        let filename = (path as NSString).lastPathComponent
        let contentType = options.contentType ?? "application/octet-stream"
        body.append("Content-Disposition: form-data; name=\"file\"; filename=\"\(filename)\"\r\n".data(using: .utf8)!)
        body.append("Content-Type: \(contentType)\r\n\r\n".data(using: .utf8)!)
        body.append(data)
        body.append("\r\n".data(using: .utf8)!)
        
        if let metadata = options.metadata {
            let encoder = JSONEncoder()
            if let metadataData = try? encoder.encode(metadata) {
                body.append("--\(boundary)\r\n".data(using: .utf8)!)
                body.append("Content-Disposition: form-data; name=\"metadata\"\r\n\r\n".data(using: .utf8)!)
                body.append(metadataData)
                body.append("\r\n".data(using: .utf8)!)
            }
        }
        
        body.append("--\(boundary)--\r\n".data(using: .utf8)!)
        
        var headers: [String: String] = [
            "Content-Type": "multipart/form-data; boundary=\(boundary)"
        ]
        if let cacheControl = options.cacheControl {
            headers["Cache-Control"] = cacheControl
        }
        if options.upsert {
            headers["X-Upsert"] = "true"
        }
        
        let url = ctx.baseURL.appendingPathComponent("storage/v1/object/\(bucketId.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? bucketId)/\(path)")
        
        let httpOptions = HTTPRequestOptions(method: .post, headers: headers, body: body)
        let response = try await ctx.httpClient.request(url, options: httpOptions)
        
        guard response.isSuccess else {
            throw errorFromResponse(status: response.status, body: response.data)
        }
        
        let decoder = JSONDecoder()
        return try decoder.decode(UploadResult.self, from: response.data)
    }
    
    #if canImport(UIKit) || canImport(AppKit)
    /// Upload a file from a URL (supports background uploads on iOS).
    ///
    /// - Parameters:
    ///   - path: Path within the bucket
    ///   - fileURL: Local file URL
    ///   - options: Upload options
    /// - Returns: Upload result with path and ID
    public func upload(path: String, fileURL: URL, options: UploadOptions = .init()) async throws -> UploadResult {
        let data = try Data(contentsOf: fileURL)
        return try await upload(path: path, data: data, options: options)
    }
    #endif
    
    /// Download a file from the bucket.
    ///
    /// - Parameter path: Path within the bucket
    /// - Returns: File data
    public func download(path: String) async throws -> Data {
        let url = ctx.baseURL.appendingPathComponent("storage/v1/object/\(bucketId.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? bucketId)/\(path)")
        
        var headers: [String: String] = [:]
        if let projectKey = ctx.projectKey {
            headers["X-Reactor-Project-Key"] = projectKey
        }
        if let tokenProvider = ctx.accessTokenProvider, let token = await tokenProvider() {
            headers["Authorization"] = "Bearer \(token)"
        }
        
        let options = HTTPRequestOptions(method: .get, headers: headers)
        let response = try await ctx.httpClient.request(url, options: options)
        
        guard response.isSuccess else {
            throw errorFromResponse(status: response.status, body: response.data)
        }
        
        return response.data
    }
    
    /// Download a file as an async stream.
    ///
    /// - Parameter path: Path within the bucket
    /// - Returns: Async stream of data chunks
    public func downloadStream(path: String) -> AsyncThrowingStream<Data, Error> {
        let url = ctx.baseURL.appendingPathComponent("storage/v1/object/\(bucketId.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? bucketId)/\(path)")
        
        var headers: [String: String] = [:]
        if let projectKey = ctx.projectKey {
            headers["X-Reactor-Project-Key"] = projectKey
        }
        
        let options = HTTPRequestOptions(method: .get, headers: headers)
        return ctx.httpClient.stream(url, options: options)
    }
    
    /// List files in the bucket.
    ///
    /// - Parameters:
    ///   - prefix: Path prefix to filter by
    ///   - options: List options
    /// - Returns: Array of file objects
    public func list(prefix: String? = nil, options: ListOptions = .init()) async throws -> [FileObject] {
        var components = URLComponents(url: ctx.baseURL.appendingPathComponent("storage/v1/object/list/\(bucketId)"), resolvingAgainstBaseURL: false)!
        var queryItems: [URLQueryItem] = []
        
        if let prefix {
            queryItems.append(URLQueryItem(name: "prefix", value: prefix))
        }
        if let limit = options.limit {
            queryItems.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset = options.offset {
            queryItems.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        if let search = options.search {
            queryItems.append(URLQueryItem(name: "search", value: search))
        }
        
        if !queryItems.isEmpty {
            components.queryItems = queryItems
        }
        
        return try await request(ctx, path: "storage/v1/object/list/\(bucketId)" + (components.query.map { "?\($0)" } ?? ""))
    }
    
    /// Remove files from the bucket.
    ///
    /// - Parameter paths: Paths to remove
    public func remove(paths: [String]) async throws {
        struct RemoveRequest: Encodable {
            let prefixes: [String]
        }
        
        let _: [RemovedFile] = try await request(
            ctx,
            path: "storage/v1/object/\(bucketId)",
            method: .delete,
            body: RemoveRequest(prefixes: paths)
        )
    }
    
    /// Move a file within the bucket.
    ///
    /// - Parameters:
    ///   - from: Source path
    ///   - to: Destination path
    public func move(from: String, to: String) async throws {
        struct MoveRequest: Encodable {
            let bucketId: String
            let sourceKey: String
            let destinationKey: String
        }
        
        let _: MessageResponse = try await request(
            ctx,
            path: "storage/v1/object/move",
            method: .post,
            body: MoveRequest(bucketId: bucketId, sourceKey: from, destinationKey: to)
        )
    }
    
    /// Copy a file within the bucket.
    ///
    /// - Parameters:
    ///   - from: Source path
    ///   - to: Destination path
    /// - Returns: The new file path
    public func copy(from: String, to: String) async throws -> String {
        struct CopyRequest: Encodable {
            let bucketId: String
            let sourceKey: String
            let destinationKey: String
        }
        
        struct CopyResponse: Decodable {
            let path: String
        }
        
        let response: CopyResponse = try await request(
            ctx,
            path: "storage/v1/object/copy",
            method: .post,
            body: CopyRequest(bucketId: bucketId, sourceKey: from, destinationKey: to)
        )
        
        return response.path
    }
    
    /// Create a signed URL for temporary access.
    ///
    /// - Parameters:
    ///   - path: File path
    ///   - expiresIn: Expiration in seconds
    ///   - options: Signed URL options
    /// - Returns: The signed URL
    public func createSignedUrl(path: String, expiresIn: Int, options: SignedUrlOptions = .init()) async throws -> URL {
        struct SignRequest: Encodable {
            let expiresIn: Int
            let download: Bool?
            let transform: Transform?
            
            struct Transform: Encodable {
                let width: Int?
                let height: Int?
                let quality: Int?
            }
        }
        
        struct SignResponse: Decodable {
            let signedUrl: String
        }
        
        let transform = options.transform.map { SignRequest.Transform(width: $0.width, height: $0.height, quality: $0.quality) }
        
        let response: SignResponse = try await request(
            ctx,
            path: "storage/v1/object/sign/\(bucketId)/\(path)",
            method: .post,
            body: SignRequest(expiresIn: expiresIn, download: options.download ? true : nil, transform: transform)
        )
        
        guard let url = URL(string: response.signedUrl) else {
            throw ReactorError.validation(code: "invalid_url", message: "Invalid signed URL returned", issues: [])
        }
        
        return url
    }
    
    /// Get the public URL for a file (bucket must be public).
    ///
    /// - Parameter path: File path
    /// - Returns: The public URL
    public func getPublicUrl(path: String) -> URL {
        ctx.baseURL.appendingPathComponent("storage/v1/object/public/\(bucketId)/\(path)")
    }
}

/// Upload result.
public struct UploadResult: Decodable, Sendable {
    public let path: String
    public let id: String
}

private struct RemovedFile: Decodable {
    let name: String
}

private struct MessageResponse: Decodable {
    let message: String
}

/// Storage client for file storage operations.
public final class StorageClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    /// Create a StorageClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    /// Get a client for a specific bucket.
    ///
    /// - Parameter bucketId: Bucket ID
    /// - Returns: A bucket client
    public func from(_ bucketId: String) -> BucketClient {
        BucketClient(ctx, bucketId: bucketId)
    }
    
    // MARK: - Bucket Admin
    
    /// Create a new bucket.
    ///
    /// - Parameters:
    ///   - id: Bucket ID
    ///   - isPublic: Whether the bucket is public
    /// - Returns: The created bucket
    public func createBucket(id: String, isPublic: Bool = false) async throws -> Bucket {
        struct CreateBucketRequest: Encodable {
            let id: String
            let `public`: Bool
        }
        
        return try await request(
            ctx,
            path: "storage/v1/bucket",
            method: .post,
            body: CreateBucketRequest(id: id, public: isPublic)
        )
    }
    
    /// List all buckets.
    public func listBuckets() async throws -> [Bucket] {
        try await request(ctx, path: "storage/v1/bucket")
    }
    
    /// Get a bucket by ID.
    ///
    /// - Parameter id: Bucket ID
    public func getBucket(id: String) async throws -> Bucket {
        try await request(ctx, path: "storage/v1/bucket/\(id)")
    }
    
    /// Update a bucket.
    ///
    /// - Parameters:
    ///   - id: Bucket ID
    ///   - isPublic: New public status
    public func updateBucket(id: String, isPublic: Bool) async throws -> Bucket {
        struct UpdateBucketRequest: Encodable {
            let `public`: Bool
        }
        
        return try await request(
            ctx,
            path: "storage/v1/bucket/\(id)",
            method: .patch,
            body: UpdateBucketRequest(public: isPublic)
        )
    }
    
    /// Delete a bucket.
    ///
    /// - Parameter id: Bucket ID
    public func deleteBucket(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "storage/v1/bucket/\(id)", method: .delete)
    }
    
    /// Empty a bucket (delete all files).
    ///
    /// - Parameter id: Bucket ID
    public func emptyBucket(id: String) async throws {
        let _: EmptyResponse = try await request(ctx, path: "storage/v1/bucket/\(id)/empty", method: .post)
    }
}
