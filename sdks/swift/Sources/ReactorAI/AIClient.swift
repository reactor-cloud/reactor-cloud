import Foundation
import ReactorShared

// MARK: - Message Types

/// Role for chat messages.
public enum MessageRole: String, Codable, Sendable {
    case system
    case user
    case assistant
    case tool
}

/// A tool call within a message.
public struct ToolCall: Codable, Sendable {
    public let id: String
    public let type: String
    public let function: FunctionCall
    
    public struct FunctionCall: Codable, Sendable {
        public let name: String
        public let arguments: String
    }
}

/// A chat message.
public struct Message: Codable, Sendable {
    public let role: MessageRole
    public let content: String?
    public let name: String?
    public let toolCallId: String?
    public let toolCalls: [ToolCall]?
    
    enum CodingKeys: String, CodingKey {
        case role, content, name
        case toolCallId = "tool_call_id"
        case toolCalls = "tool_calls"
    }
    
    public init(
        role: MessageRole,
        content: String?,
        name: String? = nil,
        toolCallId: String? = nil,
        toolCalls: [ToolCall]? = nil
    ) {
        self.role = role
        self.content = content
        self.name = name
        self.toolCallId = toolCallId
        self.toolCalls = toolCalls
    }
    
    /// Create a system message.
    public static func system(_ content: String) -> Message {
        Message(role: .system, content: content)
    }
    
    /// Create a user message.
    public static func user(_ content: String) -> Message {
        Message(role: .user, content: content)
    }
    
    /// Create an assistant message.
    public static func assistant(_ content: String, toolCalls: [ToolCall]? = nil) -> Message {
        Message(role: .assistant, content: content, toolCalls: toolCalls)
    }
    
    /// Create a tool response message.
    public static func tool(_ content: String, toolCallId: String) -> Message {
        Message(role: .tool, content: content, toolCallId: toolCallId)
    }
}

// MARK: - Tool Definitions

/// A tool function definition.
public struct ToolFunction: Codable, Sendable {
    public let name: String
    public let description: String?
    public let parameters: [String: AnyCodable]?
    
    public init(name: String, description: String? = nil, parameters: [String: AnyCodable]? = nil) {
        self.name = name
        self.description = description
        self.parameters = parameters
    }
}

/// A tool definition.
public struct Tool: Codable, Sendable {
    public let type: String
    public let function: ToolFunction
    
    public init(function: ToolFunction) {
        self.type = "function"
        self.function = function
    }
}

/// Tool choice specification.
public enum ToolChoice: Codable, Sendable {
    case auto
    case none
    case function(name: String)
    
    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .auto:
            try container.encode("auto")
        case .none:
            try container.encode("none")
        case .function(let name):
            try container.encode(["type": "function", "function": ["name": name]])
        }
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let str = try? container.decode(String.self) {
            switch str {
            case "auto": self = .auto
            case "none": self = .none
            default: throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid tool choice")
            }
        } else {
            let dict = try container.decode([String: [String: String]].self)
            if let name = dict["function"]?["name"] {
                self = .function(name: name)
            } else {
                throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid tool choice")
            }
        }
    }
}

// MARK: - Request Types

/// Chat completion request.
public struct ChatCompletionRequest: Encodable, Sendable {
    public let model: String
    public let messages: [Message]
    public let temperature: Double?
    public let topP: Double?
    public let maxTokens: Int?
    public let stream: Bool
    public let tools: [Tool]?
    public let toolChoice: ToolChoice?
    public let stop: [String]?
    public let presencePenalty: Double?
    public let frequencyPenalty: Double?
    public let user: String?
    
    enum CodingKeys: String, CodingKey {
        case model, messages, temperature, stream, tools, stop, user
        case topP = "top_p"
        case maxTokens = "max_tokens"
        case toolChoice = "tool_choice"
        case presencePenalty = "presence_penalty"
        case frequencyPenalty = "frequency_penalty"
    }
    
