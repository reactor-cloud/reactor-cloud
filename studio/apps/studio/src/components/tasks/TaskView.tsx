import { useState, useEffect, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { ArrowLeft, Play } from 'lucide-react'
import { Button } from '@/components/ui'
import { PhaseStepper } from './PhaseStepper'
import { ChatView } from '@/components/chat'
import {
  taskGet,
  taskAdvance,
  type TaskInfo,
} from '@/lib/ipc'

interface TaskViewProps {
  taskId: string
  onBack: () => void
  className?: string
}

export function TaskView({ taskId, onBack, className }: TaskViewProps) {
  const [task, setTask] = useState<TaskInfo | null>(null)
  const [selectedPhase, setSelectedPhase] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    loadTask()
  }, [taskId])

  async function loadTask() {
    setLoading(true)
    try {
      const t = await taskGet(taskId)
      setTask(t)
      setSelectedPhase(t.currentPhase)
    } catch (err) {
      console.error('Failed to load task:', err)
    } finally {
      setLoading(false)
    }
  }

  const handleAdvance = useCallback(async () => {
    try {
      const updated = await taskAdvance(taskId)
      setTask(updated)
      setSelectedPhase(updated.currentPhase)
    } catch (err) {
      console.error('Failed to advance task:', err)
    }
  }, [taskId])

  if (loading || !task) {
    return (
      <div className={cn('flex items-center justify-center h-full', className)}>
        <p className="text-sm text-[var(--text-tertiary)]">Loading...</p>
      </div>
    )
  }

  const canAdvance =
    task.state === 'active' &&
    task.currentPhase !== 'Deployment' &&
    selectedPhase?.toLowerCase() === task.currentPhase.toLowerCase()

  return (
    <div className={cn('flex flex-col h-full', className)}>
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-[var(--border)]">
        <Button variant="ghost" size="sm" onClick={onBack} className="w-7 h-7 p-0">
          <ArrowLeft className="w-4 h-4" />
        </Button>
        <div className="flex-1 min-w-0">
          <h2 className="text-sm font-semibold text-[var(--text-primary)] truncate">
            {task.title}
          </h2>
          <p className="text-xs text-[var(--text-tertiary)]">
            {task.currentPhase} · {Math.round(task.progress)}% complete
          </p>
        </div>
        {canAdvance && (
          <Button variant="default" size="sm" onClick={handleAdvance}>
            <Play className="w-3 h-3 mr-1" />
            Next Phase
          </Button>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Phase stepper */}
        <div className="w-48 p-3 border-r border-[var(--border)] overflow-auto">
          <PhaseStepper
            phases={task.phases}
            currentPhase={selectedPhase || task.currentPhase}
            onSelectPhase={setSelectedPhase}
          />
        </div>

        {/* Phase conversation */}
        <div className="flex-1 overflow-hidden">
          {selectedPhase && (
            <PhaseConversation
              taskId={taskId}
              phase={selectedPhase}
              isActive={
                selectedPhase.toLowerCase() === task.currentPhase.toLowerCase()
              }
            />
          )}
        </div>
      </div>
    </div>
  )
}

interface PhaseConversationProps {
  taskId: string
  phase: string
  isActive: boolean
}

function PhaseConversation({ taskId, phase, isActive }: PhaseConversationProps) {
  // For the phase conversation, we use the phase name as a pseudo conversation ID
  const conversationId = `${taskId}:${phase.toLowerCase()}`

  // Get agent based on phase
  const agentId = ['alignment', 'planning'].includes(phase.toLowerCase())
    ? 'planner'
    : 'coder'

  return (
    <div className="h-full flex flex-col">
      {isActive ? (
        <ChatView
          agentId={agentId}
          conversationId={conversationId}
          className="flex-1"
        />
      ) : (
        <div className="flex-1 flex items-center justify-center p-4">
          <p className="text-sm text-[var(--text-tertiary)] text-center">
            This phase is read-only.
          </p>
        </div>
      )}
    </div>
  )
}
