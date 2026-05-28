import { useState, useCallback, useEffect } from 'react'
import { agentList, type AgentSummary } from '@/lib/ipc'

export interface Agent {
  id: string
  name: string
  description?: string
  avatar?: string
  color: string
  type?: string
  systemPrompt?: string
  model: string
}

function agentSummaryToAgent(summary: AgentSummary): Agent {
  return {
    id: summary.id,
    name: summary.name,
    color: summary.color,
    avatar: summary.icon,
    type: 'assistant',
    model: summary.model,
  }
}

const DEFAULT_AGENTS: Agent[] = [
  { id: 'coder', name: 'Coder', color: '#3B82F6', type: 'coder', model: 'anthropic/claude-sonnet-4.5' },
  { id: 'planner', name: 'Planner', color: '#8B5CF6', type: 'planner', model: 'anthropic/claude-sonnet-4.5' },
  { id: 'researcher', name: 'Researcher', color: '#10B981', type: 'researcher', model: 'anthropic/claude-sonnet-4.5' },
]

export function useAgents() {
  const [allAgents, setAllAgents] = useState<Agent[]>(DEFAULT_AGENTS)
  const [activeAgentIds, setActiveAgentIds] = useState<string[]>(['coder'])
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>('coder')
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    async function fetchAgents() {
      try {
        const agentDataList = await agentList()
        if (agentDataList && agentDataList.length > 0) {
          const agents = agentDataList.map(agentSummaryToAgent)
          setAllAgents(agents)
        }
      } catch (err) {
        console.error('Failed to fetch agents:', err)
        setError((err as Error).message)
      } finally {
        setIsLoading(false)
      }
    }

    fetchAgents()
  }, [])

  const activeAgents = allAgents.filter((a) => activeAgentIds.includes(a.id))

  const addAgent = useCallback((agentId: string) => {
    setActiveAgentIds((prev) =>
      prev.includes(agentId) ? prev : [...prev, agentId]
    )
    setSelectedAgentId(agentId)
  }, [])

  const removeAgent = useCallback((agentId: string) => {
    setActiveAgentIds((prev) => prev.filter((id) => id !== agentId))
    if (selectedAgentId === agentId) {
      setSelectedAgentId(activeAgentIds[0] !== agentId ? activeAgentIds[0] : null)
    }
  }, [selectedAgentId, activeAgentIds])

  const selectAgent = useCallback((agentId: string) => {
    setSelectedAgentId(agentId)
  }, [])

  const getAgent = useCallback((agentId: string): Agent | undefined => {
    return allAgents.find((a) => a.id === agentId)
  }, [allAgents])

  const refreshAgents = useCallback(async () => {
    setIsLoading(true)
    try {
      const agentDataList = await agentList()
      if (agentDataList && agentDataList.length > 0) {
        const agents = agentDataList.map(agentSummaryToAgent)
        setAllAgents(agents)
      }
    } catch (err) {
      console.error('Failed to refresh agents:', err)
      setError((err as Error).message)
    } finally {
      setIsLoading(false)
    }
  }, [])

  return {
    allAgents,
    activeAgents,
    activeAgentIds,
    selectedAgentId,
    isLoading,
    error,
    addAgent,
    removeAgent,
    selectAgent,
    getAgent,
    refreshAgents,
    setSelectedAgentId,
    setActiveAgentIds,
  }
}
