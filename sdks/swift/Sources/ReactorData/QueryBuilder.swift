import Foundation
import ReactorShared

/// Count mode for queries.
public enum CountMode: String, Sendable {
    case exact
    case planned
    case estimated
}

/// Result modifier for queries.
public enum ResultModifier: Sendable {
    case single
    case maybeSingle
}

/// Text search options.
public struct TextSearchOptions: Sendable {
    public enum SearchType: String, Sendable {
        case plain
        case phrase
        case websearch
    }
    
    public var type: SearchType
    public var config: String?
    
    public init(type: SearchType = .plain, config: String? = nil) {
        self.type = type
        self.config = config
    }
}

/// Order options.
public struct OrderOptions: Sendable {
    public var ascending: Bool
    public var nullsFirst: Bool?
    public var foreignTable: String?
    
    public init(ascending: Bool = true, nullsFirst: Bool? = nil, foreignTable: String? = nil) {
        self.ascending = ascending
        self.nullsFirst = nullsFirst
        self.foreignTable = foreignTable
    }
}

/// Pending filter to be applied.
private struct PendingFilter: Sendable {
    let column: String
    let `operator`: String
    let value: String
    let negated: Bool
}

/// PostgREST-style query builder with full filter/modifier/mutation operator parity.
public struct QueryBuilder<Row: Codable & Sendable>: Sendable {
    private let ctx: RequestContext
    private let table: String
    private var selectColumns: String = "*"
    private var filters: [PendingFilter] = []
    private var orderClauses: [String] = []
    private var limitValue: Int?
    private var offsetValue: Int?
    private var countMode: CountMode?
    private var customHeaders: [String: String] = [:]
    private var resultModifier: ResultModifier?
    private var method: HTTPMethod = .get
    private var bodyData: Data?
    
    init(ctx: RequestContext, table: String) {
        self.ctx = ctx
        self.table = table
    }
    
    // MARK: - Filter Operators
    
