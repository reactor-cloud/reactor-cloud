import Foundation

/// Filter operator types matching reactor-data dialect.
public enum FilterOperator: String, Sendable {
    case eq
    case neq
    case gt
    case gte
    case lt
    case lte
    case like
    case ilike
    case `in` = "in"
    case `is` = "is"
    case cs     // contains
    case cd     // containedBy
    case ov     // overlaps
    case fts    // full-text search
}

/// Primitive value types for filters.
public enum FilterValue: Sendable, CustomStringConvertible {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
    case array([FilterValue])
    
    public var description: String {
        encodeFilterValue(self)
    }
}

/// Encode a value for use in a filter expression.
public func encodeFilterValue(_ value: FilterValue) -> String {
    switch value {
    case .null:
        return "null"
    case .bool(let b):
        return b ? "true" : "false"
    case .int(let i):
        return String(i)
    case .double(let d):
        return String(d)
    case .string(let s):
        return s
    case .array(let arr):
        let encoded = arr.map { item -> String in
            switch item {
            case .string(let s) where s.contains(",") || s.contains("(") || s.contains(")"):
                return "\"\(s.replacingOccurrences(of: "\"", with: "\\\""))\""
            default:
                return encodeFilterValue(item)
            }
        }
        return "(\(encoded.joined(separator: ",")))"
    }
}

/// Build a filter expression in PostgREST format.
public func buildFilterExpression(
    op: FilterOperator,
    value: FilterValue,
    negated: Bool = false
) -> String {
    let encoded = encodeFilterValue(value)
    let prefix = negated ? "not." : ""
    return "\(prefix)\(op.rawValue).\(encoded)"
}

/// Order direction.
public enum OrderDirection: String, Sendable {
    case asc
    case desc
}

/// Order nulls position.
public enum OrderNulls: String, Sendable {
    case nullsFirst = "nullsfirst"
    case nullsLast = "nullslast"
}

/// Build an order expression.
public func buildOrderExpression(
    column: String,
    ascending: Bool = true,
    nullsFirst: Bool? = nil
) -> String {
    var parts = [column]
    
    if !ascending {
        parts.append("desc")
    }
    
    if let nullsFirst {
        parts.append(nullsFirst ? "nullsfirst" : "nullslast")
    }
    
    return parts.joined(separator: ".")
}

/// Parameters collected by the query builder.
public struct QueryParams: Sendable {
    public var select: String?
    public var filters: [(column: String, expression: String)]
    public var order: [String]
    public var limit: Int?
    public var offset: Int?
    public var count: CountMode?
    
    public enum CountMode: String, Sendable {
        case exact
        case planned
        case estimated
    }
    
    public init(
        select: String? = nil,
        filters: [(column: String, expression: String)] = [],
        order: [String] = [],
        limit: Int? = nil,
        offset: Int? = nil,
        count: CountMode? = nil
    ) {
        self.select = select
        self.filters = filters
        self.order = order
        self.limit = limit
        self.offset = offset
        self.count = count
    }
}

/// Convert QueryParams to URL query items.
public func queryParamsToURLQueryItems(_ params: QueryParams) -> [URLQueryItem] {
    var items: [URLQueryItem] = []
    
    if let select = params.select {
        items.append(URLQueryItem(name: "select", value: select))
    }
    
    for filter in params.filters {
        items.append(URLQueryItem(name: filter.column, value: filter.expression))
    }
    
    if !params.order.isEmpty {
        items.append(URLQueryItem(name: "order", value: params.order.joined(separator: ",")))
    }
    
    if let limit = params.limit {
        items.append(URLQueryItem(name: "limit", value: String(limit)))
    }
    
    if let offset = params.offset {
        items.append(URLQueryItem(name: "offset", value: String(offset)))
    }
    
    return items
}

/// Build a full URL with query parameters.
public func buildURL(baseURL: URL, path: String, params: QueryParams?) -> URL {
    var components = URLComponents(url: baseURL.appendingPathComponent(path), resolvingAgainstBaseURL: true)!
    
    if let params {
        let items = queryParamsToURLQueryItems(params)
        if !items.isEmpty {
            components.queryItems = items
        }
    }
    
    return components.url!
}

/// Parse the Content-Range header for count information.
/// Format: "0-24/1234" or "*/1234"
public func parseContentRange(_ header: String?) -> (from: Int?, to: Int?, total: Int?)? {
    guard let header else { return nil }
    
    let pattern = #"^(\d+|\*)-?(\d+)?\/(\d+|\*)$"#
    guard let regex = try? NSRegularExpression(pattern: pattern),
          let match = regex.firstMatch(in: header, range: NSRange(header.startIndex..., in: header)) else {
        return nil
    }
    
    func extractInt(_ range: NSRange) -> Int? {
        guard range.location != NSNotFound,
              let swiftRange = Range(range, in: header) else { return nil }
        let str = String(header[swiftRange])
        return str == "*" ? nil : Int(str)
    }
    
    return (
        from: extractInt(match.range(at: 1)),
        to: extractInt(match.range(at: 2)),
        total: extractInt(match.range(at: 3))
    )
}

/// Parse a select string and extract column names.
/// Handles embedded relations like "author:users(name)".
public func parseSelectColumns(_ select: String) -> [String] {
    var columns: [String] = []
    var depth = 0
    var current = ""
    
    for char in select {
        switch char {
        case "(":
            depth += 1
            current.append(char)
        case ")":
            depth -= 1
            current.append(char)
        case "," where depth == 0:
            let trimmed = current.trimmingCharacters(in: .whitespaces)
            if !trimmed.isEmpty {
                columns.append(trimmed)
            }
            current = ""
        default:
            current.append(char)
        }
    }
    
    let trimmed = current.trimmingCharacters(in: .whitespaces)
    if !trimmed.isEmpty {
        columns.append(trimmed)
    }
    
    return columns
}

/// Encode a value for embedding in a URL path segment.
public func encodePathSegment(_ value: String) -> String {
    value.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed)?
        .replacingOccurrences(of: "%2F", with: "/") ?? value
}
