import { type RequestContext } from '@reactor/shared';
import { PostgrestQueryBuilder } from './builder.js';
import { RpcBuilder } from './rpc.js';
import type { GenericSchema } from './types.js';

/**
 * Data client for Reactor - PostgREST-style query builder.
 *
 * @example
 * ```ts
 * const client = new DataClient(ctx);
 *
 * // Select with filters
 * const { data, error } = await client
 *   .from('posts')
 *   .select('id, title, author:users(name)')
 *   .eq('published', true)
 *   .order('created_at', { ascending: false })
 *   .limit(10);
 *
 * // Insert
 * const { data } = await client
 *   .from('posts')
 *   .insert({ title: 'Hello', body: 'World' })
 *   .select()
 *   .single();
 *
 * // RPC
 * const { data } = await client.rpc('search', { query: 'rust' });
 * ```
 */
export class DataClient<Schema extends GenericSchema = GenericSchema> {
  constructor(private ctx: RequestContext) {}

  /**
   * Start a query on a table.
   *
   * @param table - The table name
   * @returns A query builder
   */
  from<TableName extends keyof Schema['Tables'] & string>(
    table: TableName
  ): PostgrestQueryBuilder<Schema['Tables'][TableName]['Row']> {
    return new PostgrestQueryBuilder(this.ctx, table);
  }

  /**
   * Call a database function via RPC.
   *
   * @param functionName - The function name
   * @param args - Function arguments
   * @returns RPC builder
   */
  rpc<
    FunctionName extends keyof Schema['Functions'] & string,
    Args extends Schema['Functions'][FunctionName]['Args'],
    Returns = Schema['Functions'][FunctionName]['Returns']
  >(
    functionName: FunctionName,
    args?: Args
  ): RpcBuilder<Args, Returns> {
    const builder = new RpcBuilder<Args, Returns>(this.ctx, functionName);
    if (args) {
      builder.call(args);
    }
    return builder;
  }

  /**
   * Access a schema (for multi-schema support).
   * Currently returns self as we only support public schema.
   */
  schema(_name: string): DataClient<Schema> {
    // TODO: Implement multi-schema support when needed
    return this;
  }
}

/**
 * Create a data client with typed schema.
 */
export function createDataClient<Schema extends GenericSchema = GenericSchema>(
  ctx: RequestContext
): DataClient<Schema> {
  return new DataClient<Schema>(ctx);
}
