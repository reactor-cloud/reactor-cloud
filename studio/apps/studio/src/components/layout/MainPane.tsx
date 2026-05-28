import { useCallback } from 'react'
import { useViews, TabBar, ViewContainer, EmptyState } from '@/views'

export function MainPane() {
  const {
    tabs,
    activeTabId,
    activeTab,
    switchTab,
    closeTab,
    closeOtherTabs,
    closeAllTabs,
    saveTab,
    reorderTabs,
    openView,
  } = useViews()

  const handleNewTab = useCallback(() => {
    openView({
      viewId: 'new-tab',
      title: 'New Tab',
      documentId: `newtab-${Date.now()}`,
    })
  }, [openView])

  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0 bg-background overflow-hidden">
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onTabClick={switchTab}
        onTabClose={closeTab}
        onTabMiddleClick={closeTab}
        onNewTab={handleNewTab}
        onCloseOthers={closeOtherTabs}
        onCloseAll={closeAllTabs}
        onSaveTab={saveTab}
        onReorder={reorderTabs}
      />

      {activeTab ? (
        <ViewContainer key={activeTab.id} tab={activeTab} />
      ) : (
        <EmptyState onNewFile={handleNewTab} />
      )}
    </div>
  )
}
