import { useState, useRef, useEffect, useCallback } from 'react'
import { X, Circle, Plus, ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { Tab } from '../types'
import { getView } from '../registry'

interface TabBarProps {
  tabs: Tab[]
  activeTabId: string | null
  onTabClick: (tabId: string) => void
  onTabClose: (tabId: string) => void
  onTabMiddleClick?: (tabId: string) => void
  onNewTab?: () => void
  onCloseOthers?: (tabId: string) => void
  onCloseAll?: () => void
  onSaveTab?: (tabId: string) => void
  onReorder?: (tabs: Tab[]) => void
}

interface TabItemProps {
  tab: Tab
  isActive: boolean
  onClick: () => void
  onClose: (e: React.MouseEvent) => void
  onMiddleClick?: () => void
}

function TabItem({
  tab,
  isActive,
  onClick,
  onClose,
  onMiddleClick,
}: TabItemProps) {
  const viewConfig = getView(tab.viewId)
  const Icon = viewConfig?.icon

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button === 1 && onMiddleClick) {
      e.preventDefault()
      onMiddleClick()
    }
  }

  const handleCloseClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    onClose(e)
  }

  const tooltipText = tab.filePath
    ? tab.filePath
    : tab.documentId
      ? `${viewConfig?.name || 'View'}: ${tab.documentId}`
      : tab.title

  return (
    <div
      className={cn(
        'group flex items-center gap-1.5 px-3 cursor-pointer',
        'border-r border-border/50 min-w-[120px] max-w-[200px] flex-shrink-0',
        'hover:bg-accent/50 transition-colors',
        isActive
          ? 'bg-background border-b-2 border-b-primary'
          : 'bg-card/50'
      )}
      onClick={onClick}
      onMouseDown={handleMouseDown}
      title={tooltipText}
    >
      {Icon && (
        <Icon className="w-3.5 h-3.5 shrink-0 text-muted-foreground" />
      )}

      <span
        className={cn(
          'truncate text-xs flex-1',
          isActive ? 'text-foreground' : 'text-muted-foreground'
        )}
      >
        {tab.title}
      </span>

      <div className="flex items-center shrink-0">
        {tab.isDirty ? (
          <Circle
            className="w-2 h-2 fill-current text-amber-500"
            aria-label="Unsaved changes"
          />
        ) : (
          <button
            className={cn(
              'w-4 h-4 rounded-sm flex items-center justify-center',
              'opacity-0 group-hover:opacity-100 transition-opacity',
              'hover:bg-accent'
            )}
            onClick={handleCloseClick}
            aria-label="Close tab"
          >
            <X className="w-3 h-3" />
          </button>
        )}
      </div>
    </div>
  )
}

export function TabBar({
  tabs,
  activeTabId,
  onTabClick,
  onTabClose,
  onTabMiddleClick,
  onNewTab,
}: TabBarProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)

  const checkScrollState = useCallback(() => {
    const container = scrollContainerRef.current
    if (!container) return

    const { scrollLeft, scrollWidth, clientWidth } = container
    setCanScrollLeft(scrollLeft > 0)
    setCanScrollRight(scrollLeft + clientWidth < scrollWidth - 1)
  }, [])

  useEffect(() => {
    checkScrollState()
    const container = scrollContainerRef.current
    if (!container) return

    const resizeObserver = new ResizeObserver(checkScrollState)
    resizeObserver.observe(container)

    container.addEventListener('scroll', checkScrollState)
    return () => {
      resizeObserver.disconnect()
      container.removeEventListener('scroll', checkScrollState)
    }
  }, [checkScrollState, tabs.length])

  const scrollBy = (direction: 'left' | 'right') => {
    const container = scrollContainerRef.current
    if (!container) return

    const scrollAmount = 200
    container.scrollBy({
      left: direction === 'left' ? -scrollAmount : scrollAmount,
      behavior: 'smooth',
    })
  }

  const handleWheel = (e: React.WheelEvent) => {
    const container = scrollContainerRef.current
    if (!container) return

    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      return
    }

    e.preventDefault()
    container.scrollBy({
      left: e.deltaY,
      behavior: 'auto',
    })
  }

  return (
    <div className="h-10 flex items-stretch bg-card border-b border-border flex-shrink-0">
      {canScrollLeft && (
        <button
          onClick={() => scrollBy('left')}
          className="flex items-center justify-center px-1 hover:bg-accent/50 transition-colors text-muted-foreground hover:text-foreground border-r border-border/50"
          aria-label="Scroll tabs left"
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
      )}

      <div
        ref={scrollContainerRef}
        className="flex-1 flex items-stretch overflow-x-auto scrollbar-none"
        onWheel={handleWheel}
      >
        {tabs.map((tab) => (
          <TabItem
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            onClick={() => onTabClick(tab.id)}
            onClose={() => onTabClose(tab.id)}
            onMiddleClick={
              onTabMiddleClick ? () => onTabMiddleClick(tab.id) : undefined
            }
          />
        ))}
      </div>

      {canScrollRight && (
        <button
          onClick={() => scrollBy('right')}
          className="flex items-center justify-center px-1 hover:bg-accent/50 transition-colors text-muted-foreground hover:text-foreground border-l border-border/50"
          aria-label="Scroll tabs right"
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      )}

      {onNewTab && (
        <button
          onClick={onNewTab}
          className={cn(
            'flex items-center justify-center px-3',
            'hover:bg-accent/50 transition-colors',
            'text-muted-foreground hover:text-foreground',
            'border-l border-border/50'
          )}
          aria-label="New tab"
        >
          <Plus className="w-4 h-4" />
        </button>
      )}
    </div>
  )
}
