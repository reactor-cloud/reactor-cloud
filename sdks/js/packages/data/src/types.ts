/**
 * Generic schema for untyped queries.
 */
export type GenericSchema = {
  Tables: Record<string, { Row: Record<string, unknown>; Insert: Record<string, unknown>; Update: Record<string, unknown> }>;
  Views: Record<string, { Row: Record<string, unknown> }>;
  Functions: Record<string, { Args: Record<string, unknown>; Returns: unknown }>;
};

/**
 * Count mode for queries.
 */
export type CountMode = 'exact' | 'planned' | 'estimated';

/**
 * Response format options.
 */
export type ResponseFormat = 'json' | 'csv' | 'geojson';

/**
 * Filter operators supported by reactor-data.
 */
export type FilterOperator =
  | 'eq'
  | 'neq'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'like'
  | 'ilike'
  | 'in'
  | 'is'
  | 'cs'
  | 'cd'
  | 'ov'
  | 'fts';

/**
 * Result modifier for single row queries.
 */
export type ResultModifier = 'single' | 'maybeSingle';

/**
 * Query execution options.
 */
export interface QueryOptions {
  /** AbortSignal for cancellation */
  signal?: AbortSignal;
  /** Count mode */
  count?: CountMode;
  /** Custom headers */
  headers?: Record<string, string>;
  /** Return result as CSV */
  csv?: boolean;
  /** Return query execution plan */
  explain?: boolean | { analyze?: boolean; verbose?: boolean; costs?: boolean; buffers?: boolean };
}

/**
 * Order options.
 */
export interface OrderOptions {
  ascending?: boolean;
  nullsFirst?: boolean;
  foreignTable?: string;
}

/**
 * Upsert options.
 */
export interface UpsertOptions {
  onConflict?: string;
  ignoreDuplicates?: boolean;
  count?: CountMode;
}

/**
 * Query result with optional count.
 */
export interface QueryResult<T> {
  data: T;
  count?: number;
}

/**
 * Full-text search options.
 */
export interface TextSearchOptions {
  type?: 'plain' | 'phrase' | 'websearch';
  config?: string;
}

/**
 * Filter value types.
 */
export type FilterValue = string | number | boolean | null | (string | number | boolean)[];

/**
 * Pending filter to be applied.
 */
export interface PendingFilter {
  column: string;
  operator: FilterOperator;
  value: FilterValue;
  negated: boolean;
}

/**
 * Type helper for selecting specific columns.
 */
export type SelectResult<T, Columns extends string> = Pick<T, Extract<keyof T, Columns>>;
