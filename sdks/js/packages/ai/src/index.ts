import {
  type RequestContext,
  type Result,
  post,
  get,
  ok,
  err,
  ReactorError,
} from '@reactor/shared';

// ============================================================================
// Types - OpenAI-compatible
// ============================================================================

export interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string | null;
  name?: string;
  tool_call_id?: string;
  tool_calls?: ToolCall[];
}

export interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

export interface Tool {
  type: 'function';
  function: {
    name: string;
    description?: string;
    parameters?: Record<string, unknown>;
  };
}

export interface ChatCompletionRequest {
  model: string;
  messages: Message[];
  temperature?: number;
  top_p?: number;
  max_tokens?: number;
  stream?: boolean;
  tools?: Tool[];
  tool_choice?: 'auto' | 'none' | { type: 'function'; function: { name: string } };
  response_format?: { type: 'text' | 'json_object' };
  stop?: string | string[];
  presence_penalty?: number;
  frequency_penalty?: number;
  user?: string;
}

export interface ChatCompletionChoice {
  index: number;
  message: Message;
  finish_reason: 'stop' | 'length' | 'tool_calls' | 'content_filter' | null;
}

export interface ChatCompletionUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

export interface ChatCompletionResponse {
  id: string;
  object: 'chat.completion';
  created: number;
  model: string;
  choices: ChatCompletionChoice[];
  usage?: ChatCompletionUsage;
}

export interface ChatCompletionChunkDelta {
  role?: 'assistant';
  content?: string | null;
  tool_calls?: Partial<ToolCall>[];
}

export interface ChatCompletionChunkChoice {
  index: number;
  delta: ChatCompletionChunkDelta;
  finish_reason: 'stop' | 'length' | 'tool_calls' | 'content_filter' | null;
}

export interface ChatCompletionChunk {
  id: string;
  object: 'chat.completion.chunk';
  created: number;
  model: string;
  choices: ChatCompletionChunkChoice[];
}

export interface EmbeddingRequest {
  model: string;
  input: string | string[];
  encoding_format?: 'float' | 'base64';
  dimensions?: number;
  user?: string;
}

export interface Embedding {
  index: number;
  object: 'embedding';
  embedding: number[];
}

export interface EmbeddingResponse {
  object: 'list';
  data: Embedding[];
  model: string;
  usage: {
    prompt_tokens: number;
    total_tokens: number;
  };
}

export interface Model {
  id: string;
  object: 'model';
  created: number;
  owned_by: string;
}

export interface ModelsResponse {
  object: 'list';
  data: Model[];
}

// ============================================================================
// SSE Stream Parser
// ============================================================================

export interface ChatCompletionStream extends AsyncIterable<ChatCompletionChunk> {
  [Symbol.asyncIterator](): AsyncIterator<ChatCompletionChunk>;
}

async function* parseSSEStream(
  response: Response
): AsyncGenerator<ChatCompletionChunk, void, unknown> {
  const reader = response.body?.getReader();
  if (!reader) {
    throw new ReactorError('Response body is not readable', 'STREAM_ERROR');
  }

  const decoder = new TextDecoder();
  let buffer = '';

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });

      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith(':')) continue;

        if (trimmed.startsWith('data: ')) {
          const data = trimmed.slice(6);
          if (data === '[DONE]') return;

          try {
            const chunk = JSON.parse(data) as ChatCompletionChunk;
            yield chunk;
          } catch {
            console.warn('Failed to parse SSE chunk:', data);
          }
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

// ============================================================================
// AI Client
// ============================================================================

export class AiClient {
  constructor(private ctx: RequestContext) {}

  /**
   * Create a chat completion (non-streaming).
   */
  async chatCompletion(
    request: Omit<ChatCompletionRequest, 'stream'> & { stream?: false }
  ): Promise<Result<ChatCompletionResponse>> {
    return post<ChatCompletionResponse>(this.ctx, '/ai/v1/chat/completions', {
      ...request,
      stream: false,
    });
  }

  /**
   * Create a streaming chat completion.
   * Returns an async iterable that yields chunks.
   */
  async chatCompletionStream(
    request: Omit<ChatCompletionRequest, 'stream'>
  ): Promise<Result<ChatCompletionStream>> {
    const { baseUrl, headers = {} } = this.ctx;
    const url = `${baseUrl}/ai/v1/chat/completions`;

    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'text/event-stream',
          ...headers,
        },
        body: JSON.stringify({ ...request, stream: true }),
      });

      if (!response.ok) {
        const text = await response.text();
        try {
          const error = JSON.parse(text);
          return err(new ReactorError(error.error?.message || text, error.error?.code || 'API_ERROR'));
        } catch {
          return err(new ReactorError(text, 'API_ERROR'));
        }
      }

      const stream: ChatCompletionStream = {
        [Symbol.asyncIterator]: () => parseSSEStream(response),
      };

      return ok(stream);
    } catch (error) {
      return err(
        error instanceof ReactorError
          ? error
          : new ReactorError(String(error), 'NETWORK_ERROR')
      );
    }
  }

  /**
   * Create embeddings for the given input.
   */
  async embed(request: EmbeddingRequest): Promise<Result<EmbeddingResponse>> {
    return post<EmbeddingResponse>(this.ctx, '/ai/v1/embeddings', request);
  }

  /**
   * List available models.
   */
  async listModels(): Promise<Result<ModelsResponse>> {
    return get<ModelsResponse>(this.ctx, '/ai/v1/models');
  }
}

