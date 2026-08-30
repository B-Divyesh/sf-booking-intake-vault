import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  use: {
    baseURL: 'http://127.0.0.1:8091',
    trace: 'retain-on-failure',
    extraHTTPHeaders: { 'x-test-oid': 'playwright-sociobot-entra-owner' },
  },
  webServer: {
    command: "sh -c 'db_dir=$(mktemp -d /tmp/piv-playwright-XXXXXX) && BUILD_SHA=playwright-test TEST_ENTRA_OID=playwright-sociobot-entra-owner DATABASE_URL=sqlite://$db_dir/vault.db?mode=rwc PORT=8091 cargo run'",
    url: 'http://127.0.0.1:8091/health',
    reuseExistingServer: false,
    timeout: 120_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
