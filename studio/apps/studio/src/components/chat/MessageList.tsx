import { useRef, useEffect, useCallback, useMemo } from 'react'
import { ScrollArea } from '@/components/ui/ScrollArea'
import { UserMessageBubble } from './UserMessageBubble'
import { AgentResponse } from './AgentResponse'
import { MessageSquare } from 'lucide-react'
import type { Message, ResponseEvent } from '@/hooks/useChat'

interface MergedMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  toolCalls?: Message['toolCalls']
  thinking?: string
  events?: ResponseEvent[]
  isStreaming?: boolean
}

interface MessageListProps {
  messages: Message[]
  isStreaming: boolean
}

function mergeConsecutiveAssistantMessages(messages: Message[]): MergedMessage[] {
  const merged: MergedMessage[] = []

  for (const msg of messages) {
    if (msg.role === 'user' && !msg.content?.trim()) {
      continue
    }

    if (msg.role === 'user') {
      merged.push({
        id: msg.id,
        role: 'user',
        content: msg.content,
      })
    } else {
      const last = merged[merged.length - 1]

      if (last?.role === 'assistant') {
        if (msg.toolCalls && msg.toolCalls.length > 0) {
          last.toolCalls = [...(last.toolCalls || []), ...msg.toolCalls]
        }
        if (msg.content) {
          if (last.content) {
            last.content += '\n\n' + msg.content
          } else {
            last.content = msg.content
          }
        }
        if (msg.thinking) {
          if (last.thinking) {
            last.thinking += '\n\n' + msg.thinking
          } else {
            last.thinking = msg.thinking
          }
        }
        if (msg.events && msg.events.length > 0) {
          last.events = [...(last.events || []), ...msg.events]
        }
      } else {
        merged.push({
          id: msg.id,
          role: 'assistant',
          content: msg.content,
          toolCalls: msg.toolCalls ? [...msg.toolCalls] : undefined,
          thinking: msg.thinking,
          events: msg.events ? [...msg.events] : undefined,
          isStreaming: msg.isStreaming,
        })
      }
    }
  }

  return merged
}

export function MessageList({ messages, isStreaming }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const prevMessageCountRef = useRef(0)
  const userScrolledRef = useRef(false)
  const scrollAreaRef = useRef<HTMLDivElement>(null)

  const displayMessages = useMemo(() => mergeConsecutiveAssistantMessages(messages), [messages])

  const handleScroll = useCallback(() => {
    if (!scrollAreaRef.current) return
    const scrollElement = scrollAreaRef.current.querySelector('[data-radix-scroll-area-viewport]')
    if (!scrollElement) return

    const { scrollTop, scrollHeight, clientHeight } = scrollElement
    const isNearBottom = scrollHeight - scrollTop - clientHeight < 100
    userScrolledRef.current = !isNearBottom
  }, [])

  useEffect(() => {
    const isNewMessage = messages.length > prevMessageCountRef.current
    prevMessageCountRef.current = messages.length

    if (isNewMessage && !userScrolledRef.current) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages.length])

  if (displayMessages.length === 0 && !isStreaming) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
        <div className="w-12 h-12 rounded-xl bg-muted flex items-center justify-center mb-4">
          <MessageSquare className="w-6 h-6 text-muted-foreground" />
        </div>
        <p className="text-sm text-foreground font-medium mb-1">Start a conversation</p>
        <p className="text-sm text-muted-foreground">
          Ask a question or describe what you need help with.
        </p>
      </div>
    )
  }

  const lastRawMessage = messages[messages.length - 1]
  const showPendingIndicator = isStreaming && lastRawMessage?.role === 'user' && lastRawMessage?.content?.trim()

  return (
    <ScrollArea ref={scrollAreaRef} className="flex-1 min-h-0 p-4" onScrollCapture={handleScroll}>
      <div className="space-y-4 max-w-3xl mx-auto">
        {displayMessages.map((message, index) => {
          const isLastMessage = index === displayMessages.length - 1
          const isMessageStreaming = isLastMessage && isStreaming && message.role === 'assistant'

          return message.role === 'user' ? (
            <UserMessageBubble key={message.id} content={message.content} />
          ) : (
            <AgentResponse
              key={message.id}
              content={message.content}
              toolCalls={message.toolCalls}
              thinking={message.thinking}
              isStreaming={isMessageStreaming}
              events={message.events}
            />
          )
        })}

        {showPendingIndicator && (
          <AgentResponse
            content=""
            isStreaming={true}
          />
        )}

        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  )
}