// ============================================================================
// Factory function
// ============================================================================

export function createAiClient(ctx: RequestContext): AiClient {
  return new AiClient(ctx);
}

// ============================================================================
// Helper functions for building messages
// ============================================================================

export function systemMessage(content: string): Message {
  return { role: 'system', content };
}

export function userMessage(content: string): Message {
  return { role: 'user', content };
}

export function assistantMessage(content: string, toolCalls?: ToolCall[]): Message {
  return { role: 'assistant', content, tool_calls: toolCalls };
}

export function toolMessage(content: string, toolCallId: string): Message {
  return { role: 'tool', content, tool_call_id: toolCallId };
}

// ============================================================================
// Stream helper utilities
// ============================================================================

/**
 * Collect all chunks from a stream into a single response.
 */
export async function collectStream(
  stream: ChatCompletionStream
): Promise<ChatCompletionResponse> {
  let id = '';
  let model = '';
  let created = 0;
  const contents: (string | null)[] = [];
  let finishReason: ChatCompletionChoice['finish_reason'] = null;
  const toolCalls: Map<number, ToolCall> = new Map();

  for await (const chunk of stream) {
    id = chunk.id;
    model = chunk.model;
    created = chunk.created;

    for (const choice of chunk.choices) {
      if (choice.delta.content !== undefined) {
        contents[choice.index] = (contents[choice.index] || '') + (choice.delta.content || '');
      }
      if (choice.finish_reason) {
        finishReason = choice.finish_reason;
      }
      if (choice.delta.tool_calls) {
        for (const tc of choice.delta.tool_calls) {
          if (tc.id) {
            toolCalls.set(toolCalls.size, {
              id: tc.id,
              type: 'function',
              function: {
                name: tc.function?.name || '',
                arguments: tc.function?.arguments || '',
              },
            });
          } else if (toolCalls.size > 0) {
            const lastIdx = toolCalls.size - 1;
            const last = toolCalls.get(lastIdx)!;
            if (tc.function?.arguments) {
              last.function.arguments += tc.function.arguments;
            }
          }
        }
      }
    }
  }

  const message: Message = {
    role: 'assistant',
    content: contents[0] || null,
  };

  if (toolCalls.size > 0) {
    message.tool_calls = Array.from(toolCalls.values());
  }

  return {
    id,
    object: 'chat.completion',
    created,
    model,
    choices: [
      {
        index: 0,
        message,
        finish_reason: finishReason,
      },
    ],
  };
}

/**
 * Extract the text content from a chat completion response.
 */
export function getContent(response: ChatCompletionResponse): string | null {
  return response.choices[0]?.message.content ?? null;
}

/**
 * Check if the response contains tool calls.
 */
export function hasToolCalls(response: ChatCompletionResponse): boolean {
  return (response.choices[0]?.message.tool_calls?.length ?? 0) > 0;
}

/**
 * Get tool calls from the response.
 */
export function getToolCalls(response: ChatCompletionResponse): ToolCall[] {
  return response.choices[0]?.message.tool_calls ?? [];
}
