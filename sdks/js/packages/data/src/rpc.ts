import {
  type RequestContext,
  type Result,
  post,
} from '@reactor/shared';

/**
 * Call a database function via RPC.
 */
export async function rpc<Args extends Record<string, unknown>, Returns>(
  ctx: RequestContext,
  functionName: string,
  args: Args,
  options?: { signal?: AbortSignal; headers?: Record<string, string> }
): Promise<Result<Returns>> {
  return post<Returns>(
    ctx,
    `/data/v1/rpc/${encodeURIComponent(functionName)}`,
    args,
    {
      signal: options?.signal,
      headers: options?.headers,
    }
  );
}

/**
 * RPC builder for type-safe function calls.
 */
export class RpcBuilder<Args extends Record<string, unknown>, Returns> {
  private args?: Args;
  private signal?: AbortSignal;
  private customHeaders: Record<string, string> = {};

  constructor(
    private ctx: RequestContext,
    private functionName: string
  ) {}

  /** Set function arguments */
  call(args: Args): this {
    this.args = args;
    return this;
  }

  /** Provide an AbortSignal */
  abortSignal(signal: AbortSignal): this {
    this.signal = signal;
    return this;
  }

  /** Set custom headers */
  headers(headers: Record<string, string>): this {
    this.customHeaders = { ...this.customHeaders, ...headers };
    return this;
  }

  /** Execute the RPC call */
  async then<TResult1 = Result<Returns>, TResult2 = never>(
    onfulfilled?: ((value: Result<Returns>) => TResult1 | PromiseLike<TResult1>) | null,
    _onrejected?: ((reason: unknown) => TResult2 | PromiseLike<TResult2>) | null
  ): Promise<TResult1 | TResult2> {
    const result = await rpc<Args, Returns>(
      this.ctx,
      this.functionName,
      this.args ?? ({} as Args),
      {
        signal: this.signal,
        headers: this.customHeaders,
      }
    );

    if (onfulfilled) {
      return onfulfilled(result);
    }
    return result as unknown as TResult1;
  }

  /** Throw on error */
  async throwOnError(): Promise<Returns> {
    const result = await rpc<Args, Returns>(
      this.ctx,
      this.functionName,
      this.args ?? ({} as Args),
      {
        signal: this.signal,
        headers: this.customHeaders,
      }
    );

    if (result.error) {
      throw result.error;
    }
    return result.data;
  }
}
