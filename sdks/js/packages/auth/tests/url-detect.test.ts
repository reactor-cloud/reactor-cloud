import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { detectSessionInUrl, cleanUrlAfterDetection, detectAndClean } from '../src/url-detect.js';

describe('url-detect', () => {
  const originalWindow = globalThis.window;

  function mockWindow(url: string) {
    const urlObj = new URL(url);
    (globalThis as any).window = {
      location: {
        href: url,
        hash: urlObj.hash,
      },
      history: {
        replaceState: vi.fn(),
      },
    };
    return (globalThis as any).window;
  }

  beforeEach(() => {
    // Reset window mock before each test
  });

  afterEach(() => {
    if (originalWindow) {
      (globalThis as any).window = originalWindow;
    } else {
      delete (globalThis as any).window;
    }
  });

  describe('detectSessionInUrl', () => {
    it('should detect verification token', () => {
      mockWindow('https://example.com/verify?token=abc123');

      const result = detectSessionInUrl();

      expect(result).toEqual({
        type: 'verify',
        token: 'abc123',
      });
    });

    it('should detect password reset token', () => {
      mockWindow('https://example.com/reset?token=abc123&type=password_reset');

      const result = detectSessionInUrl();

      expect(result).toEqual({
        type: 'password_reset',
        token: 'abc123',
      });
    });

    it('should detect invite token', () => {
      mockWindow('https://example.com/join?invite_token=inv123');

      const result = detectSessionInUrl();

      expect(result).toEqual({
        type: 'invite',
        token: 'inv123',
      });
    });

    it('should detect OAuth tokens in hash', () => {
      mockWindow('https://example.com/callback#access_token=at123&refresh_token=rt456&expires_at=2024-01-01');

      const result = detectSessionInUrl();

      expect(result).toEqual({
        type: 'oauth',
        token: 'at123',
        params: {
          access_token: 'at123',
          refresh_token: 'rt456',
          expires_at: '2024-01-01',
        },
      });
    });

    it('should return null when no tokens', () => {
      mockWindow('https://example.com/');

      const result = detectSessionInUrl();

      expect(result).toBeNull();
    });

    it('should return null in non-browser environment', () => {
      delete (globalThis as any).window;

      const result = detectSessionInUrl();

      expect(result).toBeNull();
    });
  });

  describe('cleanUrlAfterDetection', () => {
    it('should remove token query params', () => {
      const mockWin = mockWindow('https://example.com/verify?token=abc123&other=value');

      cleanUrlAfterDetection();

      expect(mockWin.history.replaceState).toHaveBeenCalledWith(
        null,
        '',
        expect.stringMatching(/example\.com\/verify\?other=value$/)
      );
    });

    it('should remove OAuth hash', () => {
      const mockWin = mockWindow('https://example.com/callback#access_token=at&refresh_token=rt');

      cleanUrlAfterDetection();

      expect(mockWin.history.replaceState).toHaveBeenCalledWith(
        null,
        '',
        'https://example.com/callback'
      );
    });

    it('should not modify URL without tokens', () => {
      const mockWin = mockWindow('https://example.com/?page=1');

      cleanUrlAfterDetection();

      expect(mockWin.history.replaceState).not.toHaveBeenCalled();
    });
  });

  describe('detectAndClean', () => {
    it('should detect and clean by default', () => {
      const mockWin = mockWindow('https://example.com/verify?token=abc123');

      const result = detectAndClean();

      expect(result?.token).toBe('abc123');
      expect(mockWin.history.replaceState).toHaveBeenCalled();
    });

    it('should not clean when disabled', () => {
      const mockWin = mockWindow('https://example.com/verify?token=abc123');

      const result = detectAndClean(false);

      expect(result?.token).toBe('abc123');
      expect(mockWin.history.replaceState).not.toHaveBeenCalled();
    });
  });
});
