import { useState } from 'react'
import {
  User,
  Bot,
  Wrench,
  CheckCircle,
  XCircle,
  Clock,
  Coins,
  Hash,
  ChevronRight,
  Loader2,
  AlertCircle,
  Cpu,
} from 'lucide-react'
import { ScrollArea } from '@/components/ui/ScrollArea'
import {
  useTrace,
  formatDuration,
  formatCost,
  formatTokens,
  type TraceStep,
  type TraceMetrics,
} from '@/hooks/useTrace'
import { cn } from '@/lib/utils'
import { TraceStepModal } from './TraceStepModal'

interface TraceViewProps {
  conversationId: string | null
}

function MetricsBadge({ icon: Icon, label, value, className }: {
  icon: typeof Clock
  label: string
  value: string
  className?: string
}) {
  return (
    <div className={cn('flex items-center gap-1.5 px-2 py-1 rounded-md bg-muted/50', className)}>
      <Icon className="w-3.5 h-3.5 text-muted-foreground" />
      <span className="text-xs text-muted-foreground">{label}:</span>
      <span className="text-xs font-medium text-foreground">{value}</span>
    </div>
  )
}

function MetricsBar({ metrics }: { metrics: TraceMetrics }) {
  return (
    <div className="flex flex-wrap gap-2 p-3 border-b border-border bg-card/50">
      <MetricsBadge
        icon={Clock}
        label="Duration"
        value={formatDuration(metrics.totalDuration)}
      />
      <MetricsBadge
        icon={Cpu}
        label="LLM Calls"
        value={String(metrics.llmCalls)}
      />
      <MetricsBadge
        icon={Wrench}
        label="Tool Calls"
        value={String(metrics.toolCalls)}
      />
      <MetricsBadge
        icon={Hash}
        label="Tokens"
        value={`${formatTokens(metrics.totalInputTokens)} / ${formatTokens(metrics.totalOutputTokens)}`}
      />
      <MetricsBadge
        icon={Coins}
        label="Cost"
        value={formatCost(metrics.estimatedCost)}
        className="text-emerald-600 dark:text-emerald-400"
      />
    </div>
  )
}

function getStepIcon(step: TraceStep) {
  switch (step.type) {
    case 'user_message':
      return User
    case 'llm_request':
    case 'llm_response':
      return Bot
    case 'tool_call':
    case 'tool_result':
      return Wrench
    case 'error':
      return AlertCircle
    default:
      return Clock
  }
}

function getStepTitle(step: TraceStep): string {
  switch (step.type) {
    case 'user_message':
      return 'User Message'
    case 'llm_request':
      return `LLM Request${step.model ? ` (${step.model.split('/').pop()})` : ''}`
    case 'llm_response':
      return 'LLM Response'
    case 'tool_call':
      return `Tool: ${step.toolName || 'unknown'}`
    case 'tool_result':
      return `Result: ${step.toolName || 'unknown'}`
    case 'subagent_call':
      return `Subagent: ${step.subagentType || 'unknown'}`
    case 'error':
      return 'Error'
    default:
      return step.type
  }
}

function getStepSummary(step: TraceStep): string {
  switch (step.type) {
    case 'user_message':
      return typeof step.input === 'string' ? step.input.slice(0, 100) : ''
    case 'llm_request':
      return step.model || ''
    case 'llm_response': {
      const output = step.output as { content?: string; toolCallCount?: number } | string | undefined
      if (typeof output === 'string') {
        return output.slice(0, 80)
      }
      if (output && typeof output === 'object') {
        const parts = []
        if (output.content) parts.push(`${output.content.length} chars`)
        if (output.toolCallCount) parts.push(`${output.toolCallCount} tool calls`)
        return parts.join(', ')
      }
      return ''
    }
    case 'tool_call':
      return JSON.stringify(step.toolArgs || {}).slice(0, 80)
    case 'tool_result':
      if (step.error) return `Error: ${step.error}`
      return typeof step.toolResult === 'string' 
        ? step.toolResult.slice(0, 80) 
        : JSON.stringify(step.toolResult || {}).slice(0, 80)
    case 'subagent_call':
      return step.subagentDescription || ''
    case 'error':
      return step.error || ''
    default:
      return ''
  }
}

