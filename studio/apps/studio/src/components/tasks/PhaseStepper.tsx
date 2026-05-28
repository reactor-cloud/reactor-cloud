import { cn } from '@/lib/utils'
import { Check, Lock, Play } from 'lucide-react'
import type { PhaseInfo } from '@/lib/ipc'

interface PhaseStepperProps {
  phases: PhaseInfo[]
  currentPhase: string
  onSelectPhase: (phase: string) => void
  className?: string
}

const PHASE_ORDER = [
  'Alignment',
  'Planning',
  'Development',
  'Testing',
  'UAT',
  'Deployment',
]

export function PhaseStepper({
  phases,
  currentPhase,
  onSelectPhase,
  className,
}: PhaseStepperProps) {
  const getPhaseStatus = (phaseName: string): string => {
    const phase = phases.find(
      (p) => p.phase.toLowerCase() === phaseName.toLowerCase()
    )
    return phase?.status || 'locked'
  }

  return (
    <div className={cn('flex flex-col gap-1', className)}>
      {PHASE_ORDER.map((phaseName, _index) => {
        const status = getPhaseStatus(phaseName)
        const isActive = phaseName.toLowerCase() === currentPhase.toLowerCase()
        const isClickable = status !== 'locked'

        return (
          <button
            key={phaseName}
            onClick={() => isClickable && onSelectPhase(phaseName)}
            disabled={!isClickable}
            className={cn(
              'flex items-center gap-2 px-3 py-2 rounded-lg text-left',
              'transition-colors',
              isClickable && 'hover:bg-[var(--background-tertiary)]',
              isActive && 'bg-[var(--background-tertiary)]',
              !isClickable && 'opacity-50 cursor-not-allowed'
            )}
          >
            <PhaseIcon status={status} />
            <span
              className={cn(
                'text-sm',
                isActive
                  ? 'text-[var(--text-primary)] font-medium'
                  : 'text-[var(--text-secondary)]'
              )}
            >
              {phaseName}
            </span>
          </button>
        )
      })}
    </div>
  )
}

function PhaseIcon({ status }: { status: string }) {
  switch (status) {
    case 'completed':
      return (
        <div className="w-5 h-5 rounded-full bg-green-500/20 flex items-center justify-center">
          <Check className="w-3 h-3 text-green-400" />
        </div>
      )
    case 'active':
      return (
        <div className="w-5 h-5 rounded-full bg-blue-500/20 flex items-center justify-center">
          <Play className="w-3 h-3 text-blue-400 fill-current" />
        </div>
      )
    default:
      return (
        <div className="w-5 h-5 rounded-full bg-[var(--background-tertiary)] flex items-center justify-center">
          <Lock className="w-3 h-3 text-[var(--text-tertiary)]" />
        </div>
      )
  }
}
