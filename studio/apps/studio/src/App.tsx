import { createContext, useContext, useState, useCallback } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { TitleBar } from '@/components/layout/TitleBar'
import { ChatPanel } from '@/components/layout/ChatPanel'
import { MainPane } from '@/components/layout/MainPane'
import { FileBrowserPanel } from '@/components/layout/FileBrowserPanel'
import { ViewsProvider, initializeViews, useViews } from '@/views'
import { useTheme } from '@/hooks/useTheme'
import { ChatContextProvider, useChatContext } from '@/hooks/useChatContext'
import { FileClipboardProvider } from '@/hooks/useFileClipboard'

// Initialize views registry
initializeViews()

interface WorkspaceContextValue {
  workspacePath: string | null
  workspaceName: string | null
  projectId: string | null
}

const WorkspaceContext = createContext<WorkspaceContextValue>({
  workspacePath: null,
  workspaceName: null,
  projectId: null,
})

export const useWorkspaceContext = () => useContext(WorkspaceContext)

function WorkspaceScreen({ onOpen }: { onOpen: (path: string) => void }) {
  const handleOpenFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Open Project Folder',
      })
      if (selected && typeof selected === 'string') {
        onOpen(selected)
      }
    } catch (error) {
      console.error('Failed to open folder:', error)
    }
  }

  return (
    <div className="h-screen flex items-center justify-center bg-background">
      <div className="text-center">
        <h1 className="text-3xl font-bold text-foreground mb-2">Reactor Studio</h1>
        <p className="text-muted-foreground mb-8">Open a folder to get started</p>
        <button
          onClick={handleOpenFolder}
          className="px-6 py-3 bg-primary text-primary-foreground rounded-lg hover:bg-primary/90 transition-colors font-medium"
        >
          Open Folder
        </button>
      </div>
    </div>
  )
}

function AppContent({ workspace }: { workspace: WorkspaceContextValue }) {
  const [fileBrowserOpen, setFileBrowserOpen] = useState(true)
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>('coder')
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null)
  const { openFile, openView } = useViews()
  const { addContextRef } = useChatContext()

  const handleSettingsClick = useCallback(() => {
    openView({ viewId: 'settings' })
  }, [openView])

  const handleFoundryClick = useCallback(() => {
    openView({ viewId: 'foundry' })
  }, [openView])

  const handleDocsClick = useCallback(() => {
    console.log('Docs clicked')
  }, [])

  const handleFileBrowserToggle = useCallback(() => {
    setFileBrowserOpen((prev) => !prev)
  }, [])

  const handleAgentSelect = useCallback((agentId: string) => {
    setSelectedAgentId(agentId)
  }, [])

  const handleConversationSelect = useCallback((conversationId: string | null) => {
    setActiveConversationId(conversationId)
  }, [])

  const handleOpenFile = useCallback((filePath: string) => {
    openFile({ filePath })
  }, [openFile])

  const handleAddToChat = useCallback((path: string, name: string) => {
    const isFolder = !name.includes('.') || name.endsWith('/')
    addContextRef(path, name, isFolder ? 'folder' : 'file')
  }, [addContextRef])

  return (
    <div className="h-screen flex flex-col bg-background text-foreground overflow-hidden">
      <TitleBar
        onSettingsClick={handleSettingsClick}
        onFoundryClick={handleFoundryClick}
        onDocsClick={handleDocsClick}
        onFileBrowserToggle={handleFileBrowserToggle}
        fileBrowserOpen={fileBrowserOpen}
        workspaceName={workspace.workspaceName || undefined}
      />

      <div className="flex-1 flex overflow-hidden">
        <ChatPanel
          selectedAgentId={selectedAgentId}
          onAgentSelect={handleAgentSelect}
          activeConversationId={activeConversationId}
          onConversationSelect={handleConversationSelect}
        />

        <MainPane />

        {fileBrowserOpen && (
          <FileBrowserPanel
            workspacePath={workspace.workspacePath}
            onOpenFile={handleOpenFile}
            onAddToChat={handleAddToChat}
          />
        )}
      </div>
    </div>
  )
}

export default function App() {
  // Initialize theme hook at app root - handles localStorage persistence and system preference
  useTheme()

  const [workspace, setWorkspace] = useState<WorkspaceContextValue>({
    workspacePath: null,
    workspaceName: null,
    projectId: null,
  })

  const handleOpenWorkspace = useCallback(async (path: string) => {
    try {
      const result: { projectId: string; projectName: string; path: string } = await invoke(
        'workspace_open',
        { path }
      )
      setWorkspace({
        workspacePath: result.path,
        workspaceName: result.projectName,
        projectId: result.projectId,
      })
    } catch (error) {
      console.error('Failed to open workspace:', error)
      alert(`Failed to open workspace: ${error}`)
    }
  }, [])

  if (!workspace.workspacePath) {
    return <WorkspaceScreen onOpen={handleOpenWorkspace} />
  }

  return (
    <WorkspaceContext.Provider value={workspace}>
      <ViewsProvider>
        <ChatContextProvider>
          <FileClipboardProvider>
            <AppContent workspace={workspace} />
          </FileClipboardProvider>
        </ChatContextProvider>
      </ViewsProvider>
    </WorkspaceContext.Provider>
  )
}
