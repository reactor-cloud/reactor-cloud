import {
  type RequestContext,
  type Result,
  request,
  ok,
  encodeFilterValue,
} from '@reactor/shared';

import type {
  CountMode,
  FilterOperator,
  FilterValue,
  OrderOptions,
  PendingFilter,
  TextSearchOptions,
  UpsertOptions,
} from './types.js';

/**
 * PostgrestFilterBuilder provides methods for building filter queries.
 */
export class PostgrestFilterBuilder<T, ResultType = T[]> {
  protected table: string;
  protected ctx: RequestContext;
  protected selectColumns: string = '*';
  protected filters: PendingFilter[] = [];
  protected orderClauses: string[] = [];
  protected limitValue?: number;
  protected offsetValue?: number;
  protected countMode?: CountMode;
  protected signalValue?: AbortSignal;
  protected customHeaders: Record<string, string> = {};
  protected responseFormat: 'json' | 'csv' = 'json';
  protected explainMode?: { analyze?: boolean; verbose?: boolean; costs?: boolean; buffers?: boolean };
  protected resultModifier?: 'single' | 'maybeSingle';
  protected method: 'GET' | 'POST' | 'PATCH' | 'DELETE' = 'GET';
  protected body?: unknown;

  constructor(ctx: RequestContext, table: string) {
    this.ctx = ctx;
    this.table = table;
  }

  /** Equal to */
  eq<K extends keyof T & string>(column: K, value: T[K]): this {
    this.filters.push({ column, operator: 'eq', value: value as FilterValue, negated: false });
    return this;
  }

  /** Not equal to */
  neq<K extends keyof T & string>(column: K, value: T[K]): this {
    this.filters.push({ column, operator: 'neq', value: value as FilterValue, negated: false });
    return this;
  }

  /** Greater than */
  gt<K extends keyof T & string>(column: K, value: T[K]): this {
    this.filters.push({ column, operator: 'gt', value: value as FilterValue, negated: false });
    return this;
  }

  /** Greater than or equal */
  gte<K extends keyof T & string>(column: K, value: T[K]): this {
    this.filters.push({ column, operator: 'gte', value: value as FilterValue, negated: false });
    return this;
  }

  /** Less than */
  lt<K extends keyof T & string>(column: K, value: T[K]): this {
    this.filters.push({ column, operator: 'lt', value: value as FilterValue, negated: false });
    return this;
  }

  /** Less than or equal */
  lte<K extends keyof T & string>(column: K, value: T[K]): this {
    this.filters.push({ column, operator: 'lte', value: value as FilterValue, negated: false });
    return this;
  }

  /** Pattern match (LIKE) */
  like<K extends keyof T & string>(column: K, pattern: string): this {
    this.filters.push({ column, operator: 'like', value: pattern, negated: false });
    return this;
  }

  /** Case-insensitive pattern match (ILIKE) */
  ilike<K extends keyof T & string>(column: K, pattern: string): this {
    this.filters.push({ column, operator: 'ilike', value: pattern, negated: false });
    return this;
  }

  /** Is NULL or boolean */
  is<K extends keyof T & string>(column: K, value: null | boolean): this {
    this.filters.push({ column, operator: 'is', value, negated: false });
    return this;
  }

  /** In list */
  in<K extends keyof T & string>(column: K, values: T[K][]): this {
    this.filters.push({ column, operator: 'in', value: values as FilterValue, negated: false });
    return this;
  }

  /** Array contains */
  contains<K extends keyof T & string>(column: K, values: unknown[]): this {
    this.filters.push({ column, operator: 'cs', value: values as FilterValue, negated: false });
    return this;
  }

  /** Array contained by */
  containedBy<K extends keyof T & string>(column: K, values: unknown[]): this {
    this.filters.push({ column, operator: 'cd', value: values as FilterValue, negated: false });
    return this;
  }

  /** Array overlaps */
  overlaps<K extends keyof T & string>(column: K, values: unknown[]): this {
    this.filters.push({ column, operator: 'ov', value: values as FilterValue, negated: false });
    return this;
  }

  /** Full-text search */
  textSearch<K extends keyof T & string>(column: K, query: string, options?: TextSearchOptions): this {
    const { type = 'plain', config } = options ?? {};
    const value = config ? `${config}:${type}:${query}` : `${type}:${query}`;
    this.filters.push({ column, operator: 'fts', value, negated: false });
    return this;
  }

