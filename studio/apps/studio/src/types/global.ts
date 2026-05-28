// Global type declarations for Reactor Studio
// Tauri types are provided by @tauri-apps/api

export interface Tab {
  id: string
  title: string
  viewId: string
  filePath?: string
  isDirty: boolean
}

export interface Agent {
  id: string
  name: string
  color: string
  icon?: string
}

export interface Conversation {
  id: string
  agentId: string
  title: string
  created: string
  updated: string
}

export interface Task {
  id: string
  title: string
  status: 'alignment' | 'planning' | 'development' | 'testing' | 'uat' | 'deployment' | 'completed'
  created: string
  updated: string
}
