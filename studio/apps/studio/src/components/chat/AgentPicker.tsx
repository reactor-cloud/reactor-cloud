import { useState } from 'react'
import { ArrowLeft, Search, Check, RefreshCw } from 'lucide-react'
import { ScrollArea } from '@/components/ui/ScrollArea'
import type { Agent } from '@/hooks/useAgents'
import { cn } from '@/lib/utils'
import { AgentIcon } from './AgentIcon'

interface AgentPickerProps {
  activeAgentIds: string[]
  allAgents: Agent[]
  isLoading?: boolean
  onSelect: (agentId: string) => void
  onBack: () => void
  onRefresh?: () => void
}

export function AgentPicker({
  activeAgentIds,
  allAgents,
  isLoading,
  onSelect,
  onBack,
  onRefresh,
}: AgentPickerProps) {
  const [search, setSearch] = useState('')

  const filteredAgents = allAgents.filter(
    (agent) =>
      agent.name.toLowerCase().includes(search.toLowerCase()) ||
      (agent.description || '').toLowerCase().includes(search.toLowerCase())
  )

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
      <div className="p-3 border-b border-border flex-shrink-0">
        <button
          onClick={onBack}
          className="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-colors mb-3"
        >
          <ArrowLeft className="w-4 h-4" />
          Back
        </button>

        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold text-foreground">Add Agent</h2>
          {onRefresh && (
            <button
              onClick={onRefresh}
              disabled={isLoading}
              className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
              title="Refresh agents"
            >
              <RefreshCw className={cn('w-4 h-4', isLoading && 'animate-spin')} />
            </button>
          )}
        </div>

        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search agents..."
            className="w-full pl-9 pr-4 py-2 bg-background border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>
      </div>

      <ScrollArea className="flex-1 min-h-0 p-3">
        <div className="grid gap-2">
          {filteredAgents.map((agent) => {
            const isActive = activeAgentIds.includes(agent.id)

            return (
              <button
                key={agent.id}
                onClick={() => onSelect(agent.id)}
                className={cn(
                  'flex items-center gap-3 p-2.5 rounded-lg text-left transition-colors',
                  'hover:bg-accent border border-transparent',
                  isActive && 'border-primary/20 bg-primary/5'
                )}
              >
                <div
                  className="w-9 h-9 rounded-full flex items-center justify-center flex-shrink-0"
                  style={{ backgroundColor: `${agent.color}20` }}
                >
                  <AgentIcon agent={agent} size="sm" />
                </div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-foreground text-sm">{agent.name}</span>
                    {isActive && (
                      <span className="flex items-center gap-1 text-xs text-primary">
                        <Check className="w-3 h-3" />
                        Active
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground line-clamp-1 mt-0.5">
                    {agent.description || `${agent.name} agent`}
                  </p>
                </div>
              </button>
            )
          })}
        </div>
      </ScrollArea>
    </div>
  )
}
