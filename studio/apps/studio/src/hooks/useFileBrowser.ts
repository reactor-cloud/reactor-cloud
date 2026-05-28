import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'

export interface FileTreeNode {
  name: string
  path: string
  type: 'file' | 'directory'
  children?: FileTreeNode[]
}

interface FileEntry {
  name: string
  path: string
  isDir: boolean
  isFile: boolean
  size?: number
}

interface UseFileBrowserOptions {
  workspacePath: string | null
  autoLoad?: boolean
}

async function loadDirectory(dirPath: string): Promise<FileTreeNode[]> {
  try {
    console.log('[useFileBrowser] Loading directory:', dirPath)
    const entries: FileEntry[] = await invoke('file_list', { path: dirPath })
    console.log('[useFileBrowser] Loaded entries:', entries)
    return entries.map((entry) => ({
      name: entry.name,
      path: entry.path,
      type: entry.isDir ? 'directory' : 'file',
    }))
  } catch (error) {
    console.error('[useFileBrowser] Failed to load directory:', error)
    return []
  }
}

export function useFileBrowser({ workspacePath, autoLoad = true }: UseFileBrowserOptions) {
  const [rootFolder, setRootFolder] = useState<string | null>(null)
  const [fileTree, setFileTree] = useState<FileTreeNode | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [selectedPath, setSelectedPath] = useState<string | null>(null)

  const loadFileTree = useCallback(async (folderPath: string) => {
    console.log('[useFileBrowser] loadFileTree called with:', folderPath)
    setIsLoading(true)
    setError(null)

    try {
      const children = await loadDirectory(folderPath)
      console.log('[useFileBrowser] Got children:', children.length, 'items')
      const tree: FileTreeNode = {
        name: folderPath.split('/').pop() || folderPath,
        path: folderPath,
        type: 'directory',
        children,
      }
      setFileTree(tree)
      setRootFolder(folderPath)

      const initialExpanded = new Set<string>()
      initialExpanded.add(folderPath)
      setExpandedPaths(initialExpanded)
      console.log('[useFileBrowser] Tree set successfully')
    } catch (err) {
      console.error('[useFileBrowser] Error loading tree:', err)
      setError((err as Error).message)
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    if (autoLoad && workspacePath) {
      loadFileTree(workspacePath)
    }
  }, [autoLoad, workspacePath, loadFileTree])

  const refresh = useCallback(() => {
    if (rootFolder) {
      loadFileTree(rootFolder)
    }
  }, [rootFolder, loadFileTree])

  const toggleExpand = useCallback(async (path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })

    // Load children if expanding a directory that hasn't been loaded
    if (fileTree) {
      const findNode = (node: FileTreeNode): FileTreeNode | null => {
        if (node.path === path) return node
        if (node.children) {
          for (const child of node.children) {
            const found = findNode(child)
            if (found) return found
          }
        }
        return null
      }

      const node = findNode(fileTree)
      if (node && node.type === 'directory' && !node.children?.some(c => c.children !== undefined)) {
        // Load children for directories we haven't expanded yet
        const children = await loadDirectory(path)
        setFileTree((prev) => {
          if (!prev) return prev
          const updateNode = (n: FileTreeNode): FileTreeNode => {
            if (n.path === path) {
              return { ...n, children: children.map(c => ({ ...c })) }
            }
            if (n.children) {
              return { ...n, children: n.children.map(updateNode) }
            }
            return n
          }
          return updateNode(prev)
        })
      }
    }
  }, [fileTree])

  const selectFile = useCallback((path: string) => {
    setSelectedPath(path)
  }, [])

  const expandToPath = useCallback((targetPath: string) => {
    if (!rootFolder) return

    const relativePath = targetPath.replace(rootFolder, '')
    const parts = relativePath.split('/').filter(Boolean)
    
    setExpandedPaths((prev) => {
      const next = new Set(prev)
      let currentPath = rootFolder
      for (const part of parts.slice(0, -1)) {
        currentPath = `${currentPath}/${part}`
        next.add(currentPath)
      }
      return next
    })
  }, [rootFolder])

  return {
    rootFolder,
    fileTree,
    isLoading,
    error,
    expandedPaths,
    selectedPath,
    loadFileTree,
    refresh,
    toggleExpand,
    selectFile,
    expandToPath,
  }
}