  /** Match multiple conditions (shorthand for multiple eq) */
  match(query: Partial<T>): this {
    for (const [column, value] of Object.entries(query)) {
      if (value !== undefined) {
        this.filters.push({ column, operator: 'eq', value: value as FilterValue, negated: false });
      }
    }
    return this;
  }

  /** Negate a filter */
  not<K extends keyof T & string>(column: K, operator: FilterOperator, value: FilterValue): this {
    this.filters.push({ column, operator, value, negated: true });
    return this;
  }

  /** OR condition (raw string format) */
  or(conditions: string, options?: { foreignTable?: string }): this {
    const column = options?.foreignTable ? `${options.foreignTable}.or` : 'or';
    this.filters.push({ column, operator: 'eq', value: `(${conditions})`, negated: false });
    return this;
  }

  /** Generic filter (escape hatch) */
  filter<K extends keyof T & string>(column: K, operator: FilterOperator, value: FilterValue): this {
    this.filters.push({ column, operator, value, negated: false });
    return this;
  }

  /** Order results */
  order<K extends keyof T & string>(column: K, options?: OrderOptions): this {
    const parts = [column as string];
    if (options?.ascending === false) {
      parts.push('desc');
    }
    if (options?.nullsFirst !== undefined) {
      parts.push(options.nullsFirst ? 'nullsfirst' : 'nullslast');
    }
    if (options?.foreignTable) {
      this.orderClauses.push(`${options.foreignTable}(${parts.join('.')})`);
    } else {
      this.orderClauses.push(parts.join('.'));
    }
    return this;
  }

  /** Limit results */
  limit(count: number, options?: { foreignTable?: string }): this {
    if (options?.foreignTable) {
      this.customHeaders[`${options.foreignTable}-limit`] = String(count);
    } else {
      this.limitValue = count;
    }
    return this;
  }

  /** Offset results (for pagination) */
  range(from: number, to: number, options?: { foreignTable?: string }): this {
    if (options?.foreignTable) {
      this.customHeaders[`${options.foreignTable}-offset`] = String(from);
      this.customHeaders[`${options.foreignTable}-limit`] = String(to - from + 1);
    } else {
      this.offsetValue = from;
      this.limitValue = to - from + 1;
    }
    return this;
  }

  /** Provide an AbortSignal */
  abortSignal(signal: AbortSignal): this {
    this.signalValue = signal;
    return this;
  }

  /** Return CSV instead of JSON */
  csv(): PostgrestFilterBuilder<T, string> {
    this.responseFormat = 'csv';
    return this as unknown as PostgrestFilterBuilder<T, string>;
  }

  /** Return query execution plan */
  explain(options?: { analyze?: boolean; verbose?: boolean; costs?: boolean; buffers?: boolean }): this {
    this.explainMode = options ?? {};
    return this;
  }

  /** Override return type */
  returns<R>(): PostgrestFilterBuilder<R, R[]> {
    return this as unknown as PostgrestFilterBuilder<R, R[]>;
  }

  /** Execute and return exactly one row (throws if not exactly one) */
  single(): PostgrestFilterBuilder<T, T> {
    this.resultModifier = 'single';
    return this as unknown as PostgrestFilterBuilder<T, T>;
  }

  /** Execute and return zero or one row */
  maybeSingle(): PostgrestFilterBuilder<T, T | null> {
    this.resultModifier = 'maybeSingle';
    return this as unknown as PostgrestFilterBuilder<T, T | null>;
  }

  protected buildUrl(): string {
    const url = new URL(`/data/v1/${encodeURIComponent(this.table)}`, this.ctx.baseUrl);

    // Select
    url.searchParams.set('select', this.selectColumns);

    // Filters
    for (const filter of this.filters) {
      const prefix = filter.negated ? 'not.' : '';
      const value = filter.operator === 'in' || filter.operator === 'cs' || filter.operator === 'cd' || filter.operator === 'ov'
        ? encodeFilterValue(filter.value)
        : String(filter.value);
      url.searchParams.append(filter.column, `${prefix}${filter.operator}.${value}`);
    }

    // Order
    if (this.orderClauses.length > 0) {
      url.searchParams.set('order', this.orderClauses.join(','));
    }

    // Pagination
    if (this.limitValue !== undefined) {
      url.searchParams.set('limit', String(this.limitValue));
    }
    if (this.offsetValue !== undefined) {
      url.searchParams.set('offset', String(this.offsetValue));
    }

    return url.toString();
  }

