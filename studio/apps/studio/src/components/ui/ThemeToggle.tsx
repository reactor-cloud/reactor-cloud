import { useState, useRef, useEffect } from 'react'
import { Sun, Moon, Monitor } from 'lucide-react'
import { useTheme } from '@/hooks/useTheme'
import { cn } from '@/lib/utils'

export function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const options = [
    { value: 'light' as const, label: 'Light', icon: Sun },
    { value: 'dark' as const, label: 'Dark', icon: Moon },
    { value: 'system' as const, label: 'System', icon: Monitor },
  ]

  const CurrentIcon = theme === 'light' ? Sun : theme === 'dark' ? Moon : Monitor

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
        aria-label="Toggle theme"
      >
        <CurrentIcon className="w-4 h-4" />
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 w-32 py-1 bg-popover border border-border rounded-lg shadow-lg z-50">
          {options.map(({ value, label, icon: Icon }) => (
            <button
              key={value}
              onClick={() => {
                setTheme(value)
                setOpen(false)
              }}
              className={cn(
                'w-full flex items-center gap-2 px-3 py-1.5 text-sm hover:bg-accent transition-colors',
                theme === value && 'text-foreground bg-accent'
              )}
            >
              <Icon className="w-4 h-4" />
              {label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
