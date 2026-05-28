import XCTest
@testable import ReactorShared

final class ReactorSharedTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorSharedVersion.isEmpty)
    }
}

// MARK: - JWT Tests

final class JWTTests: XCTestCase {
    let validToken = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyXzEyMyIsImVtYWlsIjoidGVzdEB0ZXN0LmNvbSIsImVtYWlsX3ZlcmlmaWVkIjp0cnVlLCJleHAiOjk5OTk5OTk5OTksImlhdCI6MTcwMDAwMDAwMH0.signature"
    
    let expiredToken = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyXzEyMyIsImV4cCI6MTAwMDAwMDAwMCwiaWF0IjoxMDAwMDAwMDAwfQ.signature"
    
    func testDecodeValidJWT() {
        let payload = decodeJWT(validToken)
        XCTAssertNotNil(payload)
        XCTAssertEqual(payload?.sub, "user_123")
        XCTAssertEqual(payload?.email, "test@test.com")
        XCTAssertEqual(payload?.emailVerified, true)
    }
    
    func testDecodeInvalidJWT() {
        XCTAssertNil(decodeJWT("invalid"))
        XCTAssertNil(decodeJWT("a.b"))
        XCTAssertNil(decodeJWT(""))
    }
    
    func testIsJWTExpired() {
        XCTAssertFalse(isJWTExpired(validToken))
        XCTAssertTrue(isJWTExpired(expiredToken))
    }
    
    func testGetJWTExpiry() {
        let expiry = getJWTExpiry(validToken)
        XCTAssertNotNil(expiry)
        XCTAssertGreaterThan(expiry!.timeIntervalSince1970, Date().timeIntervalSince1970)
    }
    
    func testGetJWTTimeRemaining() {
        let remaining = getJWTTimeRemaining(validToken)
        XCTAssertNotNil(remaining)
        XCTAssertGreaterThan(remaining!, 0)
        
        let expiredRemaining = getJWTTimeRemaining(expiredToken)
        XCTAssertNotNil(expiredRemaining)
        XCTAssertLessThan(expiredRemaining!, 0)
    }
}

// MARK: - Query Tests

final class QueryTests: XCTestCase {
    func testEncodeFilterValue() {
        XCTAssertEqual(encodeFilterValue(.null), "null")
        XCTAssertEqual(encodeFilterValue(.bool(true)), "true")
        XCTAssertEqual(encodeFilterValue(.bool(false)), "false")
        XCTAssertEqual(encodeFilterValue(.int(42)), "42")
        XCTAssertEqual(encodeFilterValue(.double(3.14)), "3.14")
        XCTAssertEqual(encodeFilterValue(.string("hello")), "hello")
        XCTAssertEqual(encodeFilterValue(.array([.int(1), .int(2), .int(3)])), "(1,2,3)")
    }
    
    func testBuildFilterExpression() {
        XCTAssertEqual(buildFilterExpression(op: .eq, value: .int(42)), "eq.42")
        XCTAssertEqual(buildFilterExpression(op: .neq, value: .string("test")), "neq.test")
        XCTAssertEqual(buildFilterExpression(op: .eq, value: .null, negated: true), "not.eq.null")
        XCTAssertEqual(buildFilterExpression(op: .in, value: .array([.int(1), .int(2)])), "in.(1,2)")
    }
    
    func testBuildOrderExpression() {
        XCTAssertEqual(buildOrderExpression(column: "name"), "name")
        XCTAssertEqual(buildOrderExpression(column: "name", ascending: false), "name.desc")
        XCTAssertEqual(buildOrderExpression(column: "name", ascending: true, nullsFirst: true), "name.nullsfirst")
        XCTAssertEqual(buildOrderExpression(column: "name", ascending: false, nullsFirst: false), "name.desc.nullslast")
    }
    
    func testQueryParamsToURLQueryItems() {
        let params = QueryParams(
            select: "id,name",
            filters: [("status", "eq.active")],
            order: ["created_at.desc"],
            limit: 10,
            offset: 20
        )
        
        let items = queryParamsToURLQueryItems(params)
        XCTAssertEqual(items.count, 5)
        XCTAssertTrue(items.contains { $0.name == "select" && $0.value == "id,name" })
        XCTAssertTrue(items.contains { $0.name == "status" && $0.value == "eq.active" })
        XCTAssertTrue(items.contains { $0.name == "order" && $0.value == "created_at.desc" })
        XCTAssertTrue(items.contains { $0.name == "limit" && $0.value == "10" })
        XCTAssertTrue(items.contains { $0.name == "offset" && $0.value == "20" })
    }
    
