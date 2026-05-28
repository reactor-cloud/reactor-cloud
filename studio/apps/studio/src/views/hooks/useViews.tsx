import { useState, useCallback, useRef, createContext, useContext } from 'react'
import type { Tab, TabsState, ViewRef, OpenFileOptions, OpenViewOptions } from '../types'
import { getView, getDefaultViewForFile, getFileInfoFromPath } from '../registry'

function generateTabId(): string {
  return `tab-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
}

interface ConversationTab {
  tabId: string
  conversationId: string
}

interface ViewsContextValue {
  tabs: Tab[]
  activeTabId: string | null
  activeTab: Tab | null
  viewRefs: Map<string, ViewRef>
  openFile: (options: OpenFileOptions) => string | null
  openView: (options: OpenViewOptions) => string | null
  closeTab: (tabId: string) => Promise<boolean>
  closeOtherTabs: (tabId: string) => Promise<boolean>
  closeAllTabs: () => Promise<boolean>
  switchTab: (tabId: string) => void
  reorderTabs: (newTabs: Tab[]) => void
  saveTab: (tabId: string) => Promise<boolean>
  saveAllTabs: () => Promise<boolean>
  setTabDirty: (tabId: string, dirty: boolean) => void
  setTabTitle: (tabId: string, title: string) => void
  updateTabPath: (tabId: string, newPath: string) => void
  registerViewRef: (tabId: string, ref: ViewRef) => void
  unregisterViewRef: (tabId: string) => void
  getTabsState: () => TabsState
  restoreTabsState: (state: TabsState) => void
  getOpenConversationTabs: () => ConversationTab[]
  switchToConversationTab: (conversationId: string) => boolean
}

const ViewsContext = createContext<ViewsContextValue | null>(null)

export function useViewsProvider() {
  const [tabs, setTabs] = useState<Tab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)
  const viewRefs = useRef<Map<string, ViewRef>>(new Map())
  
  const activeTab = tabs.find(t => t.id === activeTabId) ?? null
  
  const openFile = useCallback((options: OpenFileOptions): string | null => {
    const { filePath, viewId, focus = true } = options
    
    const fileInfo = getFileInfoFromPath(filePath)
    const viewConfig = viewId ? getView(viewId) : getDefaultViewForFile(fileInfo)
    
    if (!viewConfig) {
      // Fall back to markdown view for text files
      const fallbackView = getView('markdown')
      if (!fallbackView) {
        console.warn(`No view found for file: ${filePath}`)
        return null
      }
    }
    
    const effectiveViewId = viewConfig?.id || 'markdown'
    
    const existing = tabs.find(t => t.filePath === filePath && t.viewId === effectiveViewId)
    if (existing) {
      if (focus) {
        setActiveTabId(existing.id)
      }
      return existing.id
    }
    
    const tabId = generateTabId()
    const newTab: Tab = {
      id: tabId,
      viewId: effectiveViewId,
      filePath,
      title: fileInfo.name,
      isDirty: false,
      lastActiveAt: Date.now(),
    }
    
    setTabs(prev => [...prev, newTab])
    if (focus) {
      setActiveTabId(tabId)
    }
    
    return tabId
  }, [tabs])
  
  const openView = useCallback((options: OpenViewOptions): string | null => {
    const { viewId, title, documentId, focus = true } = options
    
    if (documentId) {
      const existing = tabs.find(t => t.documentId === documentId)
      if (existing) {
        if (focus) {
          setActiveTabId(existing.id)
        }
        return existing.id
      }
    } else {
      const existing = tabs.find(t => t.viewId === viewId && !t.documentId && !t.filePath)
      if (existing) {
        if (focus) {
          setActiveTabId(existing.id)
        }
        return existing.id
      }
    }
    
    const viewConfig = getView(viewId)
    if (!viewConfig) {
      console.warn(`View not found: ${viewId}`)
      return null
    }
    
    const tabId = generateTabId()
    const newTab: Tab = {
      id: tabId,
      viewId,
      documentId,
      title: title || viewConfig.name,
      isDirty: false,
      lastActiveAt: Date.now(),
    }
    
    setTabs(prev => [...prev, newTab])
    if (focus) {
      setActiveTabId(tabId)
    }
    
    return tabId
  }, [tabs])
  
  const closeTab = useCallback(async (tabId: string): Promise<boolean> => {
    const tab = tabs.find(t => t.id === tabId)
    if (!tab) return true
    
    const ref = viewRefs.current.get(tabId)
    if (ref && tab.isDirty) {
      const canClose = await ref.canClose()
      if (!canClose) {
        return false
      }
    }
    
    setTabs(prev => {
      const index = prev.findIndex(t => t.id === tabId)
      const newTabs = prev.filter(t => t.id !== tabId)
      
      if (activeTabId === tabId && newTabs.length > 0) {
        const nextIndex = Math.min(index, newTabs.length - 1)
        setActiveTabId(newTabs[nextIndex].id)
      } else if (newTabs.length === 0) {
        setActiveTabId(null)
      }
      
      return newTabs
    })
    
    viewRefs.current.delete(tabId)
    return true
  }, [tabs, activeTabId])
  
  const closeOtherTabs = useCallback(async (tabId: string): Promise<boolean> => {
    const otherTabs = tabs.filter(t => t.id !== tabId)
    for (const tab of otherTabs) {
      const success = await closeTab(tab.id)
      if (!success) return false
    }
    return true
  }, [tabs, closeTab])
  
  const closeAllTabs = useCallback(async (): Promise<boolean> => {
    for (const tab of tabs) {
      const success = await closeTab(tab.id)
      if (!success) return false
    }
    return true
  }, [tabs, closeTab])
  
  const switchTab = useCallback((tabId: string) => {
    const tab = tabs.find(t => t.id === tabId)
    if (tab) {
      setTabs(prev => prev.map(t => 
        t.id === tabId ? { ...t, lastActiveAt: Date.now() } : t
      ))
      setActiveTabId(tabId)
    }
  }, [tabs])
  
  const reorderTabs = useCallback((newTabs: Tab[]) => {
    setTabs(newTabs)
  }, [])
  
  const saveTab = useCallback(async (tabId: string): Promise<boolean> => {
    const ref = viewRefs.current.get(tabId)
    if (!ref) return false
    
    const success = await ref.save()
    if (success) {
      setTabs(prev => prev.map(t => 
        t.id === tabId ? { ...t, isDirty: false } : t
      ))
    }
    return success
  }, [])
  
  const saveAllTabs = useCallback(async (): Promise<boolean> => {
    const dirtyTabs = tabs.filter(t => t.isDirty)
    for (const tab of dirtyTabs) {
      const success = await saveTab(tab.id)
      if (!success) return false
    }
    return true
  }, [tabs, saveTab])
  
  const setTabDirty = useCallback((tabId: string, dirty: boolean) => {
    setTabs(prev => prev.map(t => 
      t.id === tabId ? { ...t, isDirty: dirty } : t
    ))
  }, [])
  
  const setTabTitle = useCallback((tabId: string, title: string) => {
    setTabs(prev => prev.map(t => 
      t.id === tabId ? { ...t, title } : t
    ))
  }, [])

  const updateTabPath = useCallback((tabId: string, newPath: string) => {
    const newTitle = getFileInfoFromPath(newPath).name
    setTabs(prev => prev.map(t => 
      t.id === tabId ? { ...t, filePath: newPath, title: newTitle } : t
    ))
  }, [])
  
  const registerViewRef = useCallback((tabId: string, ref: ViewRef) => {
    viewRefs.current.set(tabId, ref)
  }, [])
  
  const unregisterViewRef = useCallback((tabId: string) => {
    viewRefs.current.delete(tabId)
  }, [])
  
  const getTabsState = useCallback((): TabsState => {
    return { tabs, activeTabId }
  }, [tabs, activeTabId])
  
  const restoreTabsState = useCallback((state: TabsState) => {
    setTabs(state.tabs)
    setActiveTabId(state.activeTabId)
  }, [])

  const getOpenConversationTabs = useCallback((): ConversationTab[] => {
    return tabs
      .filter(t => t.viewId === 'chat' && t.documentId)
      .map(t => ({ tabId: t.id, conversationId: t.documentId! }))
  }, [tabs])

  const switchToConversationTab = useCallback((conversationId: string): boolean => {
    const tab = tabs.find(t => t.viewId === 'chat' && t.documentId === conversationId)
    if (tab) {
      switchTab(tab.id)
      return true
    }
    return false
  }, [tabs, switchTab])
  
  return {
    tabs,
    activeTabId,
    activeTab,
    viewRefs: viewRefs.current,
    openFile,
    openView,
    closeTab,
    closeOtherTabs,
    closeAllTabs,
    switchTab,
    reorderTabs,
    saveTab,
    saveAllTabs,
    setTabDirty,
    setTabTitle,
    updateTabPath,
    registerViewRef,
    unregisterViewRef,
    getTabsState,
    restoreTabsState,
    getOpenConversationTabs,
    switchToConversationTab,
  }
}

export function ViewsProvider({ children }: { children: React.ReactNode }) {
  const value = useViewsProvider()
  
  return (
    <ViewsContext.Provider value={value}>
      {children}
    </ViewsContext.Provider>
  )
}

export function useViews(): ViewsContextValue {
  const context = useContext(ViewsContext)
  if (!context) {
    throw new Error('useViews must be used within a ViewsProvider')
  }
  return context
}
