import { useState, useEffect, useImperativeHandle, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ScrollArea } from '@/components/ui/ScrollArea'
import type { ViewProps, ViewStatus } from '../types'

export function MarkdownView({ tabId: _tabId, filePath, viewRef, onDirty }: ViewProps) {
  const [content, setContent] = useState<string>('')
  const [originalContent, setOriginalContent] = useState<string>('')
  const [status, setStatus] = useState<ViewStatus>('idle')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (filePath) {
      loadFile(filePath)
    }
  }, [filePath])

  const loadFile = async (path: string) => {
    setStatus('loading')
    setError(null)
    try {
      const fileContent: string = await invoke('file_read', { path })
      setContent(fileContent)
      setOriginalContent(fileContent)
      setStatus('idle')
    } catch (err) {
      setError(`Failed to load file: ${err}`)
      setStatus('error')
    }
  }

  const saveFile = useCallback(async (): Promise<boolean> => {
    if (!filePath) return false
    
    setStatus('saving')
    try {
      await invoke('file_write', { path: filePath, contents: content })
      setOriginalContent(content)
      onDirty(false)
      setStatus('idle')
      return true
    } catch (err) {
      setError(`Failed to save file: ${err}`)
      setStatus('error')
      return false
    }
  }, [filePath, content, onDirty])

  const isDirty = useCallback(() => content !== originalContent, [content, originalContent])

  useImperativeHandle(viewRef, () => ({
    save: saveFile,
    canClose: async () => {
      if (!isDirty()) return true
      return confirm('You have unsaved changes. Close anyway?')
    },
    focus: () => {},
    getStatus: () => status,
    isDirty,
    getContent: () => content,
    setContent: (newContent: unknown) => {
      if (typeof newContent === 'string') {
        setContent(newContent)
        onDirty(newContent !== originalContent)
      }
    },
  }))

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newContent = e.target.value
    setContent(newContent)
    onDirty(newContent !== originalContent)
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center text-destructive p-4">
        <p>{error}</p>
      </div>
    )
  }

  if (status === 'loading') {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <p>Loading...</p>
      </div>
    )
  }

  return (
    <ScrollArea className="flex-1">
      <textarea
        value={content}
        onChange={handleChange}
        className="w-full h-full min-h-[calc(100vh-8rem)] p-4 bg-transparent resize-none focus:outline-none font-mono text-sm"
        placeholder="Start writing..."
        spellCheck={false}
      />
    </ScrollArea>
  )
}
