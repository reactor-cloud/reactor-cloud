import { useMemo } from 'react'
import { cn } from '@/lib/utils'

interface StreamingMarkdownProps {
  content: string
  isStreaming?: boolean
  className?: string
}

export function StreamingMarkdown({ content, isStreaming, className }: StreamingMarkdownProps) {
  const rendered = useMemo(() => {
    let html = content
      .replace(/```(\w+)?\n([\s\S]*?)```/g, (_, lang, code) => {
        return `<pre class="bg-muted rounded-lg p-3 overflow-x-auto my-2"><code class="text-sm language-${lang || 'text'}">${escapeHtml(code.trim())}</code></pre>`
      })
      .replace(/`([^`]+)`/g, '<code class="bg-muted px-1.5 py-0.5 rounded text-sm">$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
      .replace(/\*([^*]+)\*/g, '<em>$1</em>')
      .replace(/^### (.+)$/gm, '<h3 class="text-lg font-semibold mt-4 mb-2">$1</h3>')
      .replace(/^## (.+)$/gm, '<h2 class="text-xl font-semibold mt-4 mb-2">$1</h2>')
      .replace(/^# (.+)$/gm, '<h1 class="text-2xl font-bold mt-4 mb-2">$1</h1>')
      .replace(/^- (.+)$/gm, '<li class="ml-4">$1</li>')
      .replace(/\n\n/g, '</p><p class="my-2">')
      .replace(/\n/g, '<br />')

    html = `<p class="my-2">${html}</p>`

    return html
  }, [content])

  return (
    <div
      className={cn(
        'prose prose-sm max-w-none text-foreground',
        'prose-headings:text-foreground prose-strong:text-foreground',
        'prose-code:text-foreground prose-pre:bg-muted',
        isStreaming && 'animate-pulse',
        className
      )}
      dangerouslySetInnerHTML={{ __html: rendered }}
    />
  )
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}
