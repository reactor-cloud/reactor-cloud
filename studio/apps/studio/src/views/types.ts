import type { LucideIcon } from 'lucide-react'
import type { ComponentType, RefObject } from 'react'

export interface FileInfo {
  path: string
  name: string
  extension: string
  mimeType?: string
  size?: number
}

export type ViewStatus = 'idle' | 'loading' | 'saving' | 'error'

export interface ViewConfig {
  id: string
  name: string
  icon: LucideIcon
  
  extensions?: string[]
  mimeTypes?: string[]
  canOpen?: (file: FileInfo) => boolean
  
  priority?: number
  
  capabilities: {
    canEdit: boolean
    canSave: boolean
    canUndo: boolean
    canSearch: boolean
  }
  
  component: ComponentType<ViewProps>
}

export interface ViewProps {
  tabId: string
  filePath?: string
  documentId?: string
  content?: string
  onDirty: (dirty: boolean) => void
  onTitleChange: (title: string) => void
  onClose: () => void
  viewRef?: RefObject<ViewRef>
}

export interface ViewRef {
  save(): Promise<boolean>
  canClose(): Promise<boolean>
  focus(): void
  
  getStatus(): ViewStatus
  getDocumentId?(): string | undefined
  isDirty(): boolean
  
  getContent(): unknown
  setContent(content: unknown): void
  
  executeCommand?(command: string, args: unknown): Promise<unknown>
}

export interface Tab {
  id: string
  viewId: string
  filePath?: string
  documentId?: string
  title: string
  isDirty: boolean
  lastActiveAt: number
  state?: Record<string, unknown>
}

export interface TabsState {
  tabs: Tab[]
  activeTabId: string | null
}

export interface OpenFileOptions {
  filePath: string
  viewId?: string
  focus?: boolean
}

export interface OpenViewOptions {
  viewId: string
  title?: string
  documentId?: string
  focus?: boolean
}
