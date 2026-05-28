import { createContext, useContext, useState, useCallback, ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface FileClipboard {
  path: string
  name: string
  type: 'file' | 'directory'
  operation: 'copy' | 'cut'
}

interface FileClipboardContextValue {
  clipboard: FileClipboard | null
  copy: (path: string, name: string, type: 'file' | 'directory') => void
  cut: (path: string, name: string, type: 'file' | 'directory') => void
  clear: () => void
  paste: (destFolder: string) => Promise<void>
  canPaste: boolean
}

const FileClipboardContext = createContext<FileClipboardContextValue | null>(null)

export function FileClipboardProvider({ children }: { children: ReactNode }) {
  const [clipboard, setClipboard] = useState<FileClipboard | null>(null)

  const copy = useCallback((path: string, name: string, type: 'file' | 'directory') => {
    setClipboard({ path, name, type, operation: 'copy' })
  }, [])

  const cut = useCallback((path: string, name: string, type: 'file' | 'directory') => {
    setClipboard({ path, name, type, operation: 'cut' })
  }, [])

  const clear = useCallback(() => {
    setClipboard(null)
  }, [])

  const paste = useCallback(async (destFolder: string) => {
    if (!clipboard) return

    const destPath = `${destFolder}/${clipboard.name}`

    try {
      if (clipboard.operation === 'copy') {
        await invoke('file_copy', { source: clipboard.path, dest: destPath })
      } else {
        await invoke('file_move', { source: clipboard.path, dest: destPath })
        setClipboard(null)
      }
    } catch (error) {
      console.error('Paste failed:', error)
      throw error
    }
  }, [clipboard])

  const value: FileClipboardContextValue = {
    clipboard,
    copy,
    cut,
    clear,
    paste,
    canPaste: clipboard !== null,
  }

  return (
    <FileClipboardContext.Provider value={value}>
      {children}
    </FileClipboardContext.Provider>
  )
}

export function useFileClipboard() {
  const context = useContext(FileClipboardContext)
  if (!context) {
    throw new Error('useFileClipboard must be used within a FileClipboardProvider')
  }
  return context
}