  protected buildHeaders(): Record<string, string> {
    const headers: Record<string, string> = { ...this.customHeaders };

    if (this.countMode) {
      headers['Prefer'] = `count=${this.countMode}`;
    }

    if (this.responseFormat === 'csv') {
      headers['Accept'] = 'text/csv';
    }

    if (this.explainMode) {
      const parts = ['explain'];
      if (this.explainMode.analyze) parts.push('analyze');
      if (this.explainMode.verbose) parts.push('verbose');
      if (this.explainMode.costs !== false) parts.push('costs');
      if (this.explainMode.buffers) parts.push('buffers');
      headers['Accept'] = `application/vnd.pgrst.plan+${this.responseFormat === 'csv' ? 'text' : 'json'}`;
    }

    if (this.resultModifier === 'single' || this.resultModifier === 'maybeSingle') {
      headers['Accept'] = 'application/vnd.pgrst.object+json';
    }

    return headers;
  }

  /** Execute the query */
  async then<TResult1 = Result<ResultType>, TResult2 = never>(
    onfulfilled?: ((value: Result<ResultType>) => TResult1 | PromiseLike<TResult1>) | null,
    _onrejected?: ((reason: unknown) => TResult2 | PromiseLike<TResult2>) | null
  ): Promise<TResult1 | TResult2> {
    const result = await this.execute();

    // Handle maybeSingle - empty result should return null, not error
    if (this.resultModifier === 'maybeSingle' && result.error && result.error.statusCode === 406) {
      if (onfulfilled) {
        return onfulfilled(ok(null) as Result<ResultType>);
      }
      return ok(null) as unknown as TResult1;
    }

    if (onfulfilled) {
      return onfulfilled(result as Result<ResultType>);
    }
    return result as unknown as TResult1;
  }

  protected async execute(): Promise<Result<unknown>> {
    const url = this.buildUrl();
    const headers = this.buildHeaders();

    const result = await request<unknown>(this.ctx, url, {
      method: this.method,
      body: this.body,
      headers,
      signal: this.signalValue,
      responseType: this.responseFormat === 'csv' ? 'text' : 'json',
    });

    return result;
  }

  /** Throw on error instead of returning { data, error } */
  async throwOnError(): Promise<ResultType> {
    const result = await this.execute();

    // Handle maybeSingle - empty result should return null, not throw
    if (this.resultModifier === 'maybeSingle' && result.error && result.error.statusCode === 406) {
      return null as ResultType;
    }

    if (result.error) {
      throw result.error;
    }
    return result.data as ResultType;
  }
}

/**
 * Builder for SELECT queries with column selection.
 */
export class PostgrestQueryBuilder<T> extends PostgrestFilterBuilder<T> {
  /** Select specific columns */
  select<Columns extends string = '*'>(
    columns?: Columns,
    options?: { count?: CountMode }
  ): PostgrestFilterBuilder<T> {
    this.selectColumns = columns ?? '*';
    this.countMode = options?.count;
    this.method = 'GET';
    return this;
  }

  /** Insert row(s) */
  insert(values: Partial<T> | Partial<T>[], options?: { count?: CountMode }): PostgrestFilterBuilder<T> {
    this.method = 'POST';
    this.body = values;
    this.countMode = options?.count;
    return this;
  }

  /** Upsert row(s) */
  upsert(values: Partial<T> | Partial<T>[], options?: UpsertOptions): PostgrestFilterBuilder<T> {
    this.method = 'POST';
    this.body = values;
    this.countMode = options?.count;
    this.customHeaders['Prefer'] = `resolution=${options?.ignoreDuplicates ? 'ignore' : 'merge'}-duplicates`;
    if (options?.onConflict) {
      this.customHeaders['Prefer'] += `,on_conflict=${options.onConflict}`;
    }
    return this;
  }

  /** Update row(s) */
  update(values: Partial<T>, options?: { count?: CountMode }): PostgrestFilterBuilder<T> {
    this.method = 'PATCH';
    this.body = values;
    this.countMode = options?.count;
    return this;
  }

  /** Delete row(s) */
  delete(options?: { count?: CountMode }): PostgrestFilterBuilder<T> {
    this.method = 'DELETE';
    this.countMode = options?.count;
    return this;
  }
}
