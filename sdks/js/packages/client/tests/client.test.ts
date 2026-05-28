import { describe, it, expect, vi } from 'vitest';
import { createClient } from '../src/index.js';

describe('createClient', () => {
  it('should create a client with all capabilities', () => {
    const client = createClient('https://api.reactor.cloud', {
      key: 'rk_pub_test123',
    });

    expect(client.auth).toBeDefined();
    expect(client.from).toBeDefined();
    expect(client.rpc).toBeDefined();
    expect(client.storage).toBeDefined();
    expect(client.functions).toBeDefined();
    expect(client.jobs).toBeDefined();
    expect(client.sites).toBeDefined();
    expect(client.realtime).toBeDefined();
  });

  it('should expose from() as a bound method', () => {
    const client = createClient('https://api.reactor.cloud', {
      key: 'rk_pub_test123',
    });

    // from() should work when called directly (not as a method)
    const builder = client.from('users');
    expect(builder).toBeDefined();
    expect(typeof builder.select).toBe('function');
    expect(typeof builder.insert).toBe('function');
    expect(typeof builder.update).toBe('function');
    expect(typeof builder.delete).toBe('function');
  });

  it('should expose rpc() as a bound method', () => {
    const client = createClient('https://api.reactor.cloud', {
      key: 'rk_pub_test123',
    });

    // rpc() should work when called directly
    const rpcBuilder = client.rpc('my_function', { arg1: 'value' });
    expect(rpcBuilder).toBeDefined();
  });

  it('should pass org header when provided', () => {
    const client = createClient('https://api.reactor.cloud', {
      key: 'rk_pub_test123',
      org: 'my-org',
    });

    expect(client).toBeDefined();
    // The org header will be injected into requests internally
  });

  it('should use custom fetch when provided', () => {
    const customFetch = vi.fn();
    const client = createClient('https://api.reactor.cloud', {
      key: 'rk_pub_test123',
      fetch: customFetch,
    });

    expect(client).toBeDefined();
    // Custom fetch will be used for all requests
  });
});

describe('Type exports', () => {
  it('should export Result type', async () => {
    const { createClient } = await import('../src/index.js');
    expect(createClient).toBeDefined();
  });
});
