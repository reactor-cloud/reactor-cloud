/**
 * Query string utilities for building PostgREST-compatible URLs.
 */

/**
 * Filter operator types matching reactor-data dialect.
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
 * Primitive value types for filters.
 */
export type FilterValue =
  | string
  | number
  | boolean
  | null
  | (string | number | boolean)[];

/**
 * Encode a value for use in a filter expression.
 */
export function encodeFilterValue(value: FilterValue): string {
  if (value === null) {
    return 'null';
  }
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  if (typeof value === 'number') {
    return String(value);
  }
  if (Array.isArray(value)) {
    const encoded = value.map((v) => {
      if (v === null) return 'null';
      if (typeof v === 'string') {
        // Escape special characters in list items
        return v.includes(',') || v.includes('(') || v.includes(')')
          ? `"${v.replace(/"/g, '\\"')}"`
          : v;
      }
      return String(v);
    });
    return `(${encoded.join(',')})`;
  }
  return value;
}

/**
 * Build a filter expression in PostgREST format.
 */
export function buildFilterExpression(
  op: FilterOperator,
  value: FilterValue,
  negated: boolean = false
): string {
  const encoded = encodeFilterValue(value);
  const prefix = negated ? 'not.' : '';
  return `${prefix}${op}.${encoded}`;
}

/**
 * Order direction.
 */
export type OrderDirection = 'asc' | 'desc';

/**
 * Order nulls position.
 */
export type OrderNulls = 'nullsfirst' | 'nullslast';

/**
 * Build an order expression.
 */
export function buildOrderExpression(
  column: string,
  options?: {
    ascending?: boolean;
    nullsFirst?: boolean;
  }
): string {
  const parts = [column];

  if (options?.ascending === false) {
    parts.push('desc');
  }

  if (options?.nullsFirst !== undefined) {
    parts.push(options.nullsFirst ? 'nullsfirst' : 'nullslast');
  }

  return parts.join('.');
}

/**
 * Parameters collected by the query builder.
 */
export interface QueryParams {
  select?: string;
  filters: Array<{ column: string; expression: string }>;
  order?: string[];
  limit?: number;
  offset?: number;
  count?: 'exact' | 'planned' | 'estimated';
}

/**
 * Convert QueryParams to URLSearchParams.
 */
export function queryParamsToSearchParams(params: QueryParams): URLSearchParams {
  const searchParams = new URLSearchParams();

  if (params.select) {
    searchParams.set('select', params.select);
  }

  for (const filter of params.filters) {
    searchParams.append(filter.column, filter.expression);
  }

  if (params.order && params.order.length > 0) {
    searchParams.set('order', params.order.join(','));
  }

  if (params.limit !== undefined) {
    searchParams.set('limit', String(params.limit));
  }

  if (params.offset !== undefined) {
    searchParams.set('offset', String(params.offset));
  }

  return searchParams;
}

/**
 * Build a full URL with query parameters.
 */
export function buildUrl(baseUrl: string, path: string, params?: QueryParams): string {
  const url = new URL(path, baseUrl);

  if (params) {
    const searchParams = queryParamsToSearchParams(params);
    searchParams.forEach((value, key) => {
      url.searchParams.append(key, value);
    });
  }

  return url.toString();
}

/**
 * Parse the Content-Range header for count information.
 * Format: "0-24/1234" or star/1234
 */
export function parseContentRange(header: string | null): {
  from?: number;
  to?: number;
  total?: number;
} | null {
  if (!header) return null;

  const match = header.match(/^(\d+|\*)-?(\d+)?\/(\d+|\*)$/);
  if (!match) return null;

  const [, fromStr, toStr, totalStr] = match;

  return {
    from: fromStr && fromStr !== '*' ? parseInt(fromStr, 10) : undefined,
    to: toStr ? parseInt(toStr, 10) : undefined,
    total: totalStr && totalStr !== '*' ? parseInt(totalStr, 10) : undefined,
  };
}

/**
 * Parse a select string and extract column names.
 * Handles embedded relations like "author:users(name)".
 */
export function parseSelectColumns(select: string): string[] {
  const columns: string[] = [];
  let depth = 0;
  let current = '';

  for (const char of select) {
    if (char === '(') {
      depth++;
      current += char;
    } else if (char === ')') {
      depth--;
      current += char;
    } else if (char === ',' && depth === 0) {
      const trimmed = current.trim();
      if (trimmed) columns.push(trimmed);
      current = '';
    } else {
      current += char;
    }
  }

  const trimmed = current.trim();
  if (trimmed) columns.push(trimmed);

  return columns;
}

/**
 * Encode a value for embedding in a URL path segment.
 */
export function encodePathSegment(value: string): string {
  return encodeURIComponent(value).replace(/%2F/g, '/');
}
