import {
  type RequestContext,
  type Result,
  request,
  post,
  get,
  del,
} from '@reactor/shared';

export interface InvokeOptions {
  body?: unknown;
  headers?: Record<string, string>;
  signal?: AbortSignal;
}

export interface FunctionVersion {
  version: string;
  created_at: string;
  size_bytes: number;
  active: boolean;
}

export interface FunctionLog {
  timestamp: string;
  level: 'debug' | 'info' | 'warn' | 'error';
  message: string;
}

export interface EnvVar {
  name: string;
  value?: string;
  created_at: string;
  updated_at: string;
}

export class FunctionsClient {
  constructor(private ctx: RequestContext) {}

  /**
   * Invoke a function and return JSON response.
   */
  async invoke<T = unknown>(name: string, options?: InvokeOptions): Promise<Result<T>> {
    return request<T>(
      this.ctx,
      `/functions/v1/invoke/${encodeURIComponent(name)}`,
      {
        method: 'POST',
        body: options?.body,
        headers: options?.headers,
        signal: options?.signal,
      }
    );
  }

  /**
   * Invoke a function and stream the response (SSE).
   */
  async *invokeStream(name: string, options?: InvokeOptions): AsyncIterable<string> {
    const fetchFn = this.ctx.fetch ?? globalThis.fetch;
    const response = await fetchFn(
      `${this.ctx.baseUrl}/functions/v1/invoke/${encodeURIComponent(name)}`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Accept': 'text/event-stream',
          ...(this.ctx.projectKey && { 'X-Reactor-Project-Key': this.ctx.projectKey }),
          ...(await this.getAuthHeader()),
          ...options?.headers,
        },
        body: options?.body ? JSON.stringify(options.body) : undefined,
        signal: options?.signal,
      }
    );

    if (!response.ok || !response.body) {
      throw new Error(`Function invocation failed: ${response.status}`);
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() ?? '';

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          yield line.slice(6);
        }
      }
    }
  }

  /**
   * Invoke a function and return raw Response.
   */
  async invokeRaw(name: string, options?: InvokeOptions): Promise<Response> {
    const fetchFn = this.ctx.fetch ?? globalThis.fetch;
    return fetchFn(
      `${this.ctx.baseUrl}/functions/v1/invoke/${encodeURIComponent(name)}`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(this.ctx.projectKey && { 'X-Reactor-Project-Key': this.ctx.projectKey }),
          ...(await this.getAuthHeader()),
          ...options?.headers,
        },
        body: options?.body ? JSON.stringify(options.body) : undefined,
        signal: options?.signal,
      }
    );
  }

  private async getAuthHeader(): Promise<Record<string, string>> {
    if (this.ctx.getAccessToken) {
      const token = await this.ctx.getAccessToken();
      if (token) return { Authorization: `Bearer ${token}` };
    }
    return {};
  }

  /** Admin: Deploy a function */
  async deploy(
    name: string,
    bundle: Blob | ArrayBuffer,
    options?: { version?: string }
  ): Promise<Result<{ version: string }>> {
    const formData = new FormData();
    formData.append('bundle', bundle instanceof Blob ? bundle : new Blob([bundle]));
    if (options?.version) formData.append('version', options.version);

    return request(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/deploy`, {
      method: 'POST',
      body: formData,
    });
  }

  /** Admin: Environment variables */
  get env() {
    return {
      set: async (name: string, vars: Record<string, string>): Promise<Result<void>> =>
        post(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/env`, vars),

      list: async (name: string): Promise<Result<EnvVar[]>> =>
        get(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/env`),

      unset: async (name: string, keys: string[]): Promise<Result<void>> =>
        del(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/env`, { body: { keys } }),
    };
  }

  /** Admin: Logs */
  get logs() {
    return {
      list: async (name: string, options?: { since?: string; limit?: number }): Promise<Result<FunctionLog[]>> => {
        const params = new URLSearchParams();
        if (options?.since) params.set('since', options.since);
        if (options?.limit) params.set('limit', String(options.limit));
        return get(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/logs?${params}`);
      },
    };
  }

  /** Admin: Versions */
  get versions() {
    return {
      list: async (name: string): Promise<Result<FunctionVersion[]>> =>
        get(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/versions`),

      rollback: async (name: string, version: string): Promise<Result<void>> =>
        post(this.ctx, `/functions/v1/admin/${encodeURIComponent(name)}/rollback`, { version }),
    };
  }
}

export function createFunctionsClient(ctx: RequestContext): FunctionsClient {
  return new FunctionsClient(ctx);
}
