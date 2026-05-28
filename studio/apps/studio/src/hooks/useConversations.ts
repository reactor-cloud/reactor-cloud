import { useState, useEffect, useCallback } from 'react'
import {
  conversationList,
  conversationCreate,
  conversationDelete,
  type ConversationInfo,
} from '@/lib/ipc'

export interface Conversation {
  id: string
  agentId: string
  title: string
  messages: ConversationMessage[]
  parentConversationId?: string
  subagentMeta?: {
    subagentType: string
    description: string
    depth: number
  }
  createdAt: string
  updatedAt: string
  messageCount: number
}

interface ConversationMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date | string
}

interface UseConversationsOptions {
  agentId?: string
}

function infoToConversation(info: ConversationInfo): Conversation {
  return {
    id: info.id,
    agentId: info.agentId,
    title: info.title,
    messages: [],
    createdAt: info.created,
    updatedAt: info.updated,
    messageCount: info.messageCount,
  }
}

export function useConversations({ agentId }: UseConversationsOptions = {}) {
  const [conversations, setConversations] = useState<Conversation[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    if (!agentId) {
      setConversations([])
      setLoading(false)
      return
    }

    try {
      setLoading(true)
      const convs = await conversationList(agentId)
      setConversations(convs.map(infoToConversation))
      setError(null)
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setLoading(false)
    }
  }, [agentId])

  useEffect(() => {
    refresh()
  }, [refresh])

  const create = useCallback(async (title?: string): Promise<string | null> => {
    if (!agentId) return null

    try {
      const conversationId = await conversationCreate(agentId, title)
      await refresh()
      return conversationId
    } catch (err) {
      setError((err as Error).message)
      return null
    }
  }, [agentId, refresh])

  const deleteConversation = useCallback(async (conversationId: string): Promise<boolean> => {
    try {
      await conversationDelete(conversationId)
      setConversations((prev) => prev.filter((c) => c.id !== conversationId))
      return true
    } catch {
      return false
    }
  }, [])

  return {
    conversations,
    loading,
    error,
    refresh,
    create,
    deleteConversation,
  }
}
