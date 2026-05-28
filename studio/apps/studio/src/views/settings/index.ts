import { Settings } from 'lucide-react'
import { registerView } from '@/views/registry'
import { SettingsView } from './SettingsView'

export function registerSettingsView() {
  registerView({
    id: 'settings',
    name: 'Settings',
    icon: Settings,
    component: SettingsView,
    canOpen: () => true,
    capabilities: {
      canEdit: false,
      canSave: false,
      canUndo: false,
      canSearch: false,
    },
  })
}

export { SettingsView }
