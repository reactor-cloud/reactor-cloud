import { useState, useEffect, useCallback } from 'react'
import { Store } from '@tauri-apps/plugin-store'

interface WindowState {
  workspacePath?: string
  selectedAgentId?: string
  activeConversationId?: string
  activeTaskId?: string
  fileBrowserOpen?: boolean
}

let storePromise: Promise<Store> | null = null

async function getStore(): Promise<Store> {
  if (!storePromise) {
    storePromise = Store.load('window-state.json')
  }
  return storePromise
}

export function useWindowState() {
  const [state, setState] = useState<WindowState>({})
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    loadState()
  }, [])

  const loadState = async () => {
    try {
      const s = await getStore()
      const savedState = await s.get<WindowState>('state')
      if (savedState) {
        setState(savedState)
      }
      setLoaded(true)
    } catch (error) {
      console.error('Failed to load window state:', error)
      setLoaded(true)
    }
  }

  const updateState = useCallback(async (updates: Partial<WindowState>) => {
    const newState = { ...state, ...updates }
    setState(newState)
    try {
      const s = await getStore()
      await s.set('state', newState)
      await s.save()
    } catch (error) {
      console.error('Failed to save window state:', error)
    }
  }, [state])

  return {
    state,
    loaded,
    updateState,
  }
}