    public init(
        model: String,
        messages: [Message],
        temperature: Double? = nil,
        topP: Double? = nil,
        maxTokens: Int? = nil,
        stream: Bool = false,
        tools: [Tool]? = nil,
        toolChoice: ToolChoice? = nil,
        stop: [String]? = nil,
        presencePenalty: Double? = nil,
        frequencyPenalty: Double? = nil,
        user: String? = nil
    ) {
        self.model = model
        self.messages = messages
        self.temperature = temperature
        self.topP = topP
        self.maxTokens = maxTokens
        self.stream = stream
        self.tools = tools
        self.toolChoice = toolChoice
        self.stop = stop
        self.presencePenalty = presencePenalty
        self.frequencyPenalty = frequencyPenalty
        self.user = user
    }
}

/// Embedding request.
public struct EmbeddingRequest: Encodable, Sendable {
    public let model: String
    public let input: [String]
    public let encodingFormat: String?
    public let dimensions: Int?
    public let user: String?
    
    enum CodingKeys: String, CodingKey {
        case model, input, dimensions, user
        case encodingFormat = "encoding_format"
    }
    
    public init(
        model: String,
        input: [String],
        encodingFormat: String? = nil,
        dimensions: Int? = nil,
        user: String? = nil
    ) {
        self.model = model
        self.input = input
        self.encodingFormat = encodingFormat
        self.dimensions = dimensions
        self.user = user
    }
    
    public init(
        model: String,
        input: String,
        encodingFormat: String? = nil,
        dimensions: Int? = nil,
        user: String? = nil
    ) {
        self.init(model: model, input: [input], encodingFormat: encodingFormat, dimensions: dimensions, user: user)
    }
}

// MARK: - Response Types

/// Chat completion choice.
public struct ChatCompletionChoice: Codable, Sendable {
    public let index: Int
    public let message: Message
    public let finishReason: String?
    
    enum CodingKeys: String, CodingKey {
        case index, message
        case finishReason = "finish_reason"
    }
}

/// Token usage statistics.
public struct Usage: Codable, Sendable {
    public let promptTokens: Int
    public let completionTokens: Int
    public let totalTokens: Int
    
    enum CodingKeys: String, CodingKey {
        case promptTokens = "prompt_tokens"
        case completionTokens = "completion_tokens"
        case totalTokens = "total_tokens"
    }
}

/// Chat completion response.
public struct ChatCompletionResponse: Codable, Sendable {
    public let id: String
    public let object: String
    public let created: Int
    public let model: String
    public let choices: [ChatCompletionChoice]
    public let usage: Usage?
    
    /// Get the content from the first choice.
    public var content: String? {
        choices.first?.message.content
    }
    
    /// Get tool calls from the first choice.
    public var toolCalls: [ToolCall]? {
        choices.first?.message.toolCalls
    }
}

/// Chat completion chunk for streaming.
public struct ChatCompletionChunk: Codable, Sendable {
    public let id: String
    public let object: String
    public let created: Int
    public let model: String
    public let choices: [ChunkChoice]
    
    public struct ChunkChoice: Codable, Sendable {
        public let index: Int
        public let delta: Delta
        public let finishReason: String?
        
        enum CodingKeys: String, CodingKey {
            case index, delta
            case finishReason = "finish_reason"
        }
    }
    
    public struct Delta: Codable, Sendable {
        public let role: MessageRole?
        public let content: String?
        public let toolCalls: [PartialToolCall]?
        
        enum CodingKeys: String, CodingKey {
            case role, content
            case toolCalls = "tool_calls"
        }
    }
    
    public struct PartialToolCall: Codable, Sendable {
        public let index: Int?
        public let id: String?
        public let type: String?
        public let function: PartialFunction?
    }
    
    public struct PartialFunction: Codable, Sendable {
        public let name: String?
        public let arguments: String?
    }
}

/// Single embedding.
public struct Embedding: Codable, Sendable {
    public let index: Int
    public let object: String
    public let embedding: [Float]
}

/// Embedding response.
public struct EmbeddingResponse: Codable, Sendable {
    public let object: String
    public let data: [Embedding]
    public let model: String
    public let usage: EmbeddingUsage
    
    public struct EmbeddingUsage: Codable, Sendable {
        public let promptTokens: Int
        public let totalTokens: Int
        
        enum CodingKeys: String, CodingKey {
            case promptTokens = "prompt_tokens"
            case totalTokens = "total_tokens"
        }
    }
}

