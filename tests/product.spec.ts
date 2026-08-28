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
    const violations = await new AxeBuilder({ page }).analyze();
    expect(violations.violations.filter((item) => ['serious', 'critical'].includes(item.impact || ''))).toEqual([]);
    expect(consoleErrors).toEqual([]);
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
  });

  test('fits the client form at 390px without horizontal overflow', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/book');
    await expect(page.getByRole('heading', { name: /Tell the team/ })).toBeVisible();
    const dimensions = await page.evaluate(() => ({ scroll: document.documentElement.scrollWidth, client: document.documentElement.clientWidth }));
    expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client);
  });
});
