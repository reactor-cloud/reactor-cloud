import { Settings, PanelRight, HelpCircle, FlaskConical } from 'lucide-react'
import { ThemeToggle } from '@/components/ui/ThemeToggle'
import { cn } from '@/lib/utils'

interface TitleBarProps {
  onSettingsClick: () => void
  onFoundryClick: () => void
  onDocsClick: () => void
  onFileBrowserToggle: () => void
  fileBrowserOpen: boolean
  workspaceName?: string
}

export function TitleBar({
  onSettingsClick,
  onFoundryClick,
  onDocsClick,
  onFileBrowserToggle,
  fileBrowserOpen,
  workspaceName,
}: TitleBarProps) {
  return (
    <div
      data-tauri-drag-region
      className="h-10 flex items-center justify-between px-4 bg-card border-b border-border relative select-none"
    >
      {/* Spacer for macOS traffic lights */}
      <div data-tauri-drag-region className="flex items-center gap-2 pl-[70px] min-w-0 flex-1" />

      {workspaceName && (
        <div
          data-tauri-drag-region
          className="absolute left-1/2 -translate-x-1/2 text-sm text-muted-foreground font-medium truncate max-w-[200px] pointer-events-none"
        >
          {workspaceName}
        </div>
      )}

      <div className="flex items-center gap-1">
        <ThemeToggle />

        <button
          onClick={onDocsClick}
          className="p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          title="Documentation"
        >
          <HelpCircle className="w-4 h-4" />
        </button>

        <button
          onClick={onFoundryClick}
          className="p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          title="Foundry"
        >
          <FlaskConical className="w-4 h-4" />
        </button>

        <button
          onClick={onSettingsClick}
          className="p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          title="Settings"
        >
          <Settings className="w-4 h-4" />
        </button>

        <button
          onClick={onFileBrowserToggle}
          className={cn(
            'p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors',
            fileBrowserOpen && 'bg-accent text-foreground'
          )}
          title="File Browser"
        >
          <PanelRight className="w-4 h-4" />
        </button>
      </div>
    </div>
  )
}
