import { cn } from '@/lib/utils'
import { User } from 'lucide-react'

interface UserMessageBubbleProps {
  content: string
  className?: string
}

export function UserMessageBubble({ content, className }: UserMessageBubbleProps) {
  return (
    <div className={cn('flex gap-3', className)}>
      <div className="shrink-0 w-8 h-8 rounded-full bg-blue-500/20 flex items-center justify-center">
        <User className="w-4 h-4 text-blue-400" />
      </div>

      <div className="flex-1 min-w-0">
        <div className="text-sm text-foreground whitespace-pre-wrap">
          {content}
        </div>
      </div>
    </div>
  )
}
