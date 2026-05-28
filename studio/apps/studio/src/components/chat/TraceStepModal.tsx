import { useState } from 'react'
import {
  X,
  Copy,
  Check,
  Clock,
  Hash,
  Coins,
  ChevronDown,
  ChevronRight,
  Cpu,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { formatDuration, formatCost, formatTokens, type TraceStep } from '@/hooks/useTrace'

interface TraceStepModalProps {
  step: TraceStep
  onClose: () => void
}

function CopyButton({ text, className }: { text: string; className?: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <button
      onClick={handleCopy}
      className={cn(
        'p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors',
        className
      )}
      title="Copy to clipboard"
    >
      {copied ? (
        <Check className="w-4 h-4 text-emerald-500" />
      ) : (
        <Copy className="w-4 h-4" />
      )}
    </button>
  )
}

function JsonViewer({ data, label }: { data: unknown; label: string }) {
  const [expanded, setExpanded] = useState(true)
  const jsonStr = JSON.stringify(data, null, 2)

  return (
    <div className="border border-border rounded-lg overflow-hidden">
      <div
        className="flex items-center justify-between px-3 py-2 bg-muted/50 cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="w-4 h-4 text-muted-foreground" />
          )}
          <span className="text-sm font-medium text-foreground">{label}</span>
        </div>
        <CopyButton text={jsonStr} />
      </div>
      
      {expanded && (
        <pre className="p-3 text-xs font-mono bg-card overflow-x-auto max-h-64 overflow-y-auto">
          <code className="text-foreground">{jsonStr}</code>
        </pre>
      )}
    </div>
  )
}

function TextViewer({ text, label }: { text: string; label: string }) {
  const [expanded, setExpanded] = useState(true)

  return (
    <div className="border border-border rounded-lg overflow-hidden">
      <div
        className="flex items-center justify-between px-3 py-2 bg-muted/50 cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="w-4 h-4 text-muted-foreground" />
          )}
          <span className="text-sm font-medium text-foreground">{label}</span>
          <span className="text-xs text-muted-foreground">({text.length} chars)</span>
        </div>
        <CopyButton text={text} />
      </div>
      
      {expanded && (
        <div 
          className="p-3 text-xs bg-card overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap break-words font-mono"
          style={{ unicodeBidi: 'plaintext' }}
        >
          {text}
        </div>
      )}
    </div>
  )
}

export function TraceStepModal({ step, onClose }: TraceStepModalProps) {
  const typeLabels: Record<string, string> = {
    user_message: 'User Message',
    llm_request: 'LLM Request',
    llm_response: 'LLM Response',
    tool_call: 'Tool Call',
    tool_result: 'Tool Result',
    subagent_call: 'Subagent Call',
    error: 'Error',
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-card border border-border rounded-xl shadow-2xl w-full max-w-2xl max-h-[85vh] flex flex-col mx-4">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <div>
            <h2 className="text-lg font-semibold text-foreground">
              {typeLabels[step.type] || step.type}
            </h2>
            {step.toolName && (
              <p className="text-sm text-muted-foreground">{step.toolName}</p>
            )}
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-lg hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          <div className="flex flex-wrap gap-3">
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-muted text-sm">
              <Clock className="w-3.5 h-3.5 text-muted-foreground" />
              <span className="text-muted-foreground">Time:</span>
              <span className="font-medium text-foreground">
                {new Date(step.timestamp).toLocaleTimeString()}
              </span>
            </div>

            {step.duration !== undefined && (
              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-muted text-sm">
                <Clock className="w-3.5 h-3.5 text-muted-foreground" />
                <span className="text-muted-foreground">Duration:</span>
                <span className="font-medium text-foreground">{formatDuration(step.duration)}</span>
              </div>
            )}

            {step.model && (
              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-muted text-sm">
                <Cpu className="w-3.5 h-3.5 text-muted-foreground" />
                <span className="font-medium text-foreground">{step.model}</span>
              </div>
            )}

            {step.tokenUsage && (
              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-muted text-sm">
                <Hash className="w-3.5 h-3.5 text-muted-foreground" />
                <span className="text-muted-foreground">Tokens:</span>
                <span className="font-medium text-foreground">
                  {formatTokens(step.tokenUsage.inputTokens)} in / {formatTokens(step.tokenUsage.outputTokens)} out
                </span>
              </div>
            )}

            {step.cost && (
              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-emerald-100 dark:bg-emerald-900/30 text-sm">
                <Coins className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />
                <span className="text-emerald-700 dark:text-emerald-300">Cost:</span>
                <span className="font-medium text-emerald-700 dark:text-emerald-300">
                  {formatCost(step.cost.totalCost)}
                </span>
              </div>
            )}

            <div className={cn(
              'flex items-center gap-1.5 px-2.5 py-1 rounded-md text-sm',
              step.status === 'success' && 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300',
              step.status === 'error' && 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
              step.status === 'running' && 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
              step.status === 'pending' && 'bg-muted text-muted-foreground'
            )}>
              <span className="font-medium capitalize">{step.status}</span>
            </div>
          </div>

          {step.error && (
            <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
              <p className="text-sm text-red-700 dark:text-red-300 font-medium">Error</p>
              <p className="text-sm text-red-600 dark:text-red-400 mt-1">{step.error}</p>
            </div>
          )}

          {step.input !== undefined && (
            typeof step.input === 'string' ? (
              <TextViewer text={step.input} label="Input" />
            ) : (
              <JsonViewer data={step.input} label="Input" />
            )
          )}

          {step.toolArgs && (
            <JsonViewer data={step.toolArgs} label="Tool Arguments" />
          )}

          {step.output !== undefined && (
            typeof step.output === 'string' ? (
              <TextViewer text={step.output} label="Output" />
            ) : (
              <JsonViewer data={step.output} label="Output" />
            )
          )}

          {step.toolResult !== undefined && (
            typeof step.toolResult === 'string' ? (
              <TextViewer text={step.toolResult} label="Tool Result" />
            ) : (
              <JsonViewer data={step.toolResult} label="Tool Result" />
            )
          )}

          {step.cost?.pricing && (
            <JsonViewer data={step.cost.pricing} label="Pricing (at time of request)" />
          )}
        </div>

        <div className="px-4 py-3 border-t border-border bg-muted/30">
          <button
            onClick={onClose}
            className="w-full px-4 py-2 bg-primary text-primary-foreground rounded-lg hover:opacity-90 transition-opacity text-sm font-medium"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  )
}
