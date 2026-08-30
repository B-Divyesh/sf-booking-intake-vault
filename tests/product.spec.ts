import { expect, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const passphrase = 'correct horse route staple';

test.describe.serial('Private Intake', () => {
  test('sets up the vault and renders an accessible landing page', async ({ page }) => {
    const setup = await page.request.post('/api/setup', { data: {
      business_name: 'Northline Repairs', passphrase, timezone: 'UTC',
      region: 'European Union', deletion_days: 14,
    }});
    expect(setup.ok()).toBeTruthy();
    const consoleErrors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') consoleErrors.push(message.text()); });
    await page.goto('/');
    await expect(page).toHaveTitle(/Private Intake/);
    await expect(page.locator('h1')).toHaveCount(1);
    await expect(page.locator('main')).toBeVisible();
    await expect(page.getByRole('img')).toHaveAttribute('alt', /route/i);
    await expect(page.locator('link[rel="preload"][as="image"]')).toHaveAttribute('fetchpriority', 'high');
    const violations = await new AxeBuilder({ page }).analyze();
    expect(violations.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    expect(consoleErrors).toEqual([]);
  });

  test('returns 200 for every direct client route and exposes its configured build identity', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') consoleErrors.push(message.text()); });
    const health = await page.request.get('/health');
    expect(health.ok()).toBeTruthy();
    expect(await health.json()).toEqual({ status: 'ok', build: 'playwright-test' });

    for (const route of ['/', '/book', '/admin', '/privacy', '/terms']) {
      const response = await page.goto(route);
      expect(response?.status(), `${route} should serve the application shell`).toBe(200);
    }
    const workerDocument = await page.request.get('/worker/nonexistent-token');
    expect(workerDocument.status()).toBe(200);
    expect(consoleErrors).toEqual([]);
  });

  test('has no serious or critical axe violations on the booking form', async ({ page }) => {
    await page.goto('/book');
    const violations = await new AxeBuilder({ page }).analyze();
    expect(violations.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
  });

  test('enforces paid limits and typed validation at the API boundary', async ({ page }) => {
    expect((await page.request.post('/api/login', { data: { passphrase } })).ok()).toBeTruthy();
    const current = await (await page.request.get('/api/form')).json();
    const ninth = { id: 'premium_ninth', label: 'Premium ninth', field_type: 'text', required: false, visibility: 'worker', options: [] };
    const premiumForm = await page.request.put('/api/form', { data: { fields: [...current.fields, ninth] } });
    expect(premiumForm.status()).toBe(402);
    expect((await premiumForm.json()).error).toMatch(/valid Route pass/);

    const duplicateFields = current.fields.map((field: Record<string, unknown>, index: number) => index === 1 ? { ...field, id: current.fields[0].id } : field);
    const duplicate = await page.request.put('/api/form', { data: { fields: duplicateFields } });
    expect(duplicate.status()).toBe(422);
    expect((await duplicate.json()).error).toMatch(/unique ID/);
    expect((await (await page.request.get('/api/form/public')).json()).fields).toHaveLength(8);

    const validValues = {
      client_name: 'API validation client', contact_number: '+1 555 0199',
      service_address: '12 Route Road', appointment_date: '2026-09-02',
      arrival_window: 'Morning · 8–12', job_details: 'Replace valve',
    };
    const badDate = await page.request.post('/api/bookings', { data: { values: { ...validValues, appointment_date: 'not-a-date' }, website: '' } });
    expect(badDate.status()).toBe(422);
    expect((await badDate.json()).error).toMatch(/valid preferred date/);
    const badPhone = await page.request.post('/api/bookings', { data: { values: { ...validValues, contact_number: 'not a phone' }, website: '' } });
    expect(badPhone.status()).toBe(422);
    expect((await badPhone.json()).error).toMatch(/valid contact number/);

    expect((await page.request.post('/api/bookings', { data: { values: validValues, website: '' } })).status()).toBe(201);
    const booking = (await (await page.request.get('/api/bookings')).json()).bookings[0];
    const longLink = await page.request.post(`/api/bookings/${booking.id}/assign`, { data: { worker_name: 'Morgan', expires_hours: 336 } });
    expect(longLink.status()).toBe(402);
  });

  test('submits an intake and server-redacts the worker brief', async ({ page }) => {
    await page.goto('/book');
    await page.getByLabel(/^Client name/).fill('A PRIVATE CLIENT');
    await page.getByLabel(/^Contact number/).fill('+1 555 0199');
    await page.getByLabel(/^Service address/).fill('12 Route Road');
    await page.getByLabel(/^Preferred date/).fill('2026-09-02');
    await page.getByLabel(/^Preferred arrival window/).selectOption({ label: 'Morning · 8–12' });
    await page.getByLabel(/^What needs attention/).fill('Replace valve');
    await page.getByLabel('Access or safety notes').fill('Side gate');
    await page.getByLabel('Billing or account notes').fill('PRIVATE BILLING NOTE');
    await page.getByRole('button', { name: /Send booking request/ }).click();
    await expect(page.getByRole('heading', { name: /Your details reached/ })).toBeVisible();

    await page.goto('/admin');
    await expect(page.getByText(/not a Sociobot account sign-in/)).toBeVisible();
    await page.getByLabel('Passphrase').fill(passphrase);
    await page.getByRole('button', { name: 'Open the vault' }).click();
    await page.getByRole('button', { name: /A PRIVATE CLIENT/ }).click();
    await expect(page.getByText('PRIVATE BILLING NOTE')).toHaveCount(1);
    await expect(page.getByText('Exactly what the worker receives')).toBeVisible();
    await expect(page.locator('.preview-ticket')).not.toContainText('PRIVATE BILLING NOTE');
    await expect(page.locator('.preview-ticket')).not.toContainText('A PRIVATE CLIENT');

    await page.getByLabel('Worker name').fill('Morgan');
    await page.getByRole('button', { name: 'Create expiring link' }).click();
    const workerPath = await page.getByLabel('Worker link').inputValue();
    await page.goto(workerPath);
    await expect(page.getByRole('heading', { name: /Job brief for Morgan/ })).toBeVisible();
    await expect(page.locator('main')).toContainText('12 Route Road');
    await expect(page.locator('main')).not.toContainText('PRIVATE BILLING NOTE');
    await expect(page.locator('main')).not.toContainText('A PRIVATE CLIENT');
    const workerAxe = await new AxeBuilder({ page }).analyze();
    expect(workerAxe.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
  });

  test('covers manager detail, delete dialog, form routing, and Route pass with axe', async ({ page }) => {
    expect((await page.request.post('/api/login', { data: { passphrase } })).ok()).toBeTruthy();
    await page.goto('/admin');
    await page.getByRole('button', { name: /A PRIVATE CLIENT/ }).click();
    await page.getByLabel('Job status').selectOption('complete');
    let result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    await page.getByRole('button', { name: 'Delete this booking' }).click();
    result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    await page.getByRole('button', { name: 'Keep booking' }).click();
    await page.getByRole('button', { name: /Back to arrivals/ }).click();
    await page.getByRole('button', { name: 'Form routing' }).click();
    await page.getByLabel('Answer type').first().selectOption('select');
    result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    await page.getByRole('button', { name: /Route pass/ }).click();
    result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
  });

  test('fits the client form at 390px without horizontal overflow', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/book');
    await expect(page.getByRole('heading', { name: /Tell the team/ })).toBeVisible();
    const dimensions = await page.evaluate(() => ({ scroll: document.documentElement.scrollWidth, client: document.documentElement.clientWidth }));
    expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client);
  });

  test('keeps the configured mobile booking route stable and skips landing artwork', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.addInitScript(() => {
      const target = window as Window & { __privateIntakeCls?: number };
      target.__privateIntakeCls = 0;
      new PerformanceObserver((list) => {
        for (const rawEntry of list.getEntries()) {
          const entry = rawEntry as PerformanceEntry & { hadRecentInput: boolean; value: number };
          if (!entry.hadRecentInput) target.__privateIntakeCls = (target.__privateIntakeCls || 0) + entry.value;
        }
      }).observe({ type: 'layout-shift', buffered: true });
    });
    await page.goto('/book');
    await expect(page.getByRole('heading', { name: /Tell the team/ })).toBeVisible();
    await page.waitForTimeout(500);
    const result = await page.evaluate(() => ({
      cls: (window as Window & { __privateIntakeCls?: number }).__privateIntakeCls || 0,
      resources: performance.getEntriesByType('resource').map((entry) => entry.name),
    }));
    expect(result.cls).toBeLessThan(0.1);
    expect(result.resources.some((url) => url.includes('private-routes-'))).toBe(false);
  });

  test('keeps mobile navigation and legal links at least 44px tall', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    for (const route of ['/', '/privacy', '/terms']) {
      await page.goto(route);
      const undersized = await page.locator('a:visible').evaluateAll((links) => links
        .map((link) => ({ text: (link.textContent || '').trim(), height: link.getBoundingClientRect().height, width: link.getBoundingClientRect().width }))
        .filter((target) => target.height < 44 || target.width < 44));
      expect(undersized, `${route} has undersized links`).toEqual([]);
    }
  });

  test('renders expected unavailable states without console errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto('/worker/not-a-real-ticket');
    await expect(page.getByRole('heading', { name: /brief can’t be opened/ })).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('uses the offline shell without disconnected API console errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto('/book');
    await expect(page.getByRole('heading', { name: /Tell the team/ })).toBeVisible();
    await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller?.scriptURL || '')).toContain('/sw.js');
    const updateState = await page.evaluate(async () => {
      const registration = await navigator.serviceWorker.getRegistration();
      await registration?.update();
      return {
        waiting: Boolean(registration?.waiting),
        caches: await caches.keys(),
      };
    });
    expect(updateState.waiting).toBe(false);
    expect(updateState.caches).toContain('private-intake-shell-v4');
    await page.context().setOffline(true);
    await page.reload();
    await expect(page.getByRole('heading', { name: /booking desk is unavailable/ })).toBeVisible();
    expect(errors.filter((message) => /ERR_INTERNET_DISCONNECTED/i.test(message))).toEqual([]);
    await page.context().setOffline(false);
  });

  test('limits login by first forwarded IP and includes Retry-After on 429', async ({ page }) => {
    const headers = { 'x-forwarded-for': '203.0.113.90, 10.0.0.4' };
    for (let attempt = 1; attempt <= 20; attempt += 1) {
      const response = await page.request.post('/api/login', {
        headers,
        data: { passphrase: 'deliberately invalid passphrase' },
      });
      expect(response.status(), `attempt ${attempt}`).toBe(401);
    }
    const limited = await page.request.post('/api/login', {
      headers,
      data: { passphrase: 'deliberately invalid passphrase' },
    });
    expect(limited.status()).toBe(429);
    expect(Number(limited.headers()['retry-after'])).toBeGreaterThan(0);

    const separateClient = await page.request.post('/api/login', {
      headers: { 'x-forwarded-for': '203.0.113.91, 10.0.0.4' },
      data: { passphrase: 'deliberately invalid passphrase' },
    });
    expect(separateClient.status()).toBe(401);
  });
});
