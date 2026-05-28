import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import type {
  DiscoveryInfo,
  AppStateSnapshot,
  HealthResponse,
  AgentInfo,
  WaitResponseResult,
  LogsResponse,
  WorkspaceInfo,
} from './types.js';

export interface ClientOptions {
  baseUrl?: string;
  token?: string;
  workspacePath?: string;
}

export class Client {
  private baseUrl: string;
  private token: string;
  private workspacePath: string | null;

  private constructor(baseUrl: string, token: string, workspacePath: string | null) {
    this.baseUrl = baseUrl;
    this.token = token;
    this.workspacePath = workspacePath;
  }

  static async create(opts: ClientOptions = {}): Promise<Client> {
    if (opts.baseUrl && opts.token) {
      return new Client(opts.baseUrl, opts.token, opts.workspacePath ?? null);
    }

    const workspacePath = opts.workspacePath ?? process.cwd();
    const discoveryPath = path.join(workspacePath, '.reactor', 'dev-server.json');

    try {
      const content = await fs.readFile(discoveryPath, 'utf-8');
      const discovery: DiscoveryInfo = JSON.parse(content);
      return new Client(
        `http://127.0.0.1:${discovery.port}`,
        discovery.token,
        workspacePath
      );
    } catch {
      throw new Error(
        `Could not read discovery file at ${discoveryPath}. Is the app running with devserver enabled?`
      );
    }
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${this.token}`,
      ...((options.headers as Record<string, string>) ?? {}),
    };

    const response = await fetch(url, {
      ...options,
      headers,
    });

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Request failed: ${response.status} ${response.statusText} - ${text}`);
    }

    return response.json() as Promise<T>;
  }

  async health(): Promise<HealthResponse> {
    return this.request<HealthResponse>('/health');
  }

  async getState(): Promise<AppStateSnapshot> {
    return this.request<AppStateSnapshot>('/get-state');
  }

  async screenshot(outPath?: string): Promise<{ success: boolean; path?: string; error?: string }> {
    const result = await this.request<{ success: boolean; path?: string; error?: string }>(
      '/screenshot'
    );
    // TODO: If outPath is provided, save the screenshot there
    return result;
  }

  async closeOtherWindows(): Promise<{ closed: number }> {
    return this.request<{ closed: number }>('/windows/close-others', {
      method: 'POST',
    });
  }

  async openWorkspace(workspacePath: string): Promise<{
    success: boolean;
    workspace?: WorkspaceInfo;
    error?: string;
  }> {
    return this.request('/open-workspace', {
      method: 'POST',
      body: JSON.stringify({ path: workspacePath }),
    });
  }

  async listAgents(): Promise<AgentInfo[]> {
    return this.request<AgentInfo[]>('/agents');
  }

  async selectAgent(agentId: string): Promise<{ success: boolean; agentId: string }> {
    return this.request('/select-agent', {
      method: 'POST',
      body: JSON.stringify({ agentId }),
    });
  }

  async newConversation(agentId?: string): Promise<{
    success: boolean;
    conversationId?: string;
    error?: string;
  }> {
    return this.request('/new-conversation', {
      method: 'POST',
      body: JSON.stringify({ agentId }),
    });
  }

  async sendMessage(
    conversationId: string,
    message: string
  ): Promise<{ success: boolean; messageId?: string; error?: string }> {
    return this.request('/send-message', {
      method: 'POST',
      body: JSON.stringify({ conversationId, message }),
    });
  }

  async waitResponse(opts?: {
    timeoutMs?: number;
    onUpdate?: (data: unknown) => void;
  }): Promise<WaitResponseResult> {
    // For now, use simple polling. TODO: Implement SSE streaming
    const startTime = Date.now();
    const timeoutMs = opts?.timeoutMs ?? 120_000;

    // Simple implementation - get current result
    const result = await this.request<WaitResponseResult>('/wait-response');

    return {
      ...result,
      durationMs: Date.now() - startTime,
    };
  }

  async openView(
    viewId: string,
    title?: string,
    documentId?: string
  ): Promise<{ success: boolean; tabId?: string; error?: string }> {
    return this.request('/open-view', {
      method: 'POST',
      body: JSON.stringify({ viewId, title, documentId }),
    });
  }

  async getLogs(opts?: {
    cat?: 'agent' | 'app';
    level?: string;
    limit?: number;
    since?: string;
  }): Promise<LogsResponse> {
    const params = new URLSearchParams();
    if (opts?.cat) params.set('cat', opts.cat);
    if (opts?.level) params.set('level', opts.level);
    if (opts?.limit) params.set('limit', String(opts.limit));
    if (opts?.since) params.set('since', opts.since);

    const query = params.toString();
    return this.request<LogsResponse>(`/logs${query ? `?${query}` : ''}`);
  }

  getWorkspacePath(): string | null {
    return this.workspacePath;
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }
}