/// Model information.
public struct Model: Codable, Sendable, Identifiable {
    public let id: String
    public let object: String
    public let created: Int
    public let ownedBy: String
    
    enum CodingKeys: String, CodingKey {
        case id, object, created
        case ownedBy = "owned_by"
    }
}

/// Models list response.
public struct ModelsResponse: Codable, Sendable {
    public let object: String
    public let data: [Model]
}

// MARK: - AI Client

/// AI client for chat completions, embeddings, and model listing.
public final class AIClient: @unchecked Sendable {
    private let ctx: RequestContext
    
    /// Create an AIClient.
    ///
    /// - Parameter ctx: Request context for API calls
    public init(_ ctx: RequestContext) {
        self.ctx = ctx
    }
    
    // MARK: - Chat Completions
    
    /// Create a chat completion.
    ///
    /// - Parameter request: Chat completion request
    /// - Returns: Chat completion response
    public func chatCompletion(_ request: ChatCompletionRequest) async throws -> ChatCompletionResponse {
        var req = request
        req = ChatCompletionRequest(
            model: req.model,
            messages: req.messages,
            temperature: req.temperature,
            topP: req.topP,
            maxTokens: req.maxTokens,
            stream: false,
            tools: req.tools,
            toolChoice: req.toolChoice,
            stop: req.stop,
            presencePenalty: req.presencePenalty,
            frequencyPenalty: req.frequencyPenalty,
            user: req.user
        )
        
        return try await ReactorShared.request(ctx, path: "ai/v1/chat/completions", method: .post, body: req)
    }
    
