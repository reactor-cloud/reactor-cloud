import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// Workspace types
export interface WorkspaceInfo {
  projectId: string
  projectName: string
  path: string
}

// File types
export interface FileEntry {
  name: string
  path: string
  isDir: boolean
  isFile: boolean
  size?: number
}

// Agent types
export interface AgentSummary {
  id: string
  name: string
  color: string
  icon?: string
  model: string
}

export interface StreamChunk {
  type: 'text' | 'thinking' | 'tool_call' | 'tool_result' | 'error' | 'done'
  content?: string
  id?: string
  name?: string
  arguments?: unknown
  toolCallId?: string
  output?: string
  isError?: boolean
  message?: string
  finishReason?: string
}

// IPC Commands

export async function workspaceOpen(path: string): Promise<WorkspaceInfo> {
  return invoke('workspace_open', { path })
}

export async function fileRead(path: string): Promise<string> {
  return invoke('file_read', { path })
}

export async function fileWrite(path: string, contents: string): Promise<void> {
  return invoke('file_write', { path, contents })
}

export async function fileList(path: string): Promise<FileEntry[]> {
  return invoke('file_list', { path })
}

// Agent Commands

export async function agentList(): Promise<AgentSummary[]> {
  return invoke('agent_list')
}

export async function agentSend(
  agentId: string,
  conversationId: string,
  message: string
): Promise<void> {
  console.log('[IPC] agentSend called:', { agentId, conversationId, message: message.substring(0, 50) })
  try {
    await invoke('agent_send', { agentId, conversationId, message })
    console.log('[IPC] agentSend completed successfully')
  } catch (err) {
    console.error('[IPC] agentSend failed:', err)
    throw err
  }
}

export async function agentCancel(conversationId: string): Promise<void> {
  return invoke('agent_cancel', { conversationId })
}

export async function toolApprove(
  conversationId: string,
  toolCallId: string,
  approved: boolean
): Promise<void> {
  return invoke('tool_approve', { conversationId, toolCallId, approved })
}

// Event Listeners

export function onAgentChunk(
  callback: (event: { conversationId: string; chunk: StreamChunk }) => void
): Promise<UnlistenFn> {
  console.log('[IPC] Setting up agent:chunk listener')
  return listen('agent:chunk', (event) => {
    console.log('[IPC] Received agent:chunk:', event.payload)
    callback(event.payload as { conversationId: string; chunk: StreamChunk })
  })
}

export function onAgentComplete(
  callback: (event: { conversationId: string }) => void
): Promise<UnlistenFn> {
  console.log('[IPC] Setting up agent:complete listener')
  return listen('agent:complete', (event) => {
    console.log('[IPC] Received agent:complete:', event.payload)
    callback(event.payload as { conversationId: string })
  })
}

export function onAgentError(
  callback: (event: { conversationId: string; error: string }) => void
): Promise<UnlistenFn> {
  console.log('[IPC] Setting up agent:error listener')
  return listen('agent:error', (event) => {
    console.log('[IPC] Received agent:error:', event.payload)
    callback(event.payload as { conversationId: string; error: string })
  })
}

// Credential types
export interface CredentialInfo {
  key: string
  isSet: boolean
}

// Credential Commands

export async function credentialSet(key: string, value: string): Promise<void> {
  return invoke('credential_set', { key, value })
}

export async function credentialGet(key: string): Promise<string> {
  return invoke('credential_get', { key })
}

export async function credentialDelete(key: string): Promise<void> {
  return invoke('credential_delete', { key })
}

export async function credentialCheck(key: string): Promise<CredentialInfo> {
  return invoke('credential_check', { key })
}

export async function credentialList(): Promise<CredentialInfo[]> {
  return invoke('credential_list')
}

// Conversation types
export interface ConversationInfo {
  id: string
  agentId: string
  title: string
  created: string
  updated: string
  messageCount: number
}

// Conversation Commands

export async function conversationList(agentId: string): Promise<ConversationInfo[]> {
  return invoke('conversation_list', { agentId })
}

export async function conversationListAll(): Promise<ConversationInfo[]> {
  return invoke('conversation_list_all')
}

export async function conversationCreate(
  agentId: string,
  title?: string
): Promise<string> {
  return invoke('conversation_create', { agentId, title })
}

export async function conversationMessages(conversationId: string): Promise<unknown[]> {
  return invoke('conversation_messages', { conversationId })
}

export async function conversationDelete(conversationId: string): Promise<void> {
  return invoke('conversation_delete', { conversationId })
}

// Task types
export interface TaskSummary {
  id: string
  title: string
  state: string
  currentPhase: string
  progress: number
  createdAt: string
  updatedAt: string
}

export interface PhaseInfo {
  phase: string
  status: string
  conversationId?: string
}

export interface TaskInfo {
  id: string
  title: string
  description: string
  state: string
  currentPhase: string
  phases: PhaseInfo[]
  progress: number
  createdAt: string
  updatedAt: string
}

// Task Commands

export async function taskList(): Promise<TaskSummary[]> {
  return invoke('task_list')
}

export async function taskCreate(
  title: string,
  description?: string
): Promise<TaskInfo> {
  return invoke('task_create', { title, description })
}

export async function taskGet(taskId: string): Promise<TaskInfo> {
  return invoke('task_get', { taskId })
}

export async function taskAdvance(taskId: string): Promise<TaskInfo> {
  return invoke('task_advance', { taskId })
}

export async function taskPhaseMessages(
  taskId: string,
  phase: string
): Promise<unknown[]> {
  return invoke('task_phase_messages', { taskId, phase })
}

export async function taskDelete(taskId: string): Promise<void> {
  return invoke('task_delete', { taskId })
}

// Task Events

export function onTaskPhaseChanged(
  callback: (event: { taskId: string; phase: string }) => void
): Promise<UnlistenFn> {
  return listen('task:phase-changed', (event) => {
    callback(event.payload as { taskId: string; phase: string })
  })
}
