import { cn } from '@/lib/utils'

interface ResizeHandleProps {
  onMouseDown: (e: React.MouseEvent) => void
  orientation?: 'vertical' | 'horizontal'
  className?: string
}

export function ResizeHandle({
  onMouseDown,
  orientation = 'vertical',
  className,
}: ResizeHandleProps) {
  return (
    <div
      onMouseDown={onMouseDown}
      className={cn(
        'group flex-shrink-0 transition-colors relative',
        orientation === 'vertical'
          ? 'w-px cursor-col-resize'
          : 'h-px cursor-row-resize',
        className
      )}
    >
      {/* Invisible wider hit area for easier grabbing */}
      <div
        className={cn(
          'absolute inset-0 z-10',
          orientation === 'vertical' ? '-left-1 -right-1' : '-top-1 -bottom-1'
        )}
      />
      {/* Visible line on hover */}
      <div
        className={cn(
          'bg-primary/50 opacity-0 group-hover:opacity-100 transition-opacity',
          orientation === 'vertical' ? 'w-px h-full' : 'h-px w-full'
        )}
      />
    </div>
  )
}
