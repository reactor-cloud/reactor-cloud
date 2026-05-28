/**
 * @reactor/data - Data client for Reactor JS SDK
 *
 * PostgREST-style query builder for database operations.
 *
 * @example
 * ```ts
 * import { DataClient } from '@reactor/data';
 * import type { Database } from './database.types';
 *
 * const client = new DataClient<Database['public']>(ctx);
 *
 * // Type-safe queries
 * const { data, error } = await client
 *   .from('posts')
 *   .select('id, title')
 *   .eq('published', true);
 * ```
 */

export { DataClient, createDataClient } from './client.js';
export {
  PostgrestFilterBuilder,
  PostgrestQueryBuilder,
} from './builder.js';
export { RpcBuilder, rpc } from './rpc.js';

export type {
  GenericSchema,
  CountMode,
  ResponseFormat,
  FilterOperator,
  ResultModifier,
  QueryOptions,
  OrderOptions,
  UpsertOptions,
  QueryResult,
  TextSearchOptions,
  FilterValue,
  PendingFilter,
  SelectResult,
} from './types.js';
