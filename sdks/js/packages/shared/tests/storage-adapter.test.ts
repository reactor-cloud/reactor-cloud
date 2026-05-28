import { describe, it, expect } from 'vitest';
import { memoryAdapter } from '../src/storage-adapter.js';

describe('storage-adapter', () => {
  describe('memoryAdapter', () => {
    it('should store and retrieve items', () => {
      const adapter = memoryAdapter();

      adapter.setItem('key', 'value');
      expect(adapter.getItem('key')).toBe('value');
    });

    it('should return null for non-existent keys', () => {
      const adapter = memoryAdapter();

      expect(adapter.getItem('nonexistent')).toBeNull();
    });

    it('should remove items', () => {
      const adapter = memoryAdapter();

      adapter.setItem('key', 'value');
      adapter.removeItem('key');
      expect(adapter.getItem('key')).toBeNull();
    });

    it('should handle JSON data', () => {
      const adapter = memoryAdapter();
      const data = { user: { id: 1, name: 'test' } };

      adapter.setItem('session', JSON.stringify(data));
      const retrieved = JSON.parse(adapter.getItem('session')!);

      expect(retrieved).toEqual(data);
    });

    it('should isolate separate instances', () => {
      const adapter1 = memoryAdapter();
      const adapter2 = memoryAdapter();

      adapter1.setItem('key', 'value1');
      adapter2.setItem('key', 'value2');

      expect(adapter1.getItem('key')).toBe('value1');
      expect(adapter2.getItem('key')).toBe('value2');
    });
  });
});
