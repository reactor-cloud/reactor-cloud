import { useState, forwardRef, useImperativeHandle, useRef, useCallback, useEffect } from 'react'
import { ListTodo, ArrowLeft, Plus, Activity } from 'lucide-react'
import { AgentBar } from '@/components/chat/AgentBar'
import { AgentPicker } from '@/components/chat/AgentPicker'
import { ConversationList } from '@/components/chat/ConversationList'
import { ChatView, ChatViewRef } from '@/components/chat/ChatView'
import { TraceView } from '@/components/chat/TraceView'
import { ResizeHandle } from '@/components/ui/ResizeHandle'
import { useResizable } from '@/hooks/useResizable'
import { cn } from '@/lib/utils'
import { useAgents } from '@/hooks/useAgents'
import { useConversations } from '@/hooks/useConversations'
import { useViews } from '@/views'
import { ScrollArea } from '@/components/ui/ScrollArea'

export type ChatPanelView =
  | { type: 'conversations' }
  | { type: 'chat'; conversationId: string }
  | { type: 'agent-picker' }
  | { type: 'tasks' }
  | { type: 'trace'; conversationId: string }

interface ChatPanelProps {
  className?: string
  selectedAgentId?: string | null
  activeConversationId?: string | null
  onAgentSelect?: (agentId: string) => void
  onConversationSelect?: (conversationId: string | null) => void
}

export interface ChatPanelRef {
  selectAgent: (agentId: string) => void
  selectConversation: (conversationId: string) => void
  newConversation: (agentId?: string) => Promise<string | null>
  sendMessage: (message: string) => Promise<{ success: boolean }>
  getModel: () => { modelId: string; provider: string } | null
  setModel: (modelId: string, provider: string) => void
}

