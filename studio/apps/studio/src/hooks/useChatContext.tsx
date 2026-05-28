import { createContext, useContext, useState, useCallback, ReactNode } from 'react'

export interface ContextReference {
  id: string
  path: string
  name: string
  type: 'file' | 'folder'
}

interface ChatContextValue {
  contextRefs: ContextReference[]
  addContextRef: (path: string, name: string, type?: 'file' | 'folder') => void
  removeContextRef: (id: string) => void
  clearContextRefs: () => void
}

const ChatContextContext = createContext<ChatContextValue | null>(null)

export function ChatContextProvider({ children }: { children: ReactNode }) {
  const [contextRefs, setContextRefs] = useState<ContextReference[]>([])

  const addContextRef = useCallback((path: string, name: string, type: 'file' | 'folder' = 'file') => {
    const id = `ref-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
    setContextRefs((prev) => {
      if (prev.some((ref) => ref.path === path)) {
        return prev
      }
      return [...prev, { id, path, name, type }]
    })
  }, [])

  const removeContextRef = useCallback((id: string) => {
    setContextRefs((prev) => prev.filter((ref) => ref.id !== id))
  }, [])

  const clearContextRefs = useCallback(() => {
    setContextRefs([])
  }, [])

  return (
    <ChatContextContext.Provider value={{ contextRefs, addContextRef, removeContextRef, clearContextRefs }}>
      {children}
    </ChatContextContext.Provider>
  )
}

export function useChatContext() {
  const context = useContext(ChatContextContext)
  if (!context) {
    throw new Error('useChatContext must be used within a ChatContextProvider')
  }
  return context
}
