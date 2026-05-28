import { FileText } from 'lucide-react'
import { registerView } from '../registry'
import { MarkdownView } from './MarkdownView'

export function registerMarkdownView() {
  registerView({
    id: 'markdown',
    name: 'Markdown',
    icon: FileText,
    component: MarkdownView,
    extensions: ['.md', '.markdown', '.txt', '.json', '.yaml', '.yml', '.toml', '.js', '.ts', '.tsx', '.jsx', '.css', '.html', '.rs', '.py'],
    priority: 10,
    capabilities: {
      canEdit: true,
      canSave: true,
      canUndo: false,
      canSearch: true,
    },
  })
}

export { MarkdownView }