export const ChatPanel = forwardRef<ChatPanelRef, ChatPanelProps>(({
  className,
  selectedAgentId: controlledAgentId,
  activeConversationId: controlledConversationId,
  onAgentSelect,
  onConversationSelect,
}, ref) => {
  const { allAgents, refreshAgents, isLoading: isLoadingAgents, activeAgentIds, setActiveAgentIds } = useAgents()
  const { getOpenConversationTabs, switchToConversationTab } = useViews()
  const [internalAgentId, setInternalAgentId] = useState<string>('coder')
  const [view, setView] = useState<ChatPanelView>({ type: 'conversations' })
  const chatViewRef = useRef<ChatViewRef>(null)
  const isPoppingOutRef = useRef(false)

  const selectedAgentId = controlledAgentId ?? internalAgentId
  const openConversationTabs = getOpenConversationTabs()
  const openInTabs = openConversationTabs.map(t => t.conversationId)
  const openInTabsKey = openInTabs.join(',')

  const { create: createConversation } = useConversations({ agentId: selectedAgentId || undefined })

  useEffect(() => {
    if (isPoppingOutRef.current) {
      isPoppingOutRef.current = false
      return
    }

    if (!controlledConversationId) return
    if (openInTabsKey.split(',').includes(controlledConversationId)) return

    setView((prev) => {
      if (prev.type === 'chat' && prev.conversationId === controlledConversationId) {
        return prev
      }
      return { type: 'chat', conversationId: controlledConversationId }
    })
  }, [controlledConversationId, openInTabsKey])

  const { size, startResizing } = useResizable({
    initialSize: 380,
    minSize: 300,
    maxSize: 500,
  })

  const handleSelectAgent = useCallback((agentId: string) => {
    setInternalAgentId(agentId)
    onAgentSelect?.(agentId)
    setView({ type: 'conversations' })
  }, [onAgentSelect])

  const handleTasksClick = useCallback(() => {
    setView({ type: 'tasks' })
  }, [])

  const handleOpenTrace = useCallback((conversationId: string) => {
    setView({ type: 'trace', conversationId })
  }, [])

  const handleAddAgentClick = useCallback(() => {
    refreshAgents()
    setView({ type: 'agent-picker' })
  }, [refreshAgents])

  const handleAgentPicked = (agentId: string) => {
    if (!activeAgentIds.includes(agentId)) {
      setActiveAgentIds([...activeAgentIds, agentId])
    }
    handleSelectAgent(agentId)
  }

  const handleSelectConversation = useCallback((conversationId: string) => {
    if (switchToConversationTab(conversationId)) {
      return
    }
    onConversationSelect?.(conversationId)
    setView({ type: 'chat', conversationId })
  }, [onConversationSelect, switchToConversationTab])

  const handleNavigateToTab = useCallback((conversationId: string) => {
    switchToConversationTab(conversationId)
  }, [switchToConversationTab])

  const handlePopOut = useCallback(() => {
    isPoppingOutRef.current = true
    onConversationSelect?.(null)
    setView({ type: 'conversations' })
  }, [onConversationSelect])

  const handleBackToConversations = () => {
    onConversationSelect?.(null)
    setView({ type: 'conversations' })
  }

  const handleNewConversation = useCallback(async (agentId?: string): Promise<string | null> => {
    if (agentId && agentId !== selectedAgentId) {
      handleSelectAgent(agentId)
    }
    const newId = await createConversation()
    if (newId) {
      onConversationSelect?.(newId)
      setView({ type: 'chat', conversationId: newId })
    }
    return newId
  }, [selectedAgentId, handleSelectAgent, onConversationSelect, createConversation])

  useImperativeHandle(ref, () => ({
    selectAgent: (agentId: string) => {
      handleSelectAgent(agentId)
    },
    selectConversation: (conversationId: string) => {
      handleSelectConversation(conversationId)
    },
    newConversation: async (agentId?: string) => {
      return handleNewConversation(agentId)
    },
    sendMessage: async (message: string) => {
      if (chatViewRef.current) {
        return chatViewRef.current.sendMessage(message)
      }
      return { success: false }
    },
    getModel: () => {
      if (chatViewRef.current) {
        return chatViewRef.current.getModel()
      }
      return null
    },
    setModel: (modelId: string, provider: string) => {
      if (chatViewRef.current) {
        chatViewRef.current.setModel(modelId, provider)
      }
    },
  }), [handleSelectAgent, handleSelectConversation, handleNewConversation])

  return (
    <div className={cn('flex h-full', className)}>
      <AgentBar
        activeAgentIds={activeAgentIds}
        selectedAgentId={selectedAgentId}
        allAgents={allAgents}
        onSelectAgent={handleSelectAgent}
        onAddClick={handleAddAgentClick}
        onTasksClick={handleTasksClick}
        tasksActive={view.type === 'tasks'}
      />

      <div className="flex-1 flex flex-col min-h-0 overflow-hidden bg-card border-r border-border" style={{ width: size }}>
        {view.type === 'agent-picker' && (
          <AgentPicker
            activeAgentIds={activeAgentIds}
            allAgents={allAgents}
            isLoading={isLoadingAgents}
            onSelect={handleAgentPicked}
            onBack={() => setView({ type: 'conversations' })}
            onRefresh={refreshAgents}
          />
        )}

        {view.type === 'tasks' && (
          <div className="flex flex-col h-full">
            <div className="h-12 flex items-center gap-2 px-3 border-b border-border flex-shrink-0">
              <button
                onClick={() => setView({ type: 'conversations' })}
                className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              >
                <ArrowLeft className="w-4 h-4" />
              </button>
              <div className="flex items-center gap-2 flex-1 min-w-0">
                <div className="w-8 h-8 rounded-lg bg-purple-500/20 flex items-center justify-center flex-shrink-0">
                  <ListTodo className="w-4 h-4 text-purple-500" />
                </div>
                <div className="min-w-0">
                  <h2 className="text-sm font-semibold truncate">Tasks</h2>
                  <p className="text-xs text-muted-foreground">Project implementation tasks</p>
                </div>
              </div>
              <button
                className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                title="New Task"
              >
                <Plus className="w-4 h-4" />
              </button>
            </div>
            <ScrollArea className="flex-1">
              <div className="flex flex-col items-center justify-center h-48 text-center px-4">
                <ListTodo className="w-10 h-10 text-muted-foreground/50 mb-3" />
                <p className="text-sm text-muted-foreground">No tasks yet</p>
                <p className="text-xs text-muted-foreground/70 mt-1">
                  Create a task to track a feature implementation
                </p>
              </div>
            </ScrollArea>
          </div>
        )}

        {view.type === 'conversations' && selectedAgentId && (
          <ConversationList
            agentId={selectedAgentId}
            activeConversationId={controlledConversationId || undefined}
            onSelectConversation={handleSelectConversation}
            onNewConversation={() => handleNewConversation()}
            openInTabs={openInTabs}
            onNavigateToTab={handleNavigateToTab}
          />
        )}

        {view.type === 'chat' && selectedAgentId && (
          <ChatView
            ref={chatViewRef}
            agentId={selectedAgentId}
            conversationId={view.conversationId}
            onBack={handleBackToConversations}
            onNewChat={() => handleNewConversation()}
            onPopOut={handlePopOut}
            onOpenTrace={handleOpenTrace}
          />
        )}

        {view.type === 'trace' && (
          <div className="flex flex-col h-full">
            <div className="h-12 flex items-center gap-2 px-3 border-b border-border flex-shrink-0">
              <button
                onClick={() => setView({ type: 'chat', conversationId: view.conversationId })}
                className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              >
                <ArrowLeft className="w-4 h-4" />
              </button>
              <div className="flex items-center gap-2 flex-1 min-w-0">
                <div className="w-8 h-8 rounded-lg bg-blue-500/20 flex items-center justify-center flex-shrink-0">
                  <Activity className="w-4 h-4 text-blue-500" />
                </div>
                <div className="min-w-0">
                  <h2 className="text-sm font-semibold truncate">Trace</h2>
                  <p className="text-xs text-muted-foreground truncate">{view.conversationId.slice(0, 8)}...</p>
                </div>
              </div>
            </div>
            <TraceView conversationId={view.conversationId} />
          </div>
        )}
      </div>

      <ResizeHandle onMouseDown={startResizing} />
    </div>
  )
})

ChatPanel.displayName = 'ChatPanel'
