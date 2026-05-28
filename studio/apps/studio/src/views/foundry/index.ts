import { FlaskConical } from 'lucide-react'
import { registerView } from '@/views/registry'
import { FoundryView } from './FoundryView'

export function registerFoundryView() {
  registerView({
    id: 'foundry',
    name: 'Foundry',
    icon: FlaskConical,
    component: FoundryView,
    canOpen: () => true,
    capabilities: {
      canEdit: false,
      canSave: false,
      canUndo: false,
      canSearch: false,
    },
  })
}

export { FoundryView }
