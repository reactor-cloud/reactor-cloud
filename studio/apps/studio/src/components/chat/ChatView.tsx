import { forwardRef, useImperativeHandle, useCallback, useState } from 'react'
import { ArrowLeft, Activity, Plus, ArrowRightToLine, CornerDownLeft, Users } from 'lucide-react'
import { MessageList } from './MessageList'
import { PromptInput } from './PromptInput'
import { useAgents } from '@/hooks/useAgents'
import { useChat } from '@/hooks/useChat'
import { AgentIcon } from './AgentIcon'
import { cn } from '@/lib/utils'

interface ChatViewProps {
  agentId: string
  conversationId: string
  onBack?: () => void
  onNewChat?: () => void
  onPopOut?: () => void
  onOpenTrace?: (conversationId: string) => void
  className?: string
}

export interface ChatViewRef {
  sendMessage: (message: string) => Promise<{ success: boolean }>
  getModel: () => { modelId: string; provider: string }
  setModel: (modelId: string, provider: string) => void
}

export const ChatView = forwardRef<ChatViewRef, ChatViewProps>(({
  agentId,
  conversationId,
  onBack,
  onNewChat,
  onPopOut,
  onOpenTrace,
  className,
}, ref) => {
  const { getAgent } = useAgents()
  const agent = getAgent(agentId)

  const [parentInfo] = useState<{
    parentConversationId: string
    parentAgentId: string
    description: string
  } | null>(null)

  const {
    messages,
    isStreaming,
    error,
    send,
    stop,
  } = useChat({
    agentId,
    conversationId,
  })

  const handleSend = useCallback(async (message: string) => {
    await send(message)
  }, [send])

  const handleStop = useCallback(() => {
    stop()
  }, [stop])

  // Extract model info from agent model string (e.g., "anthropic/claude-sonnet-4.5")
  const modelInfo = agent?.model?.split('/') || []
  const provider = modelInfo[0] || 'unknown'
  const modelId = modelInfo[1] || agent?.model || 'unknown'

  useImperativeHandle(ref, () => ({
    sendMessage: async (message: string) => {
      try {
        await send(message)
        return { success: true }
      } catch {
        return { success: false }
      }
    },
    getModel: () => ({
      modelId,
      provider,
    }),
    setModel: () => {
      // Model is defined by agent config, not changeable per-conversation yet
    },
  }), [send, modelId, provider])

  if (!agent) return null

  const handleOpenTrace = useCallback(() => {
    onOpenTrace?.(conversationId)
  }, [conversationId, onOpenTrace])

  const handleNewChat = useCallback(() => {
    onNewChat?.()
  }, [onNewChat])

  const handleOpenParent = useCallback(() => {
    if (!parentInfo) return
    console.log('Open parent:', parentInfo.parentConversationId)
  }, [parentInfo])

  const handlePopOut = useCallback(() => {
    onPopOut?.()
  }, [onPopOut])

  return (
    <div className={cn('flex-1 flex flex-col min-h-0 overflow-hidden', className)}>
      <div className="h-12 flex-shrink-0 flex items-center justify-between px-3 border-b border-border">
        <div className="flex items-center gap-3">
          {onBack && (
            <button
              onClick={onBack}
              className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            >
              <ArrowLeft className="w-4 h-4" />
            </button>
          )}
          <div
            className="w-7 h-7 rounded-full flex items-center justify-center"
            style={{ backgroundColor: `${agent.color}20` }}
          >
            <AgentIcon agent={agent} size="sm" />
          </div>
          <span className="font-medium text-sm text-foreground">{agent.name}</span>
        </div>

        <div className="flex items-center gap-1">
          {onPopOut && (
            <button
              onClick={handlePopOut}
              className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              title="Open in main pane"
            >
              <ArrowRightToLine className="w-4 h-4" />
            </button>
          )}
          {onNewChat && (
            <button
              onClick={handleNewChat}
              className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              title="New chat"
            >
              <Plus className="w-4 h-4" />
            </button>
          )}
          <button
            onClick={handleOpenTrace}
            className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            title="View trace"
          >
            <Activity className="w-4 h-4" />
          </button>
        </div>
      </div>

      {error && (
        <div className="flex-shrink-0 px-4 py-2 bg-destructive/10 border-b border-destructive/20 text-sm text-destructive">
          {error}
        </div>
      )}

      {parentInfo && (
        <button
          onClick={handleOpenParent}
          className="flex-shrink-0 flex items-center gap-2 px-4 py-2 border-b border-border bg-muted/30 hover:bg-muted/50 transition-colors text-left w-full"
        >
          <CornerDownLeft className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
          <Users className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
          <span className="text-xs text-muted-foreground">Sub-agent of</span>
          <span className="text-xs font-medium text-foreground">
            {getAgent(parentInfo.parentAgentId)?.name || 'Parent'}
          </span>
          {parentInfo.description && (
            <>
              <span className="text-muted-foreground/50">·</span>
              <span className="text-xs text-muted-foreground truncate">
                "{parentInfo.description}"
              </span>
            </>
          )}
        </button>
      )}

      <MessageList messages={messages} isStreaming={isStreaming} />
      
      <div className="p-3 border-t border-border">
        <PromptInput
          onSubmit={handleSend}
          onCancel={handleStop}
          isStreaming={isStreaming}
          placeholder="Type @ to mention files..."
        />
        <div className="flex items-center justify-between mt-2 px-1">
          <div className="flex items-center">
            <span className="text-xs text-muted-foreground">{agent.model}</span>
          </div>
        </div>
      </div>
    </div>
  )
})

ChatView.displayName = 'ChatView'
