import { Plus, ListTodo } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { Agent } from '@/hooks/useAgents'
import { ReactorLogoMark } from '@/components/ReactorLogo'
import { AgentIcon } from './AgentIcon'

interface AgentBarProps {
  activeAgentIds: string[]
  selectedAgentId: string | null
  allAgents: Agent[]
  onSelectAgent: (agentId: string) => void
  onAddClick: () => void
  onTasksClick?: () => void
  tasksActive?: boolean
}

export function AgentBar({
  activeAgentIds,
  selectedAgentId,
  allAgents,
  onSelectAgent,
  onAddClick,
  onTasksClick,
  tasksActive,
}: AgentBarProps) {
  const activeAgents = allAgents.filter((a) => activeAgentIds.includes(a.id))

  return (
    <div className="w-14 flex flex-col items-center py-3 gap-2 bg-card/50 border-r border-border">
      <div className="relative group mt-[5px] mb-2">
        <ReactorLogoMark size={28} variant="blue" />
        <div className="absolute left-full ml-2 px-2 py-1 bg-popover border border-border rounded text-xs font-medium text-foreground whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity z-50 shadow-lg">
          Reactor
        </div>
      </div>

      {/* Tasks button */}
      <button
        onClick={onTasksClick}
        className={cn(
          'relative w-10 h-10 rounded-full flex items-center justify-center transition-all group mb-4',
          tasksActive
            ? 'bg-purple-500/20 ring-1 ring-purple-500/30'
            : 'bg-muted/50 hover:bg-muted'
        )}
        title="Tasks"
      >
        <ListTodo className={cn('w-5 h-5', tasksActive ? 'text-purple-500' : 'text-muted-foreground')} />
        <div className="absolute left-full ml-2 px-2 py-1 bg-popover border border-border rounded text-xs font-medium text-foreground whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity z-50 shadow-lg">
          Tasks
        </div>
      </button>

      <div className="w-8 border-t border-border mb-2" />

      {activeAgents.map((agent) => {
        const isSelected = selectedAgentId === agent.id && !tasksActive
        return (
          <button
            key={agent.id}
            onClick={() => onSelectAgent(agent.id)}
            className={cn(
              'relative w-10 h-10 rounded-full flex items-center justify-center transition-all group mb-[10px]',
              isSelected && 'ring-1 ring-border shadow-sm'
            )}
            style={{
              backgroundColor: isSelected ? `${agent.color}30` : `${agent.color}15`,
            }}
            title={agent.name}
          >
            <AgentIcon agent={agent} size="sm" />

            <div className="absolute left-full ml-2 px-2 py-1 bg-popover border border-border rounded text-xs font-medium text-foreground whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity z-50 shadow-lg">
              {agent.name}
            </div>
          </button>
        )
      })}

      <div className="flex-1" />

      <button
        onClick={onAddClick}
        className="w-10 h-10 rounded-full border-2 border-dashed border-border flex items-center justify-center text-muted-foreground hover:text-foreground hover:border-muted-foreground transition-colors"
        title="Add Agent"
      >
        <Plus className="w-4 h-4" />
      </button>
    </div>
  )
}
