export interface DiscoveryInfo {
  port: number;
  token: string;
  pid: number;
  version: string;
  startedAt: string;
}

export interface WorkspaceInfo {
  path: string;
  name: string;
  projectId?: string;
}

export interface AgentInfo {
  id: string;
  name: string;
  color: string;
  icon?: string;
}

export interface ConversationInfo {
  id: string;
  agentId: string;
  title?: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

export interface AppStateSnapshot {
  workspace: WorkspaceInfo | null;
  selectedAgentId: string | null;
  activeConversationId: string | null;
  agents: AgentInfo[];
  conversations: ConversationInfo[];
}

export interface HealthResponse {
  status: string;
  version: string;
  uptimeMs: number;
}

export interface ToolCallSummary {
  name: string;
  args: unknown;
  result?: unknown;
  status: string;
  durationMs?: number;
}

export interface WaitResponseResult {
  success: boolean;
  finalText?: string;
  toolSequence: ToolCallSummary[];
  durationMs: number;
  error?: string;
}

export interface TestSetup {
  agent?: string;
  restart?: 'always' | 'if-stale' | 'never';
  workspace?: string;
  model?: {
    provider: string;
    modelId: string;
  };
}

export interface TestAssertion {
  name: string;
  passed: boolean;
  expected?: unknown;
  actual?: unknown;
}

export interface TestLog {
  timestamp: string;
  level: string;
  message: string;
  data?: unknown;
}

export interface TestResult {
  testRunId: string;
  testName: string;
  success: boolean;
  duration: number;
  assertions: TestAssertion[];
  logs: TestLog[];
  screenshots: string[];
  data?: Record<string, unknown>;
  error?: string;
}

export interface TestDefinition {
  name: string;
  description?: string;
  setup?: TestSetup;
  run: (session: TestSession) => Promise<{ data?: Record<string, unknown> }>;
}

export interface TestSession {
  conversationId: string;
  agentId: string;
  model: { provider: string; modelId: string } | null;
  workspacePath: string;
  client: import('./client.js').Client;

  log(msg: string, data?: Record<string, unknown>): void;
  screenshot(name?: string): Promise<string | null>;

  sendAndWait(
    prompt: string,
    opts?: {
      timeoutMs?: number;
      onUpdate?: (messageCount: number) => void;
    }
  ): Promise<WaitResponseResult>;

  assert(name: string, passed: boolean, expected?: unknown, actual?: unknown): void;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  category: string;
  event: string;
  data?: unknown;
}

export interface LogsResponse {
  entries: LogEntry[];
  total: number;
  truncated: boolean;
}
