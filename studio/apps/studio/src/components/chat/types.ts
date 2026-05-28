export type MessageRole = 'user' | 'assistant' | 'tool'

export interface ToolCall {
  id: string
  name: string
  arguments: Record<string, unknown>
}

export interface ToolResult {
  toolCallId: string
  output: string
  isError: boolean
}

export interface Message {
  id: string
  role: MessageRole
  content: string
  toolCalls?: ToolCall[]
  toolResult?: ToolResult
  timestamp: Date
}

export interface StreamState {
  isStreaming: boolean
  content: string
  toolCalls: ToolCall[]
  pendingToolResults: ToolResult[]
}
