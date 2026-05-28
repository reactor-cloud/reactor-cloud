import { useRef, useCallback, useEffect } from 'react'
import type { Tab, ViewRef, ViewProps } from '../types'
import { getView } from '../registry'
import { useViews } from '../hooks/useViews'

interface ViewContainerProps {
  tab: Tab
}

export function ViewContainer({ tab }: ViewContainerProps) {
  const { setTabDirty, setTabTitle, closeTab, registerViewRef, unregisterViewRef } = useViews()
  const viewRef = useRef<ViewRef>(null)
  
  const viewConfig = getView(tab.viewId)
  
  useEffect(() => {
    if (viewRef.current) {
      registerViewRef(tab.id, viewRef.current)
    }
    return () => {
      unregisterViewRef(tab.id)
    }
  }, [tab.id, registerViewRef, unregisterViewRef])
  
  const handleDirty = useCallback((dirty: boolean) => {
    setTabDirty(tab.id, dirty)
  }, [tab.id, setTabDirty])
  
  const handleTitleChange = useCallback((title: string) => {
    setTabTitle(tab.id, title)
  }, [tab.id, setTabTitle])
  
  const handleClose = useCallback(() => {
    closeTab(tab.id)
  }, [tab.id, closeTab])
  
  if (!viewConfig) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <p>View not found: {tab.viewId}</p>
      </div>
    )
  }
  
  const ViewComponent = viewConfig.component
  
  const viewProps: ViewProps = {
    tabId: tab.id,
    filePath: tab.filePath,
    documentId: tab.documentId,
    onDirty: handleDirty,
    onTitleChange: handleTitleChange,
    onClose: handleClose,
    viewRef,
  }
  
  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
      <ViewComponent {...viewProps} />
    </div>
  )
}
