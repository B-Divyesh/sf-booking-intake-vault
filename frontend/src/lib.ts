export const PRODUCT_SLUG = 'booking-intake-vault';
export const BILLING_BASE = 'https://api.sociobot.in/api/v1';
export const LICENSE_KEY = `sb_license:${PRODUCT_SLUG}`;
export const VERDICT_KEY = `sb_license_verdict:${PRODUCT_SLUG}`;

export type Verdict = { valid: boolean; reason: string; checkedAt: number; expires_at?: string | null };

export function formatDate(value: string, withTime = false): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(undefined, withTime
    ? { dateStyle: 'medium', timeStyle: 'short' }
    : { dateStyle: 'medium' }).format(date);
}

export function workerSafe<T extends { visibility_snapshot: string }>(responses: T[]): T[] {
  return responses.filter((response) => response.visibility_snapshot === 'worker');
}

export function captureLicense(url: URL, storage: Storage): string | null {
  const license = url.searchParams.get('license')?.trim() || null;
  if (license) storage.setItem(LICENSE_KEY, license);
  return license;
}

export function cachedVerdict(storage: Storage, now = Date.now()): Verdict | null {
  try {
    const parsed = JSON.parse(storage.getItem(VERDICT_KEY) || 'null') as Verdict | null;
    return parsed && now - parsed.checkedAt < 86_400_000 ? parsed : null;
  } catch { return null; }
}
