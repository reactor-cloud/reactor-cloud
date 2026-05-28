import { describe, it, expect } from 'vitest';
import {
  encodeFilterValue,
  buildFilterExpression,
  buildOrderExpression,
  queryParamsToSearchParams,
  parseContentRange,
  parseSelectColumns,
  buildUrl,
} from '../src/query.js';

describe('query', () => {
  describe('encodeFilterValue', () => {
    it('should encode null', () => {
      expect(encodeFilterValue(null)).toBe('null');
    });

    it('should encode booleans', () => {
      expect(encodeFilterValue(true)).toBe('true');
      expect(encodeFilterValue(false)).toBe('false');
    });

    it('should encode numbers', () => {
      expect(encodeFilterValue(42)).toBe('42');
      expect(encodeFilterValue(3.14)).toBe('3.14');
      expect(encodeFilterValue(-1)).toBe('-1');
    });

    it('should encode strings', () => {
      expect(encodeFilterValue('hello')).toBe('hello');
      expect(encodeFilterValue('with spaces')).toBe('with spaces');
    });

    it('should encode arrays', () => {
      expect(encodeFilterValue(['a', 'b', 'c'])).toBe('(a,b,c)');
      expect(encodeFilterValue([1, 2, 3])).toBe('(1,2,3)');
      expect(encodeFilterValue([])).toBe('()');
    });

    it('should escape special characters in array items', () => {
      expect(encodeFilterValue(['a,b', 'c'])).toBe('("a,b",c)');
      expect(encodeFilterValue(['(test)'])).toBe('("(test)")');
    });
  });

  describe('buildFilterExpression', () => {
    it('should build eq filter', () => {
      expect(buildFilterExpression('eq', 'value')).toBe('eq.value');
    });

    it('should build negated filter', () => {
      expect(buildFilterExpression('eq', 'value', true)).toBe('not.eq.value');
    });

    it('should build in filter with array', () => {
      expect(buildFilterExpression('in', ['a', 'b'])).toBe('in.(a,b)');
    });

    it('should build null filter', () => {
      expect(buildFilterExpression('is', null)).toBe('is.null');
    });

    it('should build numeric filter', () => {
      expect(buildFilterExpression('gt', 100)).toBe('gt.100');
    });
  });

  describe('buildOrderExpression', () => {
    it('should build simple ascending order', () => {
      expect(buildOrderExpression('name')).toBe('name');
    });

    it('should build descending order', () => {
      expect(buildOrderExpression('name', { ascending: false })).toBe('name.desc');
    });

    it('should build order with nulls first', () => {
      expect(buildOrderExpression('name', { nullsFirst: true })).toBe('name.nullsfirst');
    });

    it('should build order with nulls last', () => {
      expect(buildOrderExpression('name', { nullsFirst: false })).toBe('name.nullslast');
    });

    it('should build complex order', () => {
      expect(
        buildOrderExpression('created_at', { ascending: false, nullsFirst: false })
      ).toBe('created_at.desc.nullslast');
    });
  });

  describe('queryParamsToSearchParams', () => {
    it('should convert select', () => {
      const params = queryParamsToSearchParams({
        select: 'id,name',
        filters: [],
      });
      expect(params.get('select')).toBe('id,name');
    });

    it('should convert filters', () => {
      const params = queryParamsToSearchParams({
        filters: [
          { column: 'status', expression: 'eq.active' },
          { column: 'age', expression: 'gt.18' },
        ],
      });
      expect(params.get('status')).toBe('eq.active');
      expect(params.get('age')).toBe('gt.18');
    });

    it('should convert order', () => {
      const params = queryParamsToSearchParams({
        filters: [],
        order: ['name.asc', 'created_at.desc'],
      });
      expect(params.get('order')).toBe('name.asc,created_at.desc');
    });

    it('should convert limit and offset', () => {
      const params = queryParamsToSearchParams({
        filters: [],
        limit: 10,
        offset: 20,
      });
      expect(params.get('limit')).toBe('10');
      expect(params.get('offset')).toBe('20');
    });
  });

  describe('buildUrl', () => {
    it('should build URL with path', () => {
      expect(buildUrl('https://api.example.com', '/data/v1/users')).toBe(
        'https://api.example.com/data/v1/users'
      );
    });

    it('should build URL with query params', () => {
      const url = buildUrl('https://api.example.com', '/data/v1/users', {
        select: 'id,name',
        filters: [{ column: 'active', expression: 'eq.true' }],
        limit: 10,
      });
      expect(url).toContain('select=id%2Cname');
      expect(url).toContain('active=eq.true');
      expect(url).toContain('limit=10');
    });
  });

  describe('parseContentRange', () => {
    it('should parse standard range', () => {
      expect(parseContentRange('0-24/1234')).toEqual({
        from: 0,
        to: 24,
        total: 1234,
      });
    });

    it('should parse range with unknown total', () => {
      expect(parseContentRange('0-24/*')).toEqual({
        from: 0,
        to: 24,
        total: undefined,
      });
    });

    it('should parse count only', () => {
      expect(parseContentRange('*/1234')).toEqual({
        from: undefined,
        to: undefined,
        total: 1234,
      });
    });

    it('should return null for invalid header', () => {
      expect(parseContentRange(null)).toBeNull();
      expect(parseContentRange('')).toBeNull();
      expect(parseContentRange('invalid')).toBeNull();
    });
  });

  describe('parseSelectColumns', () => {
    it('should parse simple columns', () => {
      expect(parseSelectColumns('id, name, email')).toEqual(['id', 'name', 'email']);
    });

    it('should parse embedded relations', () => {
      expect(parseSelectColumns('id, author:users(name)')).toEqual([
        'id',
        'author:users(name)',
      ]);
    });

    it('should parse nested relations', () => {
      expect(parseSelectColumns('id, comments(id, author:users(name))')).toEqual([
        'id',
        'comments(id, author:users(name))',
      ]);
    });

    it('should handle wildcard', () => {
      expect(parseSelectColumns('*')).toEqual(['*']);
    });

    it('should handle empty string', () => {
      expect(parseSelectColumns('')).toEqual([]);
    });
  });
});
