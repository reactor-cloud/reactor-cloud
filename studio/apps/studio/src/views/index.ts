export * from './types'
export * from './registry'
export { useViews, ViewsProvider } from './hooks/useViews'
export { TabBar } from './components/TabBar'
export { ViewContainer } from './components/ViewContainer'
export { EmptyState } from './components/EmptyState'

import { registerNewTabView } from './new-tab'
import { registerMarkdownView } from './markdown'
import { registerSettingsView } from './settings'
import { registerFoundryView } from './foundry'

export function initializeViews() {
  registerNewTabView()
  registerMarkdownView()
  registerSettingsView()
  registerFoundryView()
}
