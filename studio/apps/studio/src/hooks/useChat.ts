import { useState, useEffect, useCallback, useRef } from 'react'
import {
  agentSend,
  agentCancel,
  conversationMessages,
  onAgentChunk,
  onAgentComplete,
  onAgentError,
  type StreamChunk,
} from '@/lib/ipc'

interface ToolCall {
  id: string
  name: string
  arguments: Record<string, unknown>
  status?: 'pending' | 'running' | 'complete' | 'error'
  sessionId?: string
  childConversationId?: string
}

interface ToolResult {
  toolCallId: string
  name?: string
  result: unknown
  error?: string
  childConversationId?: string
}

export type ResponseEvent =
  | { type: 'text'; content: string }
  | { type: 'tool_call'; toolCall: ToolCall }
  | { type: 'thinking'; content: string }

export interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
  toolCalls?: ToolCall[]
  toolResults?: ToolResult[]
  thinking?: string
  isStreaming?: boolean
  events?: ResponseEvent[]
}

interface UseChatOptions {
  agentId: string
  conversationId: string | null
}

function generateId(): string {
  return `msg-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
}

export function useChat({ agentId, conversationId }: UseChatOptions) {
  const [messages, setMessages] = useState<Message[]>([])
  const [isStreaming, setIsStreaming] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const streamingMessageRef = useRef<Message | null>(null)
  const currentConversationRef = useRef(conversationId)

  useEffect(() => {
    currentConversationRef.current = conversationId
  }, [conversationId])

  useEffect(() => {
    const loadConversation = async () => {
      if (!conversationId) {
        setMessages([])
        return
      }

      try {
        const msgs = await conversationMessages(conversationId)
        setMessages(
          (msgs as Array<Message & { timestamp: Date | string }>).map((m) => ({
            ...m,
            timestamp: new Date(m.timestamp),
          }))
        )
      } catch (err) {
        console.error('Failed to load conversation:', err)
        setMessages([])
      }
    }

    loadConversation()
  }, [conversationId])

  useEffect(() => {
    let unsubChunk: (() => void) | undefined
    let unsubComplete: (() => void) | undefined
    let unsubError: (() => void) | undefined

    const setupListeners = async () => {
      unsubChunk = await onAgentChunk(({ conversationId: convId, chunk }) => {
        if (convId !== currentConversationRef.current) return

        setMessages((prev) => {
          const newMessages = [...prev]
          const lastIdx = newMessages.length - 1

          let currentMsg: Message
          const needsNewMessage =
            lastIdx < 0 ||
            newMessages[lastIdx].role !== 'assistant' ||
            !newMessages[lastIdx].isStreaming

          if (needsNewMessage) {
            currentMsg = {
              id: generateId(),
              role: 'assistant',
              content: '',
              timestamp: new Date(),
              isStreaming: true,
              toolCalls: [],
              toolResults: [],
              events: [],
            }
            newMessages.push(currentMsg)
          } else {
            currentMsg = {
              ...newMessages[lastIdx],
              events: [...(newMessages[lastIdx].events || [])],
            }
          }

          const events = currentMsg.events || []

          handleChunk(chunk, currentMsg, events)

          currentMsg.events = events

          if (needsNewMessage) {
            newMessages[newMessages.length - 1] = currentMsg
          } else {
            newMessages[lastIdx] = currentMsg
          }
          streamingMessageRef.current = currentMsg
          return newMessages
        })
      })

      unsubComplete = await onAgentComplete(({ conversationId: convId }) => {
        if (convId !== currentConversationRef.current) return

        setMessages((prev) => {
          const newMessages = [...prev]
          const lastIdx = newMessages.length - 1

          if (lastIdx >= 0 && newMessages[lastIdx].isStreaming) {
            newMessages[lastIdx] = {
              ...newMessages[lastIdx],
              isStreaming: false,
            }
          }

          return newMessages
        })

        setIsStreaming(false)
        streamingMessageRef.current = null
      })

      unsubError = await onAgentError(({ conversationId: convId, error: errorMsg }) => {
        if (convId !== currentConversationRef.current) return
        setError(errorMsg)
        setIsStreaming(false)
      })
    }

    setupListeners()

    return () => {
      unsubChunk?.()
      unsubComplete?.()
      unsubError?.()
    }
  }, [])

  const send = useCallback(
    async (message: string, contextFiles?: string[]) => {
      if (!conversationId) {
        setError('No conversation selected')
        return
      }

      if (isStreaming) return

      setError(null)
      setIsStreaming(true)

      const displayMessage =
        contextFiles && contextFiles.length > 0
          ? `${contextFiles.map((f) => `@${f}`).join(' ')} ${message}`
          : message

      const userMessage: Message = {
        id: generateId(),
        role: 'user',
        content: displayMessage,
        timestamp: new Date(),
      }

      setMessages((prev) => [...prev, userMessage])

      try {
        await agentSend(agentId, conversationId, message)
      } catch (err) {
        setError((err as Error).message)
        setIsStreaming(false)
      }
    },
    [agentId, conversationId, isStreaming]
  )

  const stop = useCallback(async () => {
    if (!conversationId) return

    try {
      await agentCancel(conversationId)
    } catch (err) {
      console.error('Failed to stop:', err)
    }
    setIsStreaming(false)
  }, [conversationId])

  const clearMessages = useCallback(() => {
    setMessages([])
  }, [])

  return {
    messages,
    isStreaming,
    error,
    send,
    stop,
    clearMessages,
  }
}

function handleChunk(
  chunk: StreamChunk,
  currentMsg: Message,
  events: ResponseEvent[]
) {
  switch (chunk.type) {
    case 'text':
      currentMsg.content += chunk.content || ''
      if (events.length > 0 && events[events.length - 1].type === 'text') {
        const lastEvent = events[events.length - 1] as { type: 'text'; content: string }
        events[events.length - 1] = {
          type: 'text',
          content: lastEvent.content + (chunk.content || ''),
        }
      } else {
        events.push({ type: 'text', content: chunk.content || '' })
      }
      break
    case 'thinking':
      currentMsg.thinking = (currentMsg.thinking || '') + (chunk.content || '')
      if (events.length > 0 && events[events.length - 1].type === 'thinking') {
        const lastEvent = events[events.length - 1] as { type: 'thinking'; content: string }
        events[events.length - 1] = {
          type: 'thinking',
          content: lastEvent.content + (chunk.content || ''),
        }
      } else {
        events.push({ type: 'thinking', content: chunk.content || '' })
      }
      break
    case 'tool_call':
      if (chunk.id && chunk.name) {
        const toolCall: ToolCall = {
          id: chunk.id,
          name: chunk.name,
          arguments: (chunk.arguments as Record<string, unknown>) || {},
          status: 'running',
        }
        currentMsg.toolCalls = [...(currentMsg.toolCalls || []), toolCall]
        events.push({ type: 'tool_call', toolCall })
      }
      break
    case 'tool_result':
      if (chunk.toolCallId) {
        const toolResult: ToolResult = {
          toolCallId: chunk.toolCallId,
          result: chunk.output,
          error: chunk.isError ? chunk.output as string : undefined,
        }
        currentMsg.toolResults = [...(currentMsg.toolResults || []), toolResult]
        const resultStatus = chunk.isError ? ('error' as const) : ('complete' as const)
        currentMsg.toolCalls = (currentMsg.toolCalls || []).map((tc) =>
          tc.id === chunk.toolCallId ? { ...tc, status: resultStatus } : tc
        )
      }
      break
    case 'error':
      break
    case 'done':
      break
  }
}
