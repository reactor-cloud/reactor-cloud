import { useState, useCallback, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'

export interface TokenUsage {
  inputTokens: number
  outputTokens: number
  cacheCreationTokens?: number
  cacheReadTokens?: number
}

export interface ModelPricing {
  modelId: string
  inputPerMTok: number
  outputPerMTok: number
  cacheWritePerMTok?: number
  cacheReadPerMTok?: number
  fetchedAt: number
}

export interface StepCost {
  inputCost: number
  outputCost: number
  totalCost: number
  pricing: ModelPricing
}

export interface TraceStep {
  id: string
  type: 'user_message' | 'llm_request' | 'llm_response' | 'tool_call' | 'tool_result' | 'subagent_call' | 'error'
  timestamp: number
  duration?: number

  input?: unknown
  output?: unknown

  model?: string
  tokenUsage?: TokenUsage
  cost?: StepCost

  toolName?: string
  toolArgs?: Record<string, unknown>
  toolResult?: unknown

  childConversationId?: string
  subagentType?: string
  subagentDescription?: string

  status: 'pending' | 'running' | 'success' | 'error'
  error?: string
}

export interface TraceMetrics {
  totalDuration: number
  llmCalls: number
  toolCalls: number
  totalInputTokens: number
  totalOutputTokens: number
  estimatedCost: number
}

export interface ConversationTrace {
  conversationId: string
  agentId: string
  parentConversationId?: string
  subagentMeta?: {
    subagentType: string
    description: string
    depth: number
  }
  steps: TraceStep[]
  metrics: TraceMetrics
  createdAt: number
  updatedAt: number
}

export interface TraceSummary {
  conversationId: string
  agentId: string
  stepCount: number
  metrics: TraceMetrics
  createdAt: number
  updatedAt: number
}

interface UseTraceOptions {
  conversationId: string | null
  autoRefresh?: boolean
  refreshInterval?: number
}

interface TraceGetResult {
  success: boolean
  trace: ConversationTrace | null
  error?: string
}

export function useTrace({ conversationId, autoRefresh = false, refreshInterval = 2000 }: UseTraceOptions) {
  const [trace, setTrace] = useState<ConversationTrace | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const unlistenRef = useRef<UnlistenFn | null>(null)

  const refresh = useCallback(async () => {
    if (!conversationId) {
      setTrace(null)
      return
    }

    setLoading(true)
    try {
      const result = await invoke<TraceGetResult>('trace_get', { conversationId })
      if (result.success && result.trace) {
        setTrace(result.trace)
        setError(null)
      } else {
        setTrace(null)
        setError(result.error || 'No trace found')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      setTrace(null)
    } finally {
      setLoading(false)
    }
  }, [conversationId])

  // Initial fetch and auto-refresh
  useEffect(() => {
    refresh()

    let interval: ReturnType<typeof setInterval> | null = null
    if (autoRefresh && conversationId) {
      interval = setInterval(refresh, refreshInterval)
    }

    return () => {
      if (interval) {
        clearInterval(interval)
      }
    }
  }, [conversationId, autoRefresh, refreshInterval, refresh])

  // Listen for live trace step updates
  useEffect(() => {
    if (!conversationId) return

    let mounted = true

    const setupListener = async () => {
      try {
        unlistenRef.current = await listen<{ conversationId: string; step: TraceStep }>(
          'trace:step',
          (event) => {
            if (!mounted) return
            if (event.payload.conversationId === conversationId) {
              setTrace((prevTrace) => {
                if (!prevTrace) return prevTrace
                return {
                  ...prevTrace,
                  steps: [...prevTrace.steps, event.payload.step],
                  updatedAt: Date.now(),
                }
              })
            }
          }
        )
      } catch {
        // Event listening may not be available
      }
    }

    setupListener()

    return () => {
      mounted = false
      if (unlistenRef.current) {
        unlistenRef.current()
        unlistenRef.current = null
      }
    }
  }, [conversationId])

  return {
    trace,
    steps: trace?.steps || [],
    metrics: trace?.metrics || null,
    loading,
    error,
    refresh,
  }
}

export function formatDuration(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`
  }
  if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)}s`
  }
  const mins = Math.floor(ms / 60000)
  const secs = Math.floor((ms % 60000) / 1000)
  return `${mins}m ${secs}s`
}

export function formatCost(cost: number): string {
  if (cost < 0.01) {
    return `$${cost.toFixed(4)}`
  }
  if (cost < 1) {
    return `$${cost.toFixed(3)}`
  }
  return `$${cost.toFixed(2)}`
}

export function formatTokens(tokens: number): string {
  if (tokens < 1000) {
    return String(tokens)
  }
  if (tokens < 1000000) {
    return `${(tokens / 1000).toFixed(1)}k`
  }
  return `${(tokens / 1000000).toFixed(2)}M`
}
