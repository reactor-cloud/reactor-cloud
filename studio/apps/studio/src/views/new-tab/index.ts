import { Plus } from 'lucide-react'
import { registerView } from '../registry'
import { NewTabView } from './NewTabView'

export function registerNewTabView() {
  registerView({
    id: 'new-tab',
    name: 'New Tab',
    icon: Plus,
    component: NewTabView,
    capabilities: {
      canEdit: false,
      canSave: false,
      canUndo: false,
      canSearch: false,
    },
  })
}

export { NewTabView }