    /// Equal to.
    public func eq(_ column: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "eq", value: encodeValue(value), negated: false))
        return builder
    }
    
    /// Not equal to.
    public func neq(_ column: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "neq", value: encodeValue(value), negated: false))
        return builder
    }
    
    /// Greater than.
    public func gt(_ column: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "gt", value: encodeValue(value), negated: false))
        return builder
    }
    
    /// Greater than or equal.
    public func gte(_ column: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "gte", value: encodeValue(value), negated: false))
        return builder
    }
    
    /// Less than.
    public func lt(_ column: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "lt", value: encodeValue(value), negated: false))
        return builder
    }
    
    /// Less than or equal.
    public func lte(_ column: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "lte", value: encodeValue(value), negated: false))
        return builder
    }
    
    /// Pattern match (LIKE).
    public func like(_ column: String, pattern: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "like", value: pattern, negated: false))
        return builder
    }
    
    /// Case-insensitive pattern match (ILIKE).
    public func ilike(_ column: String, pattern: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "ilike", value: pattern, negated: false))
        return builder
    }
    
    /// Is NULL or boolean.
    public func `is`(_ column: String, value: Bool?) -> QueryBuilder {
        var builder = self
        let valueString = value.map { $0 ? "true" : "false" } ?? "null"
        builder.filters.append(PendingFilter(column: column, operator: "is", value: valueString, negated: false))
        return builder
    }
    
    /// In list.
    public func `in`(_ column: String, values: [Any]) -> QueryBuilder {
        var builder = self
        let encoded = "(\(values.map { encodeValue($0) }.joined(separator: ",")))"
        builder.filters.append(PendingFilter(column: column, operator: "in", value: encoded, negated: false))
        return builder
    }
    
    /// Array contains.
    public func contains(_ column: String, values: [Any]) -> QueryBuilder {
        var builder = self
        let encoded = "{\(values.map { encodeValue($0) }.joined(separator: ","))}"
        builder.filters.append(PendingFilter(column: column, operator: "cs", value: encoded, negated: false))
        return builder
    }
    
    /// Array contained by.
    public func containedBy(_ column: String, values: [Any]) -> QueryBuilder {
        var builder = self
        let encoded = "{\(values.map { encodeValue($0) }.joined(separator: ","))}"
        builder.filters.append(PendingFilter(column: column, operator: "cd", value: encoded, negated: false))
        return builder
    }
    
    /// Array overlaps.
    public func overlaps(_ column: String, values: [Any]) -> QueryBuilder {
        var builder = self
        let encoded = "{\(values.map { encodeValue($0) }.joined(separator: ","))}"
        builder.filters.append(PendingFilter(column: column, operator: "ov", value: encoded, negated: false))
        return builder
    }
    
    /// Range greater than.
    public func rangeGt(_ column: String, value: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "sr", value: value, negated: false))
        return builder
    }
    
    /// Range greater than or equal.
    public func rangeGte(_ column: String, value: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "nxl", value: value, negated: false))
        return builder
    }
    
    /// Range less than.
    public func rangeLt(_ column: String, value: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "sl", value: value, negated: false))
        return builder
    }
    
    /// Range less than or equal.
    public func rangeLte(_ column: String, value: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "nxr", value: value, negated: false))
        return builder
    }
    
    /// Range adjacent.
    public func rangeAdjacent(_ column: String, value: String) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: "adj", value: value, negated: false))
        return builder
    }
    
    /// Full-text search.
    public func textSearch(_ column: String, query: String, options: TextSearchOptions = .init()) -> QueryBuilder {
        var builder = self
        let value: String
        if let config = options.config {
            value = "\(config).\(options.type.rawValue).\(query)"
        } else {
            value = "\(options.type.rawValue).\(query)"
        }
        builder.filters.append(PendingFilter(column: column, operator: "fts", value: value, negated: false))
        return builder
    }
    
    /// Match multiple conditions (shorthand for multiple eq).
    public func match(_ query: [String: Any]) -> QueryBuilder {
        var builder = self
        for (column, value) in query {
            builder.filters.append(PendingFilter(column: column, operator: "eq", value: encodeValue(value), negated: false))
        }
        return builder
    }
    
    /// Negate a filter.
    public func not(_ column: String, operator: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: `operator`, value: encodeValue(value), negated: true))
        return builder
    }
    
    /// OR condition (raw string format).
    public func or(_ conditions: String, foreignTable: String? = nil) -> QueryBuilder {
        var builder = self
        let column = foreignTable.map { "\($0).or" } ?? "or"
        builder.filters.append(PendingFilter(column: column, operator: "", value: "(\(conditions))", negated: false))
        return builder
    }
    
    /// AND condition (raw string format).
    public func and(_ conditions: String, foreignTable: String? = nil) -> QueryBuilder {
        var builder = self
        let column = foreignTable.map { "\($0).and" } ?? "and"
        builder.filters.append(PendingFilter(column: column, operator: "", value: "(\(conditions))", negated: false))
        return builder
    }
    
    /// Generic filter (escape hatch).
    public func filter(_ column: String, operator: String, value: Any) -> QueryBuilder {
        var builder = self
        builder.filters.append(PendingFilter(column: column, operator: `operator`, value: encodeValue(value), negated: false))
        return builder
    }
    
    // MARK: - Modifiers
    
    /// Select specific columns.
    public func select(_ columns: String = "*", count: CountMode? = nil) -> QueryBuilder {
        var builder = self
        builder.selectColumns = columns
        builder.countMode = count
        builder.method = .get
        return builder
    }
    
    /// Order results.
    public func order(_ column: String, options: OrderOptions = .init()) -> QueryBuilder {
        var builder = self
        var parts = [column]
        if !options.ascending {
            parts.append("desc")
        }
        if let nullsFirst = options.nullsFirst {
            parts.append(nullsFirst ? "nullsfirst" : "nullslast")
        }
        if let foreignTable = options.foreignTable {
            builder.orderClauses.append("\(foreignTable)(\(parts.joined(separator: ".")))")
        } else {
            builder.orderClauses.append(parts.joined(separator: "."))
        }
        return builder
    }
    
    /// Order ascending (convenience).
    public func order(_ column: String, ascending: Bool) -> QueryBuilder {
        order(column, options: OrderOptions(ascending: ascending))
    }
    
    /// Limit results.
    public func limit(_ count: Int, foreignTable: String? = nil) -> QueryBuilder {
        var builder = self
        if let foreignTable {
            builder.customHeaders["\(foreignTable)-limit"] = String(count)
        } else {
            builder.limitValue = count
        }
        return builder
    }
    
    /// Offset results / range (for pagination).
    public func range(from: Int, to: Int, foreignTable: String? = nil) -> QueryBuilder {
        var builder = self
        if let foreignTable {
            builder.customHeaders["\(foreignTable)-offset"] = String(from)
            builder.customHeaders["\(foreignTable)-limit"] = String(to - from + 1)
        } else {
            builder.offsetValue = from
            builder.limitValue = to - from + 1
        }
        return builder
    }
    
    /// Execute and return exactly one row (throws if not exactly one).
    public func single() -> QueryBuilder {
        var builder = self
        builder.resultModifier = .single
        return builder
    }
    
    /// Execute and return zero or one row.
    public func maybeSingle() -> QueryBuilder {
        var builder = self
        builder.resultModifier = .maybeSingle
        return builder
    }
    
    // MARK: - Mutations
    
    /// Insert row(s).
    public func insert(_ values: any Encodable, count: CountMode? = nil) -> QueryBuilder {
        var builder = self
        builder.method = .post
        builder.countMode = count
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        builder.bodyData = try? encoder.encode(AnyEncodableWrapper(values))
        return builder
    }
    
    /// Upsert row(s).
    public func upsert(_ values: any Encodable, onConflict: String? = nil, ignoreDuplicates: Bool = false, count: CountMode? = nil) -> QueryBuilder {
        var builder = self
        builder.method = .post
        builder.countMode = count
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        builder.bodyData = try? encoder.encode(AnyEncodableWrapper(values))
        
        var prefer = "resolution=\(ignoreDuplicates ? "ignore" : "merge")-duplicates"
        if let onConflict {
            prefer += ",on_conflict=\(onConflict)"
        }
        builder.customHeaders["Prefer"] = prefer
        return builder
    }
    
    /// Update row(s).
    public func update(_ values: any Encodable, count: CountMode? = nil) -> QueryBuilder {
        var builder = self
        builder.method = .patch
        builder.countMode = count
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        builder.bodyData = try? encoder.encode(AnyEncodableWrapper(values))
        return builder
    }
    
    /// Delete row(s).
    public func delete(count: CountMode? = nil) -> QueryBuilder {
        var builder = self
        builder.method = .delete
        builder.countMode = count
        return builder
    }
    
    // MARK: - Execution
    
    /// Execute the query and return results.
    public func execute() async throws -> [Row] {
        let url = buildURL()
        let headers = buildHeaders()
        
        let options = HTTPRequestOptions(
            method: method,
            headers: headers,
            body: bodyData
        )
        
        let response = try await ctx.httpClient.request(url, options: options)
        
        guard response.isSuccess else {
            throw errorFromResponse(status: response.status, body: response.data)
        }
        
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        
        if resultModifier == .single || resultModifier == .maybeSingle {
            if response.data.isEmpty || String(data: response.data, encoding: .utf8) == "null" {
                if resultModifier == .maybeSingle {
                    return []
                }
                throw ReactorError.notFound(code: "PGRST116", message: "Row not found", hint: nil)
            }
            let single = try decoder.decode(Row.self, from: response.data)
            return [single]
        }
        
        return try decoder.decode([Row].self, from: response.data)
    }
    
    /// Execute the query and return a single result.
    public func executeSingle() async throws -> Row {
        let results = try await single().execute()
        guard let first = results.first else {
            throw ReactorError.notFound(code: "PGRST116", message: "Row not found", hint: nil)
        }
        return first
    }
    
    /// Execute the query and return zero or one result.
    public func executeMaybeSingle() async throws -> Row? {
        let results = try await maybeSingle().execute()
        return results.first
    }
    
    // MARK: - Private Helpers
    
    private func buildURL() -> URL {
        var components = URLComponents(url: ctx.baseURL.appendingPathComponent("data/v1/\(table)"), resolvingAgainstBaseURL: false)!
        var queryItems: [URLQueryItem] = []
        
        queryItems.append(URLQueryItem(name: "select", value: selectColumns))
        
        for filter in filters {
            let prefix = filter.negated ? "not." : ""
            let op = filter.operator.isEmpty ? "" : "\(filter.operator)."
            queryItems.append(URLQueryItem(name: filter.column, value: "\(prefix)\(op)\(filter.value)"))
        }
        
        if !orderClauses.isEmpty {
            queryItems.append(URLQueryItem(name: "order", value: orderClauses.joined(separator: ",")))
        }
        
        if let limit = limitValue {
            queryItems.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        
        if let offset = offsetValue {
            queryItems.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        
        components.queryItems = queryItems
        return components.url!
    }
    
    private func buildHeaders() -> [String: String] {
        var headers = customHeaders
        headers["Content-Type"] = "application/json"
        headers["Accept"] = "application/json"
        headers["X-Reactor-Client"] = "swift/\(ReactorSharedVersion)"
        
        if let projectKey = ctx.projectKey {
            headers["X-Reactor-Project-Key"] = projectKey
        }
        
        if let countMode {
            headers["Prefer"] = (headers["Prefer"].map { $0 + "," } ?? "") + "count=\(countMode.rawValue)"
        }
        
        if resultModifier == .single || resultModifier == .maybeSingle {
            headers["Accept"] = "application/vnd.pgrst.object+json"
        }
        
        return headers
    }
    
    private func encodeValue(_ value: Any) -> String {
        switch value {
        case let string as String:
            return string
        case let int as Int:
            return String(int)
        case let double as Double:
            return String(double)
        case let bool as Bool:
            return bool ? "true" : "false"
        case is NSNull:
            return "null"
        case let optional as Any? where optional == nil:
            return "null"
        default:
            return String(describing: value)
        }
    }
}

/// Wrapper to encode any Encodable type.
private struct AnyEncodableWrapper: Encodable {
    private let _encode: (Encoder) throws -> Void
    
    init(_ wrapped: any Encodable) {
        _encode = wrapped.encode(to:)
    }
    
    func encode(to encoder: Encoder) throws {
        try _encode(encoder)
    }
}
