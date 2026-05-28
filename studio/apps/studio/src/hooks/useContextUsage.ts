import { useState, useMemo, useCallback } from 'react'

const MODEL_CONTEXT_WINDOWS: Record<string, number> = {
  'claude-opus-4-5-20251101': 200000,
  'claude-sonnet-4-20250514': 200000,
  'claude-opus-4-20250514': 200000,
  'claude-3-5-sonnet-20241022': 200000,
  'claude-3-5-haiku-20241022': 200000,
  'claude-3-opus-20240229': 200000,
  'claude-3-sonnet-20240229': 200000,
  'claude-3-haiku-20240307': 200000,
  'anthropic/claude-opus-4-5-20251101': 200000,
  'anthropic/claude-sonnet-4-20250514': 200000,
  'anthropic/claude-opus-4-20250514': 200000,
  'anthropic/claude-3.5-sonnet': 200000,
  'anthropic/claude-3-opus': 200000,
  'gpt-4o': 128000,
  'gpt-4-turbo': 128000,
  'gpt-4': 8192,
  'gpt-3.5-turbo': 16385,
  'openai/gpt-4o': 128000,
  'openai/gpt-4-turbo': 128000,
  'openai/gpt-4': 8192,
  'openai/gpt-3.5-turbo': 16385,
  'google/gemini-pro-1.5': 1000000,
  'google/gemini-pro': 32768,
  'meta-llama/llama-3.1-405b-instruct': 131072,
  'meta-llama/llama-3.1-70b-instruct': 131072,
}

const DEFAULT_CONTEXT_WINDOW = 200000

function getContextWindow(model: string): number {
  if (MODEL_CONTEXT_WINDOWS[model]) {
    return MODEL_CONTEXT_WINDOWS[model]
  }

  const lowerModel = model.toLowerCase()
  if (lowerModel.includes('claude')) return 200000
  if (lowerModel.includes('gpt-4o') || lowerModel.includes('gpt-4-turbo')) return 128000
  if (lowerModel.includes('gpt-4')) return 8192
  if (lowerModel.includes('gemini-1.5') || lowerModel.includes('gemini-pro-1.5')) return 1000000
  if (lowerModel.includes('llama-3.1')) return 131072

  return DEFAULT_CONTEXT_WINDOW
}

interface UseContextUsageOptions {
  conversationId: string | null
  model: string
  messageCount?: number
}

export interface ContextUsage {
  usedTokens: number
  contextWindow: number
  percentage: number
  remainingTokens: number
  isNearLimit: boolean
  isAtLimit: boolean
  lastInputTokens: number
  lastOutputTokens: number
  loading: boolean
  refresh: () => Promise<void>
}

export function useContextUsage({
  model,
  messageCount = 0,
}: UseContextUsageOptions): ContextUsage {
  const [loading] = useState(false)

  const contextWindow = useMemo(() => getContextWindow(model), [model])

  const estimatedTokens = useMemo(() => {
    return messageCount * 500
  }, [messageCount])

  const refresh = useCallback(async () => {
  }, [])

  const percentage = contextWindow > 0 ? Math.min((estimatedTokens / contextWindow) * 100, 100) : 0
  const remainingTokens = Math.max(contextWindow - estimatedTokens, 0)

  return {
    usedTokens: estimatedTokens,
    contextWindow,
    percentage,
    remainingTokens,
    isNearLimit: percentage > 75,
    isAtLimit: percentage > 95,
    lastInputTokens: 0,
    lastOutputTokens: 0,
    loading,
    refresh,
  }
}

export function formatTokenCount(tokens: number): string {
  if (tokens < 1000) {
    return String(tokens)
  }
  if (tokens < 1000000) {
    return `${(tokens / 1000).toFixed(1)}k`
  }
  return `${(tokens / 1000000).toFixed(2)}M`
}