    /// Create a streaming chat completion.
    ///
    /// - Parameter request: Chat completion request
    /// - Returns: AsyncThrowingStream of chat completion chunks
    public func chatCompletionStream(_ request: ChatCompletionRequest) -> AsyncThrowingStream<ChatCompletionChunk, Error> {
        var req = request
        req = ChatCompletionRequest(
            model: req.model,
            messages: req.messages,
            temperature: req.temperature,
            topP: req.topP,
            maxTokens: req.maxTokens,
            stream: true,
            tools: req.tools,
            toolChoice: req.toolChoice,
            stop: req.stop,
            presencePenalty: req.presencePenalty,
            frequencyPenalty: req.frequencyPenalty,
            user: req.user
        )
        
        return AsyncThrowingStream { continuation in
            Task {
                do {
                    let url = ctx.baseURL.appendingPathComponent("ai/v1/chat/completions")
                    
                    var headers: [String: String] = [
                        "Content-Type": "application/json",
                        "Accept": "text/event-stream"
                    ]
                    if let projectKey = ctx.projectKey {
                        headers["X-Reactor-Project-Key"] = projectKey
                    }
                    if let tokenProvider = ctx.accessTokenProvider, let token = await tokenProvider() {
                        headers["Authorization"] = "Bearer \(token)"
                    }
                    
                    let encoder = JSONEncoder()
                    let bodyData = try encoder.encode(req)
                    
                    let options = HTTPRequestOptions(method: .post, headers: headers, body: bodyData)
                    
                    for try await chunk in ctx.httpClient.stream(url, options: options) {
                        let text = String(data: chunk, encoding: .utf8) ?? ""
                        let lines = text.components(separatedBy: "\n")
                        
                        for line in lines {
                            let trimmed = line.trimmingCharacters(in: .whitespaces)
                            guard trimmed.hasPrefix("data: ") else { continue }
                            
                            let data = String(trimmed.dropFirst(6))
                            if data == "[DONE]" {
                                continuation.finish()
                                return
                            }
                            
                            if let jsonData = data.data(using: .utf8) {
                                let decoder = JSONDecoder()
                                if let chunk = try? decoder.decode(ChatCompletionChunk.self, from: jsonData) {
                                    continuation.yield(chunk)
                                }
                            }
                        }
                    }
                    
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }
    
    /// Create a simple chat completion with just a model and prompt.
    ///
    /// - Parameters:
    ///   - model: Model ID or alias
    ///   - prompt: User prompt
    ///   - systemPrompt: Optional system prompt
    ///   - maxTokens: Maximum tokens to generate
    /// - Returns: The assistant's response content
    public func chat(
        model: String,
        prompt: String,
        systemPrompt: String? = nil,
        maxTokens: Int? = nil
    ) async throws -> String {
        var messages: [Message] = []
        if let systemPrompt {
            messages.append(.system(systemPrompt))
        }
        messages.append(.user(prompt))
        
        let request = ChatCompletionRequest(
            model: model,
            messages: messages,
            maxTokens: maxTokens
        )
        
        let response = try await chatCompletion(request)
        return response.content ?? ""
    }
    
    // MARK: - Embeddings
    
    /// Create embeddings for the given input.
    ///
    /// - Parameter request: Embedding request
    /// - Returns: Embedding response
    public func embed(_ request: EmbeddingRequest) async throws -> EmbeddingResponse {
        try await ReactorShared.request(ctx, path: "ai/v1/embeddings", method: .post, body: request)
    }
    
    /// Create embeddings for a single text.
    ///
    /// - Parameters:
    ///   - model: Model ID
    ///   - text: Text to embed
    /// - Returns: The embedding vector
    public func embed(model: String, text: String) async throws -> [Float] {
        let request = EmbeddingRequest(model: model, input: text)
        let response = try await embed(request)
        return response.data.first?.embedding ?? []
    }
    
    /// Create embeddings for multiple texts.
    ///
    /// - Parameters:
    ///   - model: Model ID
    ///   - texts: Texts to embed
    /// - Returns: Array of embedding vectors
    public func embed(model: String, texts: [String]) async throws -> [[Float]] {
        let request = EmbeddingRequest(model: model, input: texts)
        let response = try await embed(request)
        return response.data.map(\.embedding)
    }
    
    // MARK: - Models
    
    /// List available models.
    ///
    /// - Returns: List of available models
    public func listModels() async throws -> [Model] {
        let response: ModelsResponse = try await ReactorShared.request(ctx, path: "ai/v1/models")
        return response.data
    }
}

// MARK: - Stream Collector

extension AIClient {
    /// Collect all chunks from a streaming response into a single response.
    ///
    /// - Parameter stream: The stream to collect
    /// - Returns: Collected chat completion response
    public func collect(_ stream: AsyncThrowingStream<ChatCompletionChunk, Error>) async throws -> ChatCompletionResponse {
        var id = ""
        var model = ""
        var created = 0
        var content = ""
        var finishReason: String?
        var toolCalls: [ToolCall] = []
        var currentToolCall: (id: String, name: String, arguments: String)?
        
        for try await chunk in stream {
            id = chunk.id
            model = chunk.model
            created = chunk.created
            
            for choice in chunk.choices {
                if let deltaContent = choice.delta.content {
                    content += deltaContent
                }
                if let reason = choice.finishReason {
                    finishReason = reason
                }
                
                if let deltaToolCalls = choice.delta.toolCalls {
                    for tc in deltaToolCalls {
                        if let tcId = tc.id {
                            if let current = currentToolCall {
                                toolCalls.append(ToolCall(
                                    id: current.id,
                                    type: "function",
                                    function: ToolCall.FunctionCall(name: current.name, arguments: current.arguments)
                                ))
                            }
                            currentToolCall = (tcId, tc.function?.name ?? "", tc.function?.arguments ?? "")
                        } else if var current = currentToolCall {
                            if let args = tc.function?.arguments {
                                current.arguments += args
                            }
                            currentToolCall = current
                        }
                    }
                }
            }
        }
        
        if let current = currentToolCall {
            toolCalls.append(ToolCall(
                id: current.id,
                type: "function",
                function: ToolCall.FunctionCall(name: current.name, arguments: current.arguments)
            ))
        }
        
        let message = Message(
            role: .assistant,
            content: content.isEmpty ? nil : content,
            toolCalls: toolCalls.isEmpty ? nil : toolCalls
        )
        
        return ChatCompletionResponse(
            id: id,
            object: "chat.completion",
            created: created,
            model: model,
            choices: [ChatCompletionChoice(index: 0, message: message, finishReason: finishReason)],
            usage: nil
        )
    }
}
