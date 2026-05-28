import { cn } from '@/lib/utils'
import { ScrollArea, Button } from '@/components/ui'
import { Plus, CheckCircle, Circle, Clock } from 'lucide-react'
import type { TaskSummary } from '@/lib/ipc'

interface TaskListProps {
  tasks: TaskSummary[]
  activeTaskId: string | null
  onSelectTask: (taskId: string) => void
  onNewTask: () => void
  className?: string
}

export function TaskList({
  tasks,
  activeTaskId,
  onSelectTask,
  onNewTask,
  className,
}: TaskListProps) {
  return (
    <div className={cn('flex flex-col', className)}>
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
        <h2 className="text-sm font-semibold text-[var(--text-primary)]">
          Tasks
        </h2>
        <Button
          variant="ghost"
          size="sm"
          onClick={onNewTask}
          className="w-7 h-7 p-0"
        >
          <Plus className="w-4 h-4" />
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="p-2 space-y-1">
          {tasks.length === 0 ? (
            <p className="px-3 py-6 text-center text-sm text-[var(--text-tertiary)]">
              No tasks yet. Create one to get started.
            </p>
          ) : (
            tasks.map((task) => (
              <div
                key={task.id}
                className={cn(
                  'flex items-center gap-3 px-3 py-2 rounded-lg',
                  'cursor-pointer transition-colors',
                  'hover:bg-[var(--background-tertiary)]',
                  activeTaskId === task.id && 'bg-[var(--background-tertiary)]'
                )}
                onClick={() => onSelectTask(task.id)}
              >
                <TaskStateIcon state={task.state} />
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-[var(--text-primary)] truncate">
                    {task.title}
                  </p>
                  <p className="text-xs text-[var(--text-tertiary)]">
                    {task.currentPhase} · {Math.round(task.progress)}%
                  </p>
                </div>
              </div>
            ))
          )}
        </div>
      </ScrollArea>
    </div>
  )
}

function TaskStateIcon({ state }: { state: string }) {
  switch (state) {
    case 'completed':
      return <CheckCircle className="w-4 h-4 text-green-400 shrink-0" />
    case 'active':
      return <Clock className="w-4 h-4 text-blue-400 shrink-0" />
    default:
      return <Circle className="w-4 h-4 text-[var(--text-tertiary)] shrink-0" />
  }
}
