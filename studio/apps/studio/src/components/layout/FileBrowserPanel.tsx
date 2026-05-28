import { useCallback, useEffect } from 'react'
import { FolderTree, ChevronRight, ChevronDown, File, Folder, RefreshCw, FilePlus, FolderPlus, Plus } from 'lucide-react'
import { ResizeHandle } from '@/components/ui/ResizeHandle'
import { useResizable } from '@/hooks/useResizable'
import { useFileBrowser, FileTreeNode } from '@/hooks/useFileBrowser'
import { ScrollArea } from '@/components/ui/ScrollArea'
import { cn } from '@/lib/utils'

interface FileTreeItemProps {
  node: FileTreeNode
  level: number
  expanded: boolean
  selected: boolean
  onToggle: (path: string) => void
  onSelect: (path: string) => void
  onOpen: (path: string) => void
  onAddToChat?: (path: string, name: string) => void
  expandedPaths: Set<string>
}

function FileTreeItem({
  node,
  level,
  expanded,
  selected,
  onToggle,
  onSelect,
  onOpen,
  onAddToChat,
  expandedPaths,
}: FileTreeItemProps) {
  const isDirectory = node.type === 'directory'
  const Icon = isDirectory ? (expanded ? ChevronDown : ChevronRight) : File
  const FolderIcon = Folder

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    if (onAddToChat) {
      onAddToChat(node.path, node.name)
    }
  }

  return (
    <div>
      <div
        className={cn(
          'flex items-center gap-1 py-1 px-2 cursor-pointer hover:bg-accent/50 rounded-sm transition-colors group',
          selected && 'bg-accent'
        )}
        style={{ paddingLeft: `${level * 12 + 8}px` }}
        onClick={() => {
          onSelect(node.path)
          if (isDirectory) {
            onToggle(node.path)
          }
        }}
        onDoubleClick={() => {
          if (!isDirectory) {
            onOpen(node.path)
          }
        }}
        onContextMenu={handleContextMenu}
      >
        {isDirectory ? (
          <>
            <Icon className="w-3.5 h-3.5 text-muted-foreground flex-shrink-0" />
            <FolderIcon className="w-4 h-4 text-blue-400 flex-shrink-0" />
          </>
        ) : (
          <>
            <div className="w-3.5" />
            <Icon className="w-4 h-4 text-muted-foreground flex-shrink-0" />
          </>
        )}
        <span className="text-sm truncate flex-1">{node.name}</span>
        {onAddToChat && (
          <button
            onClick={(e) => {
              e.stopPropagation()
              onAddToChat(node.path, node.name)
            }}
            className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-accent text-muted-foreground hover:text-blue-500 transition-all"
            title="Add to chat"
          >
            <Plus className="w-3 h-3" />
          </button>
        )}
      </div>

      {isDirectory && expanded && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeItem
              key={child.path}
              node={child}
              level={level + 1}
              expanded={expandedPaths.has(child.path)}
              selected={false}
              onToggle={onToggle}
              onSelect={onSelect}
              onOpen={onOpen}
              onAddToChat={onAddToChat}
              expandedPaths={expandedPaths}
            />
          ))}
        </div>
      )}
    </div>
  )
}

interface FileBrowserPanelProps {
  workspacePath: string | null
  onOpenFile: (filePath: string) => void
  onAddToChat?: (path: string, name: string) => void
}

export function FileBrowserPanel({ workspacePath, onOpenFile, onAddToChat }: FileBrowserPanelProps) {
  const { size, startResizing } = useResizable({
    initialSize: 280,
    minSize: 200,
    maxSize: 500,
  })

  const {
    fileTree,
    isLoading,
    error,
    expandedPaths,
    selectedPath,
    refresh,
    toggleExpand,
    selectFile,
  } = useFileBrowser({ workspacePath })

  // Debug logging
  useEffect(() => {
    console.log('[FileBrowser] workspacePath:', workspacePath)
    console.log('[FileBrowser] fileTree:', fileTree)
    console.log('[FileBrowser] isLoading:', isLoading)
    console.log('[FileBrowser] error:', error)
  }, [workspacePath, fileTree, isLoading, error])

  const renderTree = useCallback(
    (node: FileTreeNode, level: number = 0) => {
      return (
        <FileTreeItem
          key={node.path}
          node={node}
          level={level}
          expanded={expandedPaths.has(node.path)}
          selected={selectedPath === node.path}
          onToggle={toggleExpand}
          onSelect={selectFile}
          onOpen={onOpenFile}
          onAddToChat={onAddToChat}
          expandedPaths={expandedPaths}
        />
      )
    },
    [expandedPaths, selectedPath, toggleExpand, selectFile, onOpenFile, onAddToChat]
  )

  return (
    <div className="flex h-full">
      <ResizeHandle onMouseDown={startResizing} />

      <div
        className="flex flex-col bg-card border-l border-border"
        style={{ width: size }}
      >
        <div className="h-10 flex items-center justify-between px-3 border-b border-border">
          <div className="flex items-center gap-1.5">
            <FolderTree className="w-4 h-4 text-muted-foreground" />
            <span className="text-sm font-medium">Files</span>
          </div>
          <div className="flex items-center gap-0.5">
            <button
              className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              title="New File"
            >
              <FilePlus className="w-4 h-4" />
            </button>
            <button
              className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              title="New Folder"
            >
              <FolderPlus className="w-4 h-4" />
            </button>
            <button
              onClick={refresh}
              className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
              title="Refresh"
              disabled={isLoading}
            >
              <RefreshCw className={cn('w-4 h-4', isLoading && 'animate-spin')} />
            </button>
          </div>
        </div>

        <div className="flex-1 min-h-0 overflow-hidden">
          <ScrollArea className="h-full">
            {error ? (
              <div className="p-3 text-sm text-destructive">{error}</div>
            ) : !fileTree ? (
              <div className="p-3 text-sm text-muted-foreground">
                {isLoading ? 'Loading...' : 'No folder open'}
              </div>
            ) : !fileTree.children?.length ? (
              <div className="p-3 text-sm text-muted-foreground">
                Folder is empty
              </div>
            ) : (
              <div className="py-1">
                {fileTree.children.map((child) => renderTree(child))}
              </div>
            )}
          </ScrollArea>
        </div>
      </div>
    </div>
  )
}
