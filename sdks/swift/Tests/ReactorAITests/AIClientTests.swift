import XCTest
@testable import ReactorAI

final class AIClientTests: XCTestCase {
    func testMessageCreation() {
        let systemMsg = Message.system("You are helpful")
        XCTAssertEqual(systemMsg.role, .system)
        XCTAssertEqual(systemMsg.content, "You are helpful")
        
        let userMsg = Message.user("Hello")
        XCTAssertEqual(userMsg.role, .user)
        XCTAssertEqual(userMsg.content, "Hello")
        
        let assistantMsg = Message.assistant("Hi there")
        XCTAssertEqual(assistantMsg.role, .assistant)
        XCTAssertEqual(assistantMsg.content, "Hi there")
    }
    
    func testChatCompletionRequestEncoding() throws {
        let request = ChatCompletionRequest(
            model: "gpt-4",
            messages: [
                .system("You are helpful"),
                .user("Hello")
            ],
            maxTokens: 100
        )
        
        XCTAssertEqual(request.model, "gpt-4")
        XCTAssertEqual(request.messages.count, 2)
        XCTAssertEqual(request.maxTokens, 100)
        XCTAssertFalse(request.stream)
    }
}
