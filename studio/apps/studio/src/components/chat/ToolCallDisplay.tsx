import { useState } from 'react'
import { cn } from '@/lib/utils'
import { ChevronDown, ChevronRight, Terminal, FileText, Search, CheckCircle, XCircle, Loader2 } from 'lucide-react'
import type { Message } from '@/hooks/useChat'

type ToolCall = NonNullable<Message['toolCalls']>[number]

interface ToolCallDisplayProps {
  toolCalls: ToolCall[]
  isStreaming?: boolean
  className?: string
}

const toolIcons: Record<string, React.ComponentType<{ className?: string }>> = {
  bash: Terminal,
  shell: Terminal,
  file_read: FileText,
  file_write: FileText,
  file_edit: FileText,
  read_file: FileText,
  write_file: FileText,
  edit_file: FileText,
  grep: Search,
  glob: Search,
  search: Search,
}

function getStatusIcon(status?: string, isStreaming?: boolean) {
  if (isStreaming && (!status || status === 'running' || status === 'pending')) {
    return <Loader2 className="w-4 h-4 text-blue-400 animate-spin" />
  }
  if (status === 'error') {
    return <XCircle className="w-4 h-4 text-red-400" />
  }
  if (status === 'complete') {
    return <CheckCircle className="w-4 h-4 text-green-400" />
  }
  return <Loader2 className="w-4 h-4 text-blue-400 animate-spin" />
}

function SingleToolCall({ toolCall, isStreaming }: { toolCall: ToolCall; isStreaming?: boolean }) {
  const [expanded, setExpanded] = useState(false)
  const Icon = toolIcons[toolCall.name] || Terminal

  return (
    <div className="rounded-lg border border-border overflow-hidden bg-card">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-accent transition-colors"
      >
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="w-4 h-4 text-muted-foreground" />
        )}

        <Icon className="w-4 h-4 text-muted-foreground" />

        <span className="flex-1 text-sm font-medium text-foreground">
          {toolCall.name}
        </span>

        {getStatusIcon(toolCall.status, isStreaming)}
      </button>

      {expanded && (
        <div className="border-t border-border p-3 space-y-3">
          <div>
            <div className="text-xs font-medium text-muted-foreground mb-1">Arguments</div>
            <pre className="text-xs bg-muted rounded p-2 overflow-x-auto text-foreground">
              {JSON.stringify(toolCall.arguments, null, 2)}
            </pre>
          </div>

          {toolCall.status === 'complete' && (
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Completed</div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

export function ToolCallDisplay({ toolCalls, isStreaming, className }: ToolCallDisplayProps) {
  if (!toolCalls || toolCalls.length === 0) return null

  return (
    <div className={cn('space-y-2', className)}>
      {toolCalls.map((toolCall) => (
        <SingleToolCall
          key={toolCall.id}
          toolCall={toolCall}
          isStreaming={isStreaming && (toolCall.status === 'running' || toolCall.status === 'pending')}
        />
      ))}
    </div>
  )
}