function StepRow({ step, onClick }: { step: TraceStep; onClick: () => void }) {
  const Icon = getStepIcon(step)
  const title = getStepTitle(step)
  const summary = getStepSummary(step)

  const statusColor = {
    pending: 'text-muted-foreground',
    running: 'text-blue-500',
    success: 'text-emerald-500',
    error: 'text-red-500',
  }[step.status]

  const StatusIcon = {
    pending: Clock,
    running: Loader2,
    success: CheckCircle,
    error: XCircle,
  }[step.status]

  return (
    <button
      onClick={onClick}
      className="w-full flex items-start gap-3 p-3 text-left hover:bg-accent/50 transition-colors border-b border-border/50 last:border-0"
    >
      <div className={cn(
        'w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5',
        step.type === 'error' ? 'bg-red-100 dark:bg-red-900/30' : 'bg-muted'
      )}>
        <Icon className={cn(
          'w-3.5 h-3.5',
          step.type === 'error' ? 'text-red-500' : 'text-muted-foreground'
        )} />
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-medium text-sm text-foreground">{title}</span>
          <StatusIcon className={cn('w-3.5 h-3.5', statusColor, step.status === 'running' && 'animate-spin')} />
        </div>

        {summary && (
          <p className="text-xs text-muted-foreground mt-0.5 truncate">{summary}</p>
        )}

        <div className="flex items-center gap-3 mt-1">
          {step.duration !== undefined && (
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Clock className="w-3 h-3" />
              {formatDuration(step.duration)}
            </span>
          )}

          {step.tokenUsage && (
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Hash className="w-3 h-3" />
              {formatTokens(step.tokenUsage.inputTokens)} / {formatTokens(step.tokenUsage.outputTokens)}
            </span>
          )}

          {step.cost && (
            <span className="text-xs text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
              <Coins className="w-3 h-3" />
              {formatCost(step.cost.totalCost)}
            </span>
          )}
        </div>
      </div>

      <ChevronRight className="w-4 h-4 text-muted-foreground flex-shrink-0 mt-1" />
    </button>
  )
}

export function TraceView({ conversationId }: TraceViewProps) {
  const { trace, steps, metrics, loading, error } = useTrace({
    conversationId,
    autoRefresh: true,
    refreshInterval: 2000,
  })

  const [selectedStep, setSelectedStep] = useState<TraceStep | null>(null)

  if (!conversationId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
        <Clock className="w-12 h-12 text-muted-foreground/30 mb-4" />
        <p className="text-sm text-muted-foreground">
          Select a conversation to view its trace
        </p>
      </div>
    )
  }

  if (loading && !trace) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="w-6 h-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
        <AlertCircle className="w-12 h-12 text-red-500/50 mb-4" />
        <p className="text-sm text-red-500">{error}</p>
      </div>
    )
  }

  if (!trace || steps.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
        <Clock className="w-12 h-12 text-muted-foreground/30 mb-4" />
        <p className="text-sm text-foreground font-medium mb-1">No trace data yet</p>
        <p className="text-sm text-muted-foreground">
          Send a message to start recording trace data
        </p>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
      {metrics && <MetricsBar metrics={metrics} />}

      <ScrollArea className="flex-1 min-h-0">
        <div className="divide-y divide-border/50">
          {steps.map((step) => (
            <StepRow
              key={step.id}
              step={step}
              onClick={() => setSelectedStep(step)}
            />
          ))}
        </div>
      </ScrollArea>

      {selectedStep && (
        <TraceStepModal
          step={selectedStep}
          onClose={() => setSelectedStep(null)}
        />
      )}
    </div>
  )
}
