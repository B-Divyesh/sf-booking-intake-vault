import { describe, expect, it } from 'vitest';
import { cachedVerdict, captureLicense, LICENSE_KEY, VERDICT_KEY, workerSafe } from './lib';

class MemoryStorage implements Storage {
  data = new Map<string, string>();
  get length() { return this.data.size; }
  clear() { this.data.clear(); }
  getItem(key: string) { return this.data.get(key) ?? null; }
  key(index: number) { return [...this.data.keys()][index] ?? null; }
  removeItem(key: string) { this.data.delete(key); }
  setItem(key: string, value: string) { this.data.set(key, value); }
}

describe('privacy and licensing helpers', () => {
  it('removes manager-only fields from a worker-safe collection', () => {
    const values = workerSafe([
      { visibility_snapshot: 'worker', value: 'Boiler room' },
      { visibility_snapshot: 'admin', value: 'Card ending 1234' },
    ]);
    expect(values).toEqual([{ visibility_snapshot: 'worker', value: 'Boiler room' }]);
  });

  it('captures a returned license under the product-scoped key', () => {
    const storage = new MemoryStorage();
    expect(captureLicense(new URL('https://example.test/?license=abc123'), storage)).toBe('abc123');
    expect(storage.getItem(LICENSE_KEY)).toBe('abc123');
  });

  it('uses a verdict for at most one day', () => {
    const storage = new MemoryStorage();
    storage.setItem(VERDICT_KEY, JSON.stringify({ valid: true, reason: 'ok', checkedAt: 1000 }));
    expect(cachedVerdict(storage, 2000)?.valid).toBe(true);
    expect(cachedVerdict(storage, 90_000_000)).toBeNull();
  });
});
