import { useState, useEffect, useRef } from 'react'
import { ChevronDown, Brain } from 'lucide-react'
import { ToolCallDisplay } from './ToolCallDisplay'
import { StreamingMarkdown } from './StreamingMarkdown'
import { cn } from '@/lib/utils'
import type { ResponseEvent, Message } from '@/hooks/useChat'

type ToolCall = NonNullable<Message['toolCalls']>[number]

interface AgentResponseProps {
  content: string
  toolCalls?: ToolCall[]
  thinkingTime?: number
  thinking?: string
  isStreaming?: boolean
  events?: ResponseEvent[]
}

const THINKING_PHRASES = [
  'Thinking...',
  'Processing...',
  'Working on it...',
  'Analyzing...',
  'Considering options...',
]

const LONG_WAIT_PHRASES = [
  'Still working...',
  'This is taking a moment...',
  'Almost there...',
  'Finishing up...',
]

function ThinkingIndicator({ isWaitingForTool = false }: { isWaitingForTool?: boolean }) {
  const [phraseIndex, setPhraseIndex] = useState(0)
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const startTimeRef = useRef(Date.now())

  useEffect(() => {
    startTimeRef.current = Date.now()
    setPhraseIndex(0)
    setElapsedSeconds(0)

    const phraseInterval = setInterval(() => {
      const elapsed = Math.floor((Date.now() - startTimeRef.current) / 1000)
      setElapsedSeconds(elapsed)

      if (elapsed >= 30) {
        setPhraseIndex((prev) => (prev + 1) % LONG_WAIT_PHRASES.length)
      } else {
        setPhraseIndex((prev) => (prev + 1) % THINKING_PHRASES.length)
      }
    }, 10000)

    const timeInterval = setInterval(() => {
      setElapsedSeconds(Math.floor((Date.now() - startTimeRef.current) / 1000))
    }, 1000)

    return () => {
      clearInterval(phraseInterval)
      clearInterval(timeInterval)
    }
  }, [])

  const phrases = elapsedSeconds >= 30 ? LONG_WAIT_PHRASES : THINKING_PHRASES
  const currentPhrase = isWaitingForTool ? 'Running tool...' : phrases[phraseIndex % phrases.length]

  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground py-2">
      <span className="inline-flex items-center gap-0.5">
        {[0, 1, 2].map((i) => (
          <span
            key={i}
            className="w-1.5 h-1.5 rounded-full bg-blue-500"
            style={{
              animation: 'dot-pulse 1.4s ease-in-out infinite',
              animationDelay: `${i * 0.2}s`,
            }}
          />
        ))}
      </span>
      <span>{currentPhrase}</span>
      {elapsedSeconds >= 5 && (
        <span className="text-muted-foreground/50">({elapsedSeconds}s)</span>
      )}
    </div>
  )
}

function ThinkingBlock({ content, isStreaming }: { content: string; isStreaming?: boolean }) {
  const [isExpanded, setIsExpanded] = useState(false)

  if (!content) return null

  const lines = content.split('\n')
  const preview = lines.slice(0, 3).join('\n')
  const hasMore = lines.length > 3 || content.length > 200

  return (
    <div className="border-l-2 border-muted pl-3 py-1">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors mb-1"
      >
        <Brain className="w-3 h-3" />
        <span>Thinking</span>
        {hasMore && (
          <ChevronDown className={cn('w-3 h-3 transition-transform', isExpanded && 'rotate-180')} />
        )}
        {isStreaming && <span className="animate-pulse">...</span>}
      </button>
      <div
        className={cn(
          'text-xs text-muted-foreground/70 whitespace-pre-wrap font-mono leading-relaxed',
          !isExpanded && hasMore && 'line-clamp-3'
        )}
      >
        {isExpanded ? content : preview}
      </div>
    </div>
  )
}

export function AgentResponse({
  content,
  toolCalls,
  thinking,
  isStreaming = false,
  events,
}: AgentResponseProps) {
  const hasEvents = events && events.length > 0

  const hasNoContent = !content && !thinking && (!events || events.length === 0)
  const showInitialThinking = isStreaming && hasNoContent

  const lastEvent = events?.[events.length - 1]
  const isWaitingForToolResult = isStreaming && lastEvent?.type === 'tool_call' && (lastEvent.toolCall as ToolCall).status !== 'complete'

  if (hasEvents) {
    type GroupedEvent =
      | { type: 'thinking'; content: string; index: number }
      | { type: 'tool_calls'; toolCalls: ToolCall[]; startIndex: number }
      | { type: 'text'; content: string; index: number }

    const groupedEvents: GroupedEvent[] = []

    for (let i = 0; i < events.length; i++) {
      const event = events[i]

      if (event.type === 'tool_call') {
        const lastGrouped = groupedEvents[groupedEvents.length - 1]
        if (lastGrouped?.type === 'tool_calls') {
          lastGrouped.toolCalls.push(event.toolCall as ToolCall)
        } else {
          groupedEvents.push({ type: 'tool_calls', toolCalls: [event.toolCall as ToolCall], startIndex: i })
        }
      } else if (event.type === 'thinking') {
        groupedEvents.push({ type: 'thinking', content: event.content, index: i })
      } else if (event.type === 'text') {
        groupedEvents.push({ type: 'text', content: event.content, index: i })
      }
    }

    return (
      <div className="space-y-2">
        {showInitialThinking && <ThinkingIndicator />}

        {groupedEvents.map((grouped, gIndex) => {
          const isLastGroup = gIndex === groupedEvents.length - 1

          switch (grouped.type) {
            case 'thinking':
              return (
                <ThinkingBlock
                  key={`thinking-${grouped.index}`}
                  content={grouped.content}
                  isStreaming={isStreaming && isLastGroup}
                />
              )
            case 'tool_calls':
              const hasRunningTool = grouped.toolCalls.some(tc => tc.status !== 'complete')
              return (
                <ToolCallDisplay
                  key={`tools-${grouped.startIndex}`}
                  toolCalls={grouped.toolCalls}
                  isStreaming={isStreaming && hasRunningTool}
                />
              )
            case 'text':
              return (
                <StreamingMarkdown
                  key={`text-${grouped.index}`}
                  content={grouped.content}
                  isStreaming={isStreaming && isLastGroup}
                />
              )
            default:
              return null
          }
        })}

        {isStreaming && !isWaitingForToolResult && lastEvent?.type !== 'text' && (
          <ThinkingIndicator isWaitingForTool={isWaitingForToolResult} />
        )}
      </div>
    )
  }

  const hasActivity = toolCalls && toolCalls.length > 0
  const hasPendingTool = toolCalls?.some(tc => tc.status === 'running' || tc.status === 'pending')

  return (
    <div className="space-y-2">
      {showInitialThinking && <ThinkingIndicator />}

      {thinking && (
        <ThinkingBlock content={thinking} isStreaming={isStreaming && !content} />
      )}

      {hasActivity && (
        <ToolCallDisplay toolCalls={toolCalls} isStreaming={isStreaming} />
      )}

      {content && (
        <StreamingMarkdown content={content} isStreaming={isStreaming} />
      )}

      {isStreaming && !hasPendingTool && hasActivity && !content && (
        <ThinkingIndicator />
      )}
    </div>
  )
}
