import { useCallback, useImperativeHandle } from 'react'
import { FileText, FolderOpen, Globe } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'
import { cn } from '@/lib/utils'
import { useViews } from '../hooks/useViews'
import type { ViewProps, ViewStatus } from '../types'

interface ActionCardProps {
  icon: React.ElementType
  title: string
  description: string
  onClick?: () => void
  iconColor?: string
}

function ActionCard({ icon: Icon, title, description, onClick, iconColor }: ActionCardProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'flex flex-col items-center gap-3 p-6 rounded-xl',
        'bg-card border border-border',
        'hover:bg-accent/50 hover:border-primary/30 transition-all duration-200',
        'text-left w-full'
      )}
    >
      <div className={cn(
        'w-12 h-12 rounded-xl flex items-center justify-center',
        'bg-muted'
      )}>
        <Icon className={cn('w-6 h-6', iconColor || 'text-muted-foreground')} />
      </div>
      <div className="text-center space-y-1">
        <h3 className="font-medium text-sm text-foreground">{title}</h3>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
    </button>
  )
}

export function NewTabView({ tabId, viewRef }: ViewProps) {
  const { openView, openFile, closeTab } = useViews()

  useImperativeHandle(viewRef, () => ({
    save: async () => true,
    canClose: async () => true,
    focus: () => {},
    getStatus: () => 'idle' as ViewStatus,
    isDirty: () => false,
    getContent: () => null,
    setContent: () => {},
  }))

  const handleAction = useCallback(async (viewId: string, title: string) => {
    if (tabId) {
      await closeTab(tabId)
    }
    openView({
      viewId,
      title,
      documentId: `${viewId}-${Date.now()}`,
    })
  }, [tabId, closeTab, openView])

  const handleNewMarkdown = useCallback(() => {
    handleAction('markdown', 'Untitled.md')
  }, [handleAction])

  const handleOpenBrowser = useCallback(() => {
    handleAction('browser', 'Browser')
  }, [handleAction])

  const handleOpenFile = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        title: 'Open File',
      })
      if (selected && typeof selected === 'string') {
        if (tabId) {
          closeTab(tabId)
        }
        openFile({ filePath: selected })
      }
    } catch (err) {
      console.error('Failed to open file:', err)
    }
  }, [tabId, closeTab, openFile])

  return (
    <div className="flex-1 flex flex-col items-center justify-center bg-background p-8">
      <div className="max-w-lg w-full space-y-8">
        <div className="text-center space-y-2">
          <h1 className="text-2xl font-semibold text-foreground">
            What would you like to do?
          </h1>
          <p className="text-muted-foreground">
            Create a new file or open an existing one
          </p>
        </div>

        <div className="grid grid-cols-3 gap-4">
          <ActionCard
            icon={FileText}
            title="Markdown"
            description="New document"
            onClick={handleNewMarkdown}
            iconColor="text-blue-500"
          />
          
          <ActionCard
            icon={Globe}
            title="Browser"
            description="Browse the web"
            onClick={handleOpenBrowser}
            iconColor="text-purple-500"
          />
          
          <ActionCard
            icon={FolderOpen}
            title="Open File"
            description="From workspace"
            onClick={handleOpenFile}
            iconColor="text-amber-500"
          />
        </div>

        <div className="pt-4 text-center">
          <p className="text-xs text-muted-foreground/70">
            Or double-click a file in the file browser
          </p>
        </div>
      </div>
    </div>
  )
}
