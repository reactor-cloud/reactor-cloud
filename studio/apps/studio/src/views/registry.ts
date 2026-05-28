import type { ViewConfig, FileInfo } from './types'

const viewRegistry = new Map<string, ViewConfig>()

export function registerView(config: ViewConfig): void {
  if (viewRegistry.has(config.id)) {
    console.warn(`View "${config.id}" is already registered. Overwriting.`)
  }
  viewRegistry.set(config.id, config)
}

export function unregisterView(viewId: string): boolean {
  return viewRegistry.delete(viewId)
}

export function getView(viewId: string): ViewConfig | undefined {
  return viewRegistry.get(viewId)
}

export function getAllViews(): ViewConfig[] {
  return Array.from(viewRegistry.values())
}

export function getViewsForFile(file: FileInfo): ViewConfig[] {
  const matchingViews: ViewConfig[] = []
  
  for (const config of viewRegistry.values()) {
    let matches = false
    
    if (config.extensions?.some(ext => file.extension.toLowerCase() === ext.toLowerCase())) {
      matches = true
    }
    
    if (config.mimeTypes?.some(mime => file.mimeType === mime)) {
      matches = true
    }
    
    if (config.canOpen?.(file)) {
      matches = true
    }
    
    if (matches) {
      matchingViews.push(config)
    }
  }
  
  return matchingViews.sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0))
}

export function getDefaultViewForFile(file: FileInfo): ViewConfig | undefined {
  const views = getViewsForFile(file)
  return views[0]
}

export function getFileInfoFromPath(filePath: string): FileInfo {
  const name = filePath.split('/').pop() || filePath
  const lastDot = name.lastIndexOf('.')
  const extension = lastDot > 0 ? name.slice(lastDot) : ''
  
  return {
    path: filePath,
    name,
    extension,
  }
}
