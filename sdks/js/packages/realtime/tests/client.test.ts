import { describe, it, expect } from 'vitest';
import { RealtimeClient, createRealtimeClient } from '../src/index.js';
import type { RequestContext } from '@reactor/shared';

describe('RealtimeClient (stub)', () => {
  const mockCtx: RequestContext = {
    baseUrl: 'https://api.reactor.cloud',
    projectKey: 'rk_test_123',
  };

  describe('createRealtimeClient()', () => {
    it('should create a RealtimeClient instance', () => {
      const client = createRealtimeClient(mockCtx);
      expect(client).toBeInstanceOf(RealtimeClient);
    });
  });

  describe('channel()', () => {
    it('should throw not implemented error', () => {
      const client = new RealtimeClient(mockCtx);
      expect(() => client.channel('test')).toThrow('@reactor/realtime is not yet implemented');
    });
  });

  describe('removeChannel()', () => {
    it('should throw not implemented error', () => {
      const client = new RealtimeClient(mockCtx);
      const mockChannel = {} as any;
      expect(() => client.removeChannel(mockChannel)).toThrow('@reactor/realtime is not yet implemented');
    });
  });

  describe('removeAllChannels()', () => {
    it('should throw not implemented error', () => {
      const client = new RealtimeClient(mockCtx);
      expect(() => client.removeAllChannels()).toThrow('@reactor/realtime is not yet implemented');
    });
  });
});
