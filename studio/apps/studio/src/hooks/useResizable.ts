import { useState, useCallback, useEffect } from 'react'

interface UseResizableOptions {
  initialSize: number
  minSize: number
  maxSize: number
  direction?: 'horizontal' | 'vertical'
}

export function useResizable({
  initialSize,
  minSize,
  maxSize,
  direction = 'horizontal',
}: UseResizableOptions) {
  const [size, setSize] = useState(initialSize)
  const [isResizing, setIsResizing] = useState(false)

  const startResizing = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    setIsResizing(true)
  }, [])

  const stopResizing = useCallback(() => {
    setIsResizing(false)
  }, [])

  const resize = useCallback(
    (e: MouseEvent) => {
      if (!isResizing) return

      const newSize = direction === 'horizontal' ? e.clientX : e.clientY
      const clampedSize = Math.min(Math.max(newSize, minSize), maxSize)
      setSize(clampedSize)
    },
    [isResizing, minSize, maxSize, direction]
  )

  useEffect(() => {
    if (isResizing) {
      document.addEventListener('mousemove', resize)
      document.addEventListener('mouseup', stopResizing)
      document.body.style.cursor = direction === 'horizontal' ? 'col-resize' : 'row-resize'
      document.body.style.userSelect = 'none'
    }

    return () => {
      document.removeEventListener('mousemove', resize)
      document.removeEventListener('mouseup', stopResizing)
      document.body.style.cursor = ''
      document.body.style.userSelect = ''
    }
  }, [isResizing, resize, stopResizing, direction])

  return {
    size,
    setSize,
    isResizing,
    startResizing,
  }
}
