import { expect, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test.describe.serial('Private Intake', () => {
  test('@claim:zero-config renders a ready, accessible landing and booking form with only PORT', async ({ page }) => {
    const session = await (await page.request.get('/api/session')).json();
    expect(session).toMatchObject({ configured: true, authenticated: true, identity_provider: 'Sociobot Microsoft Entra External ID' });
    const publicForm = await (await page.request.get('/api/form/public')).json();
    expect(publicForm.available).toBe(true);
    expect(publicForm.fields).toHaveLength(8);
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

  test('@claim:build-identity returns every direct route and its build identity', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') consoleErrors.push(message.text()); });
    const health = await page.request.get('/health');
    expect(health.ok()).toBeTruthy();
    expect(await health.json()).toEqual({ status: 'ok', build: 'playwright-test' });

    for (const route of ['/', '/demo', '/book', '/admin', '/privacy', '/terms']) {
      const response = await page.goto(route);
      expect(response?.status(), `${route} should serve the application shell`).toBe(200);
    }
    const workerDocument = await page.request.get('/worker/nonexistent-token');
    expect(workerDocument.status()).toBe(200);
    expect(consoleErrors).toEqual([]);
  });

  test('@claim:route-metadata serves discovery metadata, a designed 404, and moves focus', async ({ page }) => {
    expect((await page.request.get('/robots.txt')).status()).toBe(200);
    expect(await (await page.request.get('/sitemap.xml')).text()).toContain('/demo');
    const missing = await page.goto('/not-a-real-page');
    expect(missing?.status()).toBe(404);
    await expect(page).toHaveTitle('Page not found — Private Intake');
    await expect(page.getByRole('heading', { name: 'This page does not exist' })).toBeVisible();
    await page.goto('/');
    await page.getByRole('link', { name: 'Demo', exact: true }).click();
    await expect(page).toHaveURL(/\/demo$/);
    await expect(page).toHaveTitle('Demo — Private Intake');
    await expect(page.locator('main h1')).toBeFocused();
    await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', 'https://booking-intake-vault.sociobot.in/demo');
    await expect(page.locator('meta[property="og:image"]')).toHaveAttribute('content', /private-routes-social\.webp/);
    await expect(page.getByText('Built by Param Factory')).toBeVisible();
  });

  test('@claim:demo-isolation opens and resets an isolated sample without changing real bookings', async ({ page }) => {
    const before = (await (await page.request.get('/api/bookings')).json()).bookings.length;
    await page.goto('/');
    await page.getByRole('link', { name: /Try it with sample data/ }).click();
    await expect(page.getByText('Demo — sample data, nothing is saved')).toBeVisible();
    await expect(page.getByTestId('demo-manager-record')).toContainText('Warranty account NW-204');
    await expect(page.getByTestId('demo-worker-brief')).toContainText('18 Juniper Lane');
    await expect(page.getByTestId('demo-worker-brief')).not.toContainText('Nadia Patel');
    await expect(page.getByTestId('demo-worker-brief')).not.toContainText('Warranty account NW-204');
    const firstId = await page.evaluate(() => sessionStorage.getItem('demo:private-intake:id'));
    await page.getByRole('button', { name: 'Reset demo' }).first().click();
    await expect.poll(() => page.evaluate(() => sessionStorage.getItem('demo:private-intake:id'))).not.toBe(firstId);
    expect(Object.keys(await page.evaluate(() => ({ ...localStorage })))).not.toContain('demo:private-intake:id');
    const after = (await (await page.request.get('/api/bookings')).json()).bookings.length;
    expect(after).toBe(before);
  });

  test('@claim:no-trackers keeps the complete demo flow on the product origin', async ({ page }) => {
    const origins = new Set<string>();
    page.on('request', (request) => origins.add(new URL(request.url()).origin));
    await page.goto('/demo');
    await expect(page.getByRole('heading', { name: /See a private booking/ })).toBeVisible();
    await page.getByRole('button', { name: 'Reset demo' }).first().click();
    await expect(page.getByText('Nadia Patel', { exact: true })).toBeVisible();
    expect([...origins]).toEqual(['http://127.0.0.1:8091']);
  });

  test('@claim:entra-manager uses only Sociobot Entra and rejects another identity', async ({ browser, page }) => {
    expect((await page.request.get('/api/session')).status()).toBe(200);
    const denied = await page.request.get('/api/form', { headers: { 'x-test-oid': 'playwright-sociobot-entra-owner-other' } });
    expect(denied.status()).toBe(403);

    const context = await browser.newContext({ extraHTTPHeaders: {} });
    const signedOut = await context.newPage();
    await signedOut.goto('/admin');
    await expect(signedOut.getByText('Sociobot Microsoft Entra External ID')).toBeVisible();
    await signedOut.route('https://sociobotcustomers.ciamlogin.com/**', (route) => route.abort());
    const providerRequest = signedOut.waitForRequest((request) => request.url().startsWith('https://sociobotcustomers.ciamlogin.com/'));
    await signedOut.getByRole('button', { name: 'Sign in with Sociobot' }).click();
    expect((await providerRequest).url()).toContain('sociobotcustomers.ciamlogin.com');
    await context.close();
  });

  test('has no serious or critical axe violations on the booking form', async ({ page }) => {
    await page.goto('/book');
    const violations = await new AxeBuilder({ page }).analyze();
    expect(violations.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
  });

  test('@claim:paid-boundaries enforces paid limits and typed validation at the API boundary', async ({ page }) => {
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
    await page.goto('/admin');
    await page.getByRole('button', { name: /Route pass/ }).click();
    await expect(page.getByText('$29')).toBeVisible();
    await expect(page.getByRole('link', { name: /Buy Route pass/ })).toHaveAttribute('href', 'https://api.sociobot.in/api/v1/products/booking-intake-vault/checkout');
  });

  test('schedules every public booking for its configured deletion date', async ({ page }) => {
    const createdAt = Date.now();
    const response = await page.request.post('/api/bookings', { data: { values: {
      client_name: 'Retention check', contact_number: '+1 555 0144',
      service_address: '20 Archive Road', appointment_date: '2026-09-03',
      arrival_window: 'Afternoon · 12–5', job_details: 'Inspect stop valve',
    }, website: '' } });
    expect(response.status()).toBe(201);
    const deletion = new Date((await response.json()).delete_at).valueOf();
    expect(deletion - createdAt).toBeGreaterThan(29.9 * 86_400_000);
    expect(deletion - createdAt).toBeLessThan(30.1 * 86_400_000);
  });

  test('@claim:csv-export neutralizes formula-leading public values in the CSV export', async ({ page }) => {
    const formula = '=HYPERLINK("https://example.invalid","QA")';
    const created = await page.request.post('/api/bookings', { data: { values: {
      client_name: formula, contact_number: '+1 555 0145', service_address: '22 Safe Road',
      appointment_date: '2026-09-05', arrival_window: 'Morning · 8–12', job_details: 'CSV safety check',
    }, website: '' } });
    expect(created.status()).toBe(201);
    const exportResponse = await page.request.get('/api/bookings/export.csv');
    expect(exportResponse.status()).toBe(200);
    const csv = await exportResponse.text();
    expect(csv).toContain(`"'=HYPERLINK(""https://example.invalid"",""QA"")"`);
    expect(csv).not.toContain(`,"=HYPERLINK`);
  });

  test('@claim:server-field-redaction @claim:worker-link-expiry submits an intake and server-redacts the worker brief', async ({ page }) => {
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
    const submitted = (await (await page.request.get('/api/bookings')).json()).bookings.find((booking: { summary: string }) => booking.summary === 'A PRIVATE CLIENT');
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
    expect((await page.request.delete(`/api/bookings/${submitted.id}`)).status()).toBe(200);
    expect((await (await page.request.get(`/api${new URL(workerPath).pathname}`)).json()).available).toBe(false);
  });

  test('covers manager detail, delete dialog, form routing, and Route pass with axe', async ({ page }) => {
    await page.request.post('/api/bookings', { data: { values: {
      client_name: 'A PRIVATE CLIENT', contact_number: '+1 555 0199', service_address: '12 Route Road',
      appointment_date: '2026-09-02', arrival_window: 'Morning · 8–12', job_details: 'Replace valve',
      billing_context: 'PRIVATE BILLING NOTE',
    }, website: '' } });
    await page.goto('/admin');
    await page.getByRole('button', { name: /A PRIVATE CLIENT/ }).click();
    await page.getByLabel('Job status').selectOption('complete');
    let result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    await page.getByRole('button', { name: 'Delete this booking' }).click();
    result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    await page.getByRole('button', { name: 'Keep booking' }).click();
    await page.getByRole('button', { name: /Back to bookings/ }).click();
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

  test('keeps the unavailable mobile booking state below the CLS budget', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.route('**/api/form/public', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ available: false, error: 'Scheduled maintenance.' }) }));
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
    await expect(page.getByRole('heading', { name: 'The booking desk is unavailable' })).toBeVisible();
    await page.waitForTimeout(500);
    expect(await page.evaluate(() => (window as Window & { __privateIntakeCls?: number }).__privateIntakeCls || 0)).toBeLessThan(0.1);
  });

  test('@claim:response-policy keeps security headers on body-limit responses', async ({ page }) => {
    const response = await page.request.post('/api/bookings', { data: { values: { oversized: 'x'.repeat(70_000) }, website: '' } });
    expect(response.status()).toBe(413);
    const headers = response.headers();
    expect(headers['content-security-policy']).toContain("frame-ancestors 'none'");
    expect(headers['strict-transport-security']).toContain('max-age=31536000');
    expect(headers['x-content-type-options']).toBe('nosniff');
    expect(headers['cache-control']).toBe('no-store');
  });

  test('@claim:mobile-keyboard keeps 390px links usable and exposes keyboard focus', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    for (const route of ['/', '/privacy', '/terms']) {
      await page.goto(route);
      const undersized = await page.locator('a:visible').evaluateAll((links) => links
        .map((link) => ({ text: (link.textContent || '').trim(), height: link.getBoundingClientRect().height, width: link.getBoundingClientRect().width }))
        .filter((target) => target.height < 44 || target.width < 44));
      expect(undersized, `${route} has undersized links`).toEqual([]);
    }
    await page.goto('/');
    await page.keyboard.press('Tab');
    await expect(page.getByRole('link', { name: 'Skip to main content' })).toBeFocused();
    await expect(page.getByRole('link', { name: 'Skip to main content' })).toHaveCSS('outline-style', 'solid');
  });

  test('renders expected unavailable states without console errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto('/worker/not-a-real-ticket');
    await expect(page.getByRole('heading', { name: /brief can’t be opened/ })).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('@claim:offline-reload uses the offline demo shell without disconnected API console errors', async ({ browser }) => {
    const context = await browser.newContext();
    const page = await context.newPage();
    const errors: string[] = [];
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto('/demo');
    await expect(page.getByRole('heading', { name: /See a private booking/ })).toBeVisible();
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
    expect(updateState.caches).toContain('private-intake-shell-v5');
    await context.setOffline(true);
    await page.reload();
    await expect(page.getByRole('heading', { name: /See a private booking/ })).toBeVisible();
    await expect(page.getByText('Nadia Patel', { exact: true })).toBeVisible();
    expect(errors.filter((message) => /ERR_INTERNET_DISCONNECTED/i.test(message))).toEqual([]);
    await context.setOffline(false);
    await context.close();
  });

  test('@claim:rate-limits limits identity checks by first forwarded IP and includes Retry-After on 429', async ({ page }) => {
    const headers = { 'x-forwarded-for': '203.0.113.90, 10.0.0.4' };
    for (let attempt = 1; attempt <= 20; attempt += 1) {
      const response = await page.request.get('/api/session', { headers });
      expect(response.status(), `attempt ${attempt}`).toBe(200);
    }
    const limited = await page.request.get('/api/session', { headers });
    expect(limited.status()).toBe(429);
    expect(Number(limited.headers()['retry-after'])).toBeGreaterThan(0);

    const separateClient = await page.request.get('/api/session', {
      headers: { 'x-forwarded-for': '203.0.113.91, 10.0.0.4' },
    });
    expect(separateClient.status()).toBe(200);
  });
});
