import { useEffect, useState, useMemo } from 'react'
import { Plus, MessageSquare, Loader2, Trash2, PanelRight, Users, ChevronDown } from 'lucide-react'
import { ScrollArea } from '@/components/ui/ScrollArea'
import { ConfirmDialog } from '@/components/ui/ConfirmDialog'
import { useAgents } from '@/hooks/useAgents'
import { useConversations, type Conversation } from '@/hooks/useConversations'
import { cn } from '@/lib/utils'
import { AgentIcon } from './AgentIcon'

interface ConversationListProps {
  agentId: string
  activeConversationId?: string | null
  onSelectConversation: (conversationId: string) => void
  onNewConversation: () => void
  openInTabs?: string[]
  onNavigateToTab?: (conversationId: string) => void
}

export function ConversationList({
  agentId,
  activeConversationId,
  onSelectConversation,
  onNewConversation,
  openInTabs = [],
  onNavigateToTab,
}: ConversationListProps) {
  const { getAgent } = useAgents()
  const agent = getAgent(agentId)
  const { conversations, loading, refresh, deleteConversation } = useConversations({ agentId })
  const [deleteConfirm, setDeleteConfirm] = useState<{ id: string; title: string } | null>(null)
  const [showSubtasks, setShowSubtasks] = useState(true)

  const { mainConversations, subagentCount } = useMemo(() => {
    const main: Conversation[] = []
    let subCount = 0

    for (const conv of conversations) {
      const isSubagent = conv.id.includes('::sub::') || conv.parentConversationId
      if (isSubagent) {
        subCount++
        if (showSubtasks) {
          main.push(conv)
        }
      } else {
        main.push(conv)
      }
    }

    return { mainConversations: main, subagentCount: subCount }
  }, [conversations, showSubtasks])

  useEffect(() => {
    refresh()
  }, [agentId, refresh])

  if (!agent) return null

  const formatTime = (dateStr: string) => {
    const date = new Date(dateStr)
    const now = new Date()
    const diff = now.getTime() - date.getTime()
    const hours = Math.floor(diff / (1000 * 60 * 60))
    const days = Math.floor(hours / 24)

    if (days > 0) {
      return days === 1 ? 'Yesterday' : `${days}d ago`
    }
    if (hours > 0) {
      return `${hours}h ago`
    }
    return 'Just now'
  }

  const getLastMessage = (conv: Conversation) => {
    if (!conv.messages || conv.messages.length === 0) {
      return 'No messages yet'
    }
    const lastMsg = conv.messages[conv.messages.length - 1]
    return (lastMsg.content || '').slice(0, 100)
  }

  const handleDeleteClick = (e: React.MouseEvent, conv: Conversation) => {
    e.stopPropagation()
    setDeleteConfirm({ id: conv.id, title: conv.title || 'New Conversation' })
  }

  const handleConfirmDelete = async () => {
    if (deleteConfirm) {
      await deleteConversation(deleteConfirm.id)
      setDeleteConfirm(null)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
      <div className="p-4 border-b border-border flex-shrink-0">
        <div className="flex items-center gap-3 mb-4">
          <div
            className="w-9 h-9 rounded-full flex items-center justify-center"
            style={{ backgroundColor: `${agent.color}20` }}
          >
            <AgentIcon agent={agent} size="sm" />
          </div>
          <div>
            <h2 className="font-semibold text-foreground text-sm">{agent.name}</h2>
            <p className="text-xs text-muted-foreground line-clamp-1">{agent.description || `${agent.name} agent`}</p>
          </div>
        </div>

        <button
          onClick={onNewConversation}
          className="w-full flex items-center justify-center gap-2 px-4 py-2.5 bg-blue-500 text-white rounded-full hover:bg-blue-600 transition-colors font-medium text-sm"
        >
          <Plus className="w-4 h-4" />
          New Conversation
        </button>
      </div>

      {subagentCount > 0 && (
        <div className="px-3 py-2 border-b border-border flex-shrink-0">
          <button
            onClick={() => setShowSubtasks(!showSubtasks)}
            className="w-full flex items-center gap-2 px-2 py-1.5 text-xs text-muted-foreground hover:text-foreground hover:bg-accent rounded transition-colors"
          >
            <Users className="w-3 h-3" />
            <span>{showSubtasks ? 'Hide' : 'Show'} sub-tasks</span>
            <span className="text-muted-foreground/60 ml-auto">{subagentCount}</span>
            <ChevronDown
              className={cn(
                'w-3 h-3 transition-transform',
                !showSubtasks && '-rotate-90'
              )}
            />
          </button>
        </div>
      )}

      <ScrollArea className="flex-1 min-h-0">
        {loading ? (
          <div className="flex items-center justify-center p-8">
            <Loader2 className="w-5 h-5 animate-spin text-muted-foreground" />
          </div>
        ) : mainConversations.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-8 text-center">
            <div className="w-10 h-10 rounded-full bg-muted flex items-center justify-center mb-4">
              <MessageSquare className="w-5 h-5 text-muted-foreground" />
            </div>
            <p className="text-sm text-muted-foreground">
              No conversations yet.
              <br />
              Start one to get help from {agent.name}.
            </p>
          </div>
        ) : (
          <div className="p-2">
            {mainConversations.map((conv) => {
              const isOpenInTab = openInTabs.includes(conv.id)
              const isSubagent = conv.id.includes('::sub::') || conv.parentConversationId
              const handleClick = () => {
                if (isOpenInTab && onNavigateToTab) {
                  onNavigateToTab(conv.id)
                } else {
                  onSelectConversation(conv.id)
                }
              }
              return (
                <div
                  key={conv.id}
                  onClick={handleClick}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => e.key === 'Enter' && handleClick()}
                  className={cn(
                    'w-full flex flex-col gap-1.5 p-3 rounded-lg text-left transition-colors group relative overflow-hidden cursor-pointer',
                    'hover:bg-accent',
                    activeConversationId === conv.id && 'bg-accent',
                    isOpenInTab && 'border-l-2 border-primary bg-primary/5',
                    isSubagent && 'ml-4 border-l border-muted-foreground/20'
                  )}
                >
                  <div className="flex items-center gap-2">
                    {isSubagent && (
                      <span className="flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground shrink-0">
                        <Users className="w-2.5 h-2.5" />
                        subtask
                      </span>
                    )}
                    <span className="font-medium text-sm text-foreground truncate flex-1">
                      {conv.title || 'New Conversation'}
                    </span>
                    {isOpenInTab && (
                      <PanelRight className="w-3.5 h-3.5 text-primary flex-shrink-0" />
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground line-clamp-2">
                    {getLastMessage(conv)}
                  </p>
                  <span className="text-[10px] text-muted-foreground/70">
                    {formatTime(conv.updatedAt)}
                  </span>
                  <button
                    onClick={(e) => handleDeleteClick(e, conv)}
                    className="absolute right-2 bottom-2 p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all"
                    title="Delete conversation"
                  >
                    <Trash2 className="w-3 h-3" />
                  </button>
                </div>
              )
            })}
          </div>
        )}
      </ScrollArea>

      <ConfirmDialog
        open={!!deleteConfirm}
        onClose={() => setDeleteConfirm(null)}
        onConfirm={handleConfirmDelete}
        title="Delete conversation?"
        description={`"${deleteConfirm?.title}" will be permanently deleted. This action cannot be undone.`}
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="destructive"
      />
    </div>
  )
}
