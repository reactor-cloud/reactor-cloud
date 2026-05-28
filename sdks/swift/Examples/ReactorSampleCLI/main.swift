import Foundation
import Reactor

@main
struct ReactorSampleCLI {
    static func main() async throws {
        print("Reactor Swift SDK v\(ReactorVersion)")
        print("==========================================")
        print()
        
        // Get arguments
        let args = CommandLine.arguments
        guard args.count >= 3 else {
            printUsage()
            return
        }
        
        guard let url = URL(string: args[1]) else {
            print("Error: Invalid URL '\(args[1])'")
            return
        }
        
        let projectKey = args[2]
        
        // Optional credentials for auth testing
        let email = args.count > 3 ? args[3] : nil
        let password = args.count > 4 ? args[4] : nil
        
        print("Connecting to: \(url)")
        print("Project key: \(projectKey.prefix(20))...")
        print()
        
        // Create client
        let reactor = createClient(
            url: url,
            options: .init(
                key: projectKey,
                analyticsEnabled: false
            )
        )
        
        // Run tests
        do {
            if let email = email, let password = password {
                try await testAuth(reactor: reactor, email: email, password: password)
            } else {
                print("Skipping auth tests (no credentials provided)")
            }
            
            print()
            try await testData(reactor: reactor)
            
            print()
            try await testStorage(reactor: reactor)
            
            print()
            try await testFunctions(reactor: reactor)
            
            print()
            print("==========================================")
            print("All tests completed!")
        } catch {
            print()
            print("Error: \(error)")
            print()
            print("Note: Some tests require a running Reactor instance with the appropriate services enabled.")
        }
    }
    
    static func printUsage() {
        print("This sample CLI exercises the SDK against a dev Reactor instance.")
        print()
        print("Usage: ReactorSampleCLI <reactor-url> <project-key> [email] [password]")
        print()
        print("Examples:")
        print("  ReactorSampleCLI https://localhost:8000 rk_pub_xxx")
        print("  ReactorSampleCLI https://localhost:8000 rk_pub_xxx user@example.com password123")
    }
    
    // MARK: - Auth Tests
    
    static func testAuth(reactor: ReactorClient, email: String, password: String) async throws {
        print("Testing Auth...")
        print("---------------")
        
        // Try to sign in
        print("  Signing in as \(email)...")
        let (user, session) = try await reactor.auth.signIn(email: email, password: password)
        print("  ✓ Signed in successfully")
        print("    User ID: \(user.id)")
        print("    Email: \(user.email)")
        print("    Token expires: \(session.expiresAt)")
        
        // Get current user
        print("  Getting current user...")
        if let currentUser = await reactor.getUser() {
            print("  ✓ Got user: \(currentUser.email)")
        }
        
        // Get session
        print("  Getting session...")
        if let currentSession = await reactor.getSession() {
            print("  ✓ Got session (token length: \(currentSession.accessToken.count))")
        }
        
        // Sign out
        print("  Signing out...")
        try await reactor.auth.signOut()
        print("  ✓ Signed out successfully")
    }
    
    // MARK: - Data Tests
    
    static func testData(reactor: ReactorClient) async throws {
        print("Testing Data (QueryBuilder)...")
        print("------------------------------")
        
        // Define a sample row type
        struct TestRow: Codable {
            let id: Int
            let name: String
            let created_at: String?
        }
        
        // Test query building (won't execute without a real table)
        print("  Building sample query...")
        
        let builder = reactor.from("test_table", as: TestRow.self)
            .select("id, name, created_at")
            .eq("name", value: "test")
            .order("created_at", ascending: false)
            .limit(10)
        
        print("  ✓ QueryBuilder created successfully")
        print("    Note: Execution requires a 'test_table' in the database")
        
        // Try to execute (will fail gracefully if table doesn't exist)
        print("  Attempting query execution...")
        do {
            let results: [TestRow] = try await builder.execute()
            print("  ✓ Query executed, got \(results.count) rows")
        } catch {
            print("  → Query failed (expected if no test_table): \(String(describing: error).prefix(80))...")
        }
        
        // Test RPC
        print("  Testing RPC call builder...")
        do {
            struct RPCResult: Codable {
                let result: String
            }
            let _: RPCResult = try await reactor.data.rpc("test_function", args: ["param": "value"])
            print("  ✓ RPC executed")
        } catch {
            print("  → RPC failed (expected if no test_function): \(String(describing: error).prefix(80))...")
        }
    }
    
    // MARK: - Storage Tests
    
    static func testStorage(reactor: ReactorClient) async throws {
        print("Testing Storage...")
        print("------------------")
        
        // Test bucket listing
        print("  Listing buckets...")
        do {
            let buckets = try await reactor.storage.listBuckets()
            print("  ✓ Found \(buckets.count) buckets")
            for bucket in buckets.prefix(3) {
                print("    - \(bucket.name) (\(bucket.isPublic ? "public" : "private"))")
            }
        } catch {
            print("  → Bucket list failed (expected if storage not configured): \(String(describing: error).prefix(80))...")
        }
        
        // Test file operations (with a test bucket)
        print("  Testing file operations...")
        let testData = "Hello from Reactor Swift SDK!".data(using: .utf8)!
        
        do {
            let bucket = reactor.storage.from("test-bucket")
            
            // Upload
            print("    Uploading test file...")
            _ = try await bucket.upload(path: "test/hello.txt", data: testData, options: .init(contentType: "text/plain"))
            print("    ✓ Upload successful")
            
            // Download
            print("    Downloading test file...")
            let downloaded = try await bucket.download(path: "test/hello.txt")
            if let text = String(data: downloaded, encoding: .utf8) {
                print("    ✓ Downloaded: \(text)")
            }
            
            // List
            print("    Listing files...")
            let files = try await bucket.list(prefix: "test")
            print("    ✓ Found \(files.count) files")
            
            // Get public URL
            let publicUrl = bucket.getPublicUrl(path: "test/hello.txt")
            print("    Public URL: \(publicUrl)")
            
            // Clean up
            print("    Removing test file...")
            try await bucket.remove(paths: ["test/hello.txt"])
            print("    ✓ Removed")
            
        } catch {
            print("  → Storage operations failed (expected if no test-bucket): \(String(describing: error).prefix(80))...")
        }
    }
    
    // MARK: - Functions Tests
    
    static func testFunctions(reactor: ReactorClient) async throws {
        print("Testing Functions...")
        print("--------------------")
        
        struct EchoRequest: Codable {
            let message: String
        }
        
        struct EchoResponse: Codable {
            let echo: String
        }
        
        print("  Invoking test function...")
        do {
            let response: EchoResponse = try await reactor.functions.invoke(
                "echo",
                body: EchoRequest(message: "Hello from Swift!")
            )
            print("  ✓ Function returned: \(response.echo)")
        } catch {
            print("  → Function call failed (expected if no echo function): \(String(describing: error).prefix(80))...")
        }
        
        // Test raw invoke
        print("  Testing raw invoke...")
        do {
            let data = try await reactor.functions.invokeRaw("health")
            if let text = String(data: data, encoding: .utf8) {
                print("  ✓ Raw response: \(text.prefix(100))...")
            }
        } catch {
            print("  → Raw invoke failed: \(String(describing: error).prefix(80))...")
        }
    }
}
