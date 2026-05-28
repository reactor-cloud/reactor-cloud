import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PostgrestQueryBuilder } from '../src/builder.js';
import { type RequestContext } from '@reactor/shared';

describe('PostgrestQueryBuilder', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let ctx: RequestContext;

  beforeEach(() => {
    mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve([]),
      text: () => Promise.resolve('[]'),
    } as Response);

    ctx = {
      baseUrl: 'http://localhost:8000',
      fetch: mockFetch as unknown as typeof fetch,
    };
  });

  describe('select', () => {
    it('should build select query with default columns', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string }>(ctx, 'users');
      await builder.select();

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/data/v1/users?select=*'),
        expect.anything()
      );
    });

    it('should build select query with specific columns', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string }>(ctx, 'users');
      await builder.select('id, name');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('select=id%2C+name'),
        expect.anything()
      );
    });
  });

  describe('filters', () => {
    it('should add eq filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; status: string }>(ctx, 'users');
      await builder.select().eq('status', 'active');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('status=eq.active'),
        expect.anything()
      );
    });

    it('should add neq filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; status: string }>(ctx, 'users');
      await builder.select().neq('status', 'deleted');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('status=neq.deleted'),
        expect.anything()
      );
    });

    it('should add gt filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; age: number }>(ctx, 'users');
      await builder.select().gt('age', 18);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('age=gt.18'),
        expect.anything()
      );
    });

    it('should add gte filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; age: number }>(ctx, 'users');
      await builder.select().gte('age', 21);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('age=gte.21'),
        expect.anything()
      );
    });

    it('should add lt filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; age: number }>(ctx, 'users');
      await builder.select().lt('age', 65);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('age=lt.65'),
        expect.anything()
      );
    });

    it('should add lte filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; age: number }>(ctx, 'users');
      await builder.select().lte('age', 64);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('age=lte.64'),
        expect.anything()
      );
    });

    it('should add like filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string }>(ctx, 'users');
      await builder.select().like('name', '%john%');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('name=like.%25john%25'),
        expect.anything()
      );
    });

    it('should add ilike filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string }>(ctx, 'users');
      await builder.select().ilike('name', '%John%');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('name=ilike.%25John%25'),
        expect.anything()
      );
    });

    it('should add is null filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; deleted_at: string | null }>(ctx, 'users');
      await builder.select().is('deleted_at', null);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('deleted_at=is.null'),
        expect.anything()
      );
    });

    it('should add in filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; status: string }>(ctx, 'users');
      await builder.select().in('status', ['active', 'pending']);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('status=in.'),
        expect.anything()
      );
    });

    it('should add not filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; status: string }>(ctx, 'users');
      await builder.select().not('status', 'eq', 'deleted');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('status=not.eq.deleted'),
        expect.anything()
      );
    });

    it('should add match filter', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; status: string; type: string }>(ctx, 'users');
      await builder.select().match({ status: 'active', type: 'admin' });

      const url = mockFetch.mock.calls[0][0] as string;
      expect(url).toContain('status=eq.active');
      expect(url).toContain('type=eq.admin');
    });
  });

  describe('order', () => {
    it('should add order clause', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; created_at: string }>(ctx, 'users');
      await builder.select().order('created_at');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('order=created_at'),
        expect.anything()
      );
    });

    it('should add descending order', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; created_at: string }>(ctx, 'users');
      await builder.select().order('created_at', { ascending: false });

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('order=created_at.desc'),
        expect.anything()
      );
    });

    it('should add order with nulls first', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string | null }>(ctx, 'users');
      await builder.select().order('name', { nullsFirst: true });

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('order=name.nullsfirst'),
        expect.anything()
      );
    });
  });

  describe('pagination', () => {
    it('should add limit', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string }>(ctx, 'users');
      await builder.select().limit(10);

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('limit=10'),
        expect.anything()
      );
    });

    it('should add range (offset + limit)', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string }>(ctx, 'users');
      await builder.select().range(10, 19);

      const url = mockFetch.mock.calls[0][0] as string;
      expect(url).toContain('offset=10');
      expect(url).toContain('limit=10');
    });
  });

  describe('modifiers', () => {
    it('should set single modifier', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string }>(ctx, 'users');
      await builder.select().eq('id', '123').single();

      expect(mockFetch).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({
          headers: expect.objectContaining({
            Accept: 'application/vnd.pgrst.object+json',
          }),
        })
      );
    });
  });

  describe('mutations', () => {
    it('should build insert query', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string }>(ctx, 'users');
      await builder.insert({ name: 'John' });

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/data/v1/users'),
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({ name: 'John' }),
        })
      );
    });

    it('should build update query', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string; name: string }>(ctx, 'users');
      await builder.update({ name: 'Jane' }).eq('id', '123');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/data/v1/users'),
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ name: 'Jane' }),
        })
      );
    });

    it('should build delete query', async () => {
      const builder = new PostgrestQueryBuilder<{ id: string }>(ctx, 'users');
      await builder.delete().eq('id', '123');

      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/data/v1/users'),
        expect.objectContaining({
          method: 'DELETE',
        })
      );
    });
  });
});