    func testParseContentRange() {
        let result1 = parseContentRange("0-24/100")
        XCTAssertEqual(result1?.from, 0)
        XCTAssertEqual(result1?.to, 24)
        XCTAssertEqual(result1?.total, 100)
        
        let result2 = parseContentRange("*/50")
        XCTAssertNil(result2?.from)
        XCTAssertEqual(result2?.total, 50)
        
        XCTAssertNil(parseContentRange(nil))
        XCTAssertNil(parseContentRange("invalid"))
    }
    
    func testParseSelectColumns() {
        XCTAssertEqual(parseSelectColumns("id,name,email"), ["id", "name", "email"])
        XCTAssertEqual(parseSelectColumns("id, name, email"), ["id", "name", "email"])
        XCTAssertEqual(parseSelectColumns("id,author:users(name,email)"), ["id", "author:users(name,email)"])
        XCTAssertEqual(parseSelectColumns("*"), ["*"])
    }
}

// MARK: - Error Tests

final class ErrorTests: XCTestCase {
    func testErrorFromResponse() {
        let json = """
        {"error": {"code": "invalid_credentials", "message": "Wrong password"}}
        """.data(using: .utf8)!
        
        let error = errorFromResponse(status: 401, body: json)
        
        if case .auth(let authError) = error {
            if case .invalidCredentials(let message) = authError {
                XCTAssertEqual(message, "Wrong password")
            } else {
                XCTFail("Expected invalidCredentials error")
            }
        } else {
            XCTFail("Expected auth error")
        }
    }
    
    func testValidationError() {
        let json = """
        {"error": {"code": "validation_error", "message": "Invalid input", "fields": {"email": ["Invalid format"]}}}
        """.data(using: .utf8)!
        
        let error = errorFromResponse(status: 400, body: json)
        
        if case .validation(_, let message, let issues) = error {
            XCTAssertEqual(message, "Invalid input")
            XCTAssertEqual(issues.count, 1)
            XCTAssertEqual(issues.first?.field, "email")
        } else {
            XCTFail("Expected validation error")
        }
    }
}

// MARK: - Session Store Tests

final class InMemorySessionStoreTests: XCTestCase {
    func testStoreAndRetrieve() async {
        let store = InMemorySessionStore()
        
        await store.setItem("key", value: "value")
        let retrieved = await store.getItem("key")
        
        XCTAssertEqual(retrieved, "value")
    }
    
    func testRemove() async {
        let store = InMemorySessionStore()
        
        await store.setItem("key", value: "value")
        await store.removeItem("key")
        let retrieved = await store.getItem("key")
        
        XCTAssertNil(retrieved)
    }
    
    func testGetNonexistent() async {
        let store = InMemorySessionStore()
        let retrieved = await store.getItem("nonexistent")
        XCTAssertNil(retrieved)
    }
}

// MARK: - Types Tests

final class TypesTests: XCTestCase {
    func testUserDecoding() throws {
        let json = """
        {
            "id": "user_123",
            "email": "test@test.com",
            "email_verified": true,
            "metadata": {"name": "Test User"},
            "created_at": "2024-01-01T00:00:00Z"
        }
        """.data(using: .utf8)!
        
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let user = try decoder.decode(User.self, from: json)
        
        XCTAssertEqual(user.id, "user_123")
        XCTAssertEqual(user.email, "test@test.com")
        XCTAssertTrue(user.emailVerified)
    }
    
    func testSessionDecoding() throws {
        let json = """
        {
            "access_token": "token123",
            "refresh_token": "refresh456",
            "expires_at": "2024-12-31T23:59:59Z",
            "user": {
                "id": "user_123",
                "email": "test@test.com",
                "email_verified": true,
                "metadata": {},
                "created_at": "2024-01-01T00:00:00Z"
            }
        }
        """.data(using: .utf8)!
        
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let session = try decoder.decode(Session.self, from: json)
        
        XCTAssertEqual(session.accessToken, "token123")
        XCTAssertEqual(session.refreshToken, "refresh456")
        XCTAssertEqual(session.user.id, "user_123")
    }
    
    func testAnyCodableEquality() {
        XCTAssertEqual(AnyCodable("hello"), AnyCodable("hello"))
        XCTAssertEqual(AnyCodable(42), AnyCodable(42))
        XCTAssertEqual(AnyCodable(true), AnyCodable(true))
        XCTAssertNotEqual(AnyCodable("hello"), AnyCodable(42))
    }
}
