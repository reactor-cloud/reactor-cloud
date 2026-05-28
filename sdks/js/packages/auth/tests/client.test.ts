import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { AuthClient } from '../src/client.js';
import { type RequestContext, memoryAdapter } from '@reactor/shared';

describe('AuthClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;
  let ctx: RequestContext;
  let storage: ReturnType<typeof memoryAdapter>;

  beforeEach(() => {
    mockFetch = vi.fn();
    storage = memoryAdapter();
    ctx = {
      baseUrl: 'http://localhost:8000',
      projectKey: 'rk_pub_test',
      fetch: mockFetch as unknown as typeof fetch,
    };
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  const mockUser = {
    id: 'user-123',
    email: 'test@example.com',
    email_verified: false,
    metadata: {},
    created_at: '2024-01-01T00:00:00Z',
  };

  // Create a properly formatted JWT for testing (header.payload.signature)
  const createMockToken = () => {
    const header = btoa(JSON.stringify({ alg: 'HS256', typ: 'JWT' }));
    const payload = btoa(JSON.stringify({
      sub: 'user-123',
      email: 'test@example.com',
      exp: Math.floor(Date.now() / 1000) + 3600,
      iat: Math.floor(Date.now() / 1000),
    }));
    return `${header}.${payload}.signature`;
  };

  const mockSession = {
    access_token: createMockToken(),
    refresh_token: 'refresh-token',
    expires_at: new Date(Date.now() + 3600 * 1000).toISOString(),
  };

  function mockResponse<T>(data: T, status = 200) {
    return Promise.resolve({
      ok: status >= 200 && status < 300,
      status,
      statusText: 'OK',
      json: () => Promise.resolve(data),
      text: () => Promise.resolve(JSON.stringify(data)),
    } as Response);
  }

  describe('signUp', () => {
    it('should sign up and store session', async () => {
      mockFetch.mockResolvedValueOnce(
        mockResponse({
          user: mockUser,
          session: mockSession,
        }, 201)
      );

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
      });

      const result = await client.signUp({
        email: 'test@example.com',
        password: 'password123',
      });

      expect(result.error).toBeNull();
      expect(result.data?.user.email).toBe('test@example.com');
      expect(result.data?.session.access_token).toBe(mockSession.access_token);

      // Verify session is stored
      const stored = storage.getItem('reactor.session');
      expect(stored).not.toBeNull();
      expect(JSON.parse(stored!).access_token).toBe(mockSession.access_token);
    });
  });

  describe('signIn', () => {
    it('should sign in and store session', async () => {
      mockFetch.mockResolvedValueOnce(
        mockResponse({
          user: mockUser,
          ...mockSession,
        })
      );

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
      });

      const result = await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      expect(result.error).toBeNull();
      expect(result.data?.user.email).toBe('test@example.com');

      // Verify fetch was called correctly
      expect(mockFetch).toHaveBeenCalledWith(
        'http://localhost:8000/auth/v1/login',
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({
            email: 'test@example.com',
            password: 'password123',
          }),
        })
      );
    });

    it('should handle invalid credentials', async () => {
      mockFetch.mockResolvedValueOnce(
        mockResponse(
          {
            error: {
              code: 'invalid_credentials',
              message: 'Invalid email or password',
            },
          },
          401
        )
      );

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
      });

      const result = await client.signIn({
        email: 'test@example.com',
        password: 'wrong',
      });

      expect(result.error).not.toBeNull();
      expect(result.error?.code).toBe('invalid_credentials');
    });
  });

  describe('signOut', () => {
    it('should clear session on sign out', async () => {
      // First sign in
      mockFetch.mockResolvedValueOnce(
        mockResponse({
          user: mockUser,
          ...mockSession,
        })
      );

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
      });

      await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      // Then sign out
      mockFetch.mockResolvedValueOnce(mockResponse(null));

      await client.signOut();

      // Session should be cleared
      expect(storage.getItem('reactor.session')).toBeNull();
      expect(await client.getSession()).toBeNull();
    });
  });

  describe('getSession', () => {
    it('should return cached session', async () => {
      mockFetch.mockResolvedValueOnce(
        mockResponse({
          user: mockUser,
          ...mockSession,
        })
      );

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
        autoRefresh: false,
      });

      await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      const session = await client.getSession();
      expect(session?.access_token).toBe(mockSession.access_token);
    });
  });

  describe('onAuthStateChange', () => {
    it('should emit SIGNED_IN on login', async () => {
      mockFetch.mockResolvedValueOnce(
        mockResponse({
          user: mockUser,
          ...mockSession,
        })
      );

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
      });

      const callback = vi.fn();
      client.onAuthStateChange(callback);

      await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      expect(callback).toHaveBeenCalledWith('SIGNED_IN', expect.objectContaining({
        access_token: mockSession.access_token,
      }));
    });

    it('should emit SIGNED_OUT on logout', async () => {
      mockFetch
        .mockResolvedValueOnce(
          mockResponse({
            user: mockUser,
            ...mockSession,
          })
        )
        .mockResolvedValueOnce(mockResponse(null));

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
        autoRefresh: false,
      });

      await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      const callback = vi.fn();
      client.onAuthStateChange(callback);

      await client.signOut();

      expect(callback).toHaveBeenCalledWith('SIGNED_OUT', null);
    });

    it('should allow unsubscribe', async () => {
      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
      });

      const callback = vi.fn();
      const { unsubscribe } = client.onAuthStateChange(callback);

      unsubscribe();

      mockFetch.mockResolvedValueOnce(
        mockResponse({
          user: mockUser,
          ...mockSession,
        })
      );

      await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      // Should not be called after unsubscribe
      expect(callback).not.toHaveBeenCalledWith('SIGNED_IN', expect.anything());
    });
  });

  describe('updateUser', () => {
    it('should update user and cache', async () => {
      const updatedUser = { ...mockUser, email: 'new@example.com' };

      mockFetch
        .mockResolvedValueOnce(
          mockResponse({
            user: mockUser,
            ...mockSession,
          })
        )
        .mockResolvedValueOnce(mockResponse(updatedUser));

      const client = new AuthClient(ctx, {
        storage,
        detectSessionInUrl: false,
        autoRefresh: false,
      });

      await client.signIn({
        email: 'test@example.com',
        password: 'password123',
      });

      const result = await client.updateUser({ email: 'new@example.com' });

      expect(result.error).toBeNull();
      expect(result.data?.email).toBe('new@example.com');
    });
  });
});
