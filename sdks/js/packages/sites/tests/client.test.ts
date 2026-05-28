import { describe, it, expect, vi, beforeEach } from 'vitest';
import { SitesClient } from '../src/index.js';
import type { RequestContext } from '@reactor/shared';

describe('SitesClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let mockCtx: RequestContext;
  let sites: SitesClient;

  beforeEach(() => {
    mockFetch = vi.fn();
    mockCtx = {
      baseUrl: 'https://api.reactor.cloud',
      projectKey: 'rk_test_123',
      fetch: mockFetch,
      getAccessToken: async () => 'mock-token',
      defaultRetries: 0,
      defaultTimeout: 60000,
    };
    sites = new SitesClient(mockCtx);
  });

  describe('list()', () => {
    it('should list all sites', async () => {
      const mockSites = [
        { id: 'site-1', name: 'My Site', slug: 'my-site', created_at: '2024-01-01' },
        { id: 'site-2', name: 'Another Site', slug: 'another-site', created_at: '2024-01-02' },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockSites)),
      });

      const result = await sites.list();

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockSites);
    });
  });

  describe('deployments.list()', () => {
    it('should list deployments for a site', async () => {
      const mockDeployments = [
        { id: 'deploy-1', site_id: 'site-1', status: 'ready', created_at: '2024-01-01' },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockDeployments)),
      });

      const result = await sites.deployments.list('site-1');

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockDeployments);
    });
  });

  describe('domains.list()', () => {
    it('should list domains for a site', async () => {
      const mockDomains = [
        {
          id: 'domain-1',
          site_id: 'site-1',
          domain: 'example.com',
          verified: true,
          created_at: '2024-01-01',
        },
      ];
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(mockDomains)),
      });

      const result = await sites.domains.list('site-1');

      expect(result.error).toBeNull();
      expect(result.data).toEqual(mockDomains);
    });
  });
});
