import { FileText, FolderOpen } from 'lucide-react'
import { cn } from '@/lib/utils'

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

interface EmptyStateProps {
  onNewFile?: () => void
  onOpenFile?: () => void
}

export function EmptyState({ onNewFile, onOpenFile }: EmptyStateProps) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center bg-background p-8">
      <div className="max-w-md w-full space-y-8">
        <div className="text-center space-y-2">
          <h1 className="text-2xl font-semibold text-foreground">
            Welcome to Reactor Studio
          </h1>
          <p className="text-muted-foreground">
            Open a file from the browser or create a new one
          </p>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <ActionCard
            icon={FileText}
            title="New File"
            description="Create a new document"
            onClick={onNewFile}
            iconColor="text-blue-500"
          />
          
          <ActionCard
            icon={FolderOpen}
            title="Open File"
            description="From workspace"
            onClick={onOpenFile}
            iconColor="text-amber-500"
          />
        </div>

        <div className="pt-4 space-y-3 text-center">
          <p className="text-xs text-muted-foreground/70">
            Double-click a file in the file browser to open it
          </p>
        </div>
      </div>
    </div>
  )
}
