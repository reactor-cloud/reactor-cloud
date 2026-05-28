import { useState, useRef, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { Send, Square } from 'lucide-react'

interface PromptInputProps {
  onSubmit: (message: string) => void
  onCancel?: () => void
  isStreaming?: boolean
  disabled?: boolean
  placeholder?: string
  className?: string
}

export function PromptInput({
  onSubmit,
  onCancel,
  isStreaming = false,
  disabled = false,
  placeholder = 'Type @ to mention files...',
  className,
}: PromptInputProps) {
  const [value, setValue] = useState('')
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim()
    if (!trimmed || disabled || isStreaming) return
    onSubmit(trimmed)
    setValue('')
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
    }
  }, [value, disabled, isStreaming, onSubmit])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit()
    }
  }

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setValue(e.target.value)
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 200)}px`
    }
  }

  return (
    <div className={cn('relative', className)}>
      <div className="flex items-end gap-2 p-2 bg-background border border-border rounded-xl">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={disabled || isStreaming}
          rows={1}
          className={cn(
            'flex-1 resize-none bg-transparent border-none outline-none',
            'text-sm text-foreground placeholder:text-muted-foreground',
            'min-h-[40px] max-h-[200px] py-2 px-2',
            'disabled:opacity-50'
          )}
        />

        {isStreaming ? (
          <button
            onClick={onCancel}
            className="flex-shrink-0 w-9 h-9 flex items-center justify-center rounded-lg bg-destructive text-destructive-foreground hover:opacity-90 transition-opacity"
          >
            <Square className="w-4 h-4" />
          </button>
        ) : (
          <button
            onClick={handleSubmit}
            disabled={!value.trim() || disabled}
            className={cn(
              'flex-shrink-0 w-9 h-9 flex items-center justify-center rounded-full transition-all',
              value.trim() && !disabled
                ? 'bg-blue-500 text-white hover:bg-blue-600'
                : 'bg-muted text-muted-foreground cursor-not-allowed'
            )}
          >
            <Send className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  )
}
