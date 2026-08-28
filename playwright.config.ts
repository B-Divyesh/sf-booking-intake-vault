import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  use: {
    baseURL: 'http://127.0.0.1:8091',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: "sh -c 'rm -f /tmp/piv-playwright.db && BUILD_SHA=playwright-test DATABASE_URL=sqlite:///tmp/piv-playwright.db?mode=rwc PORT=8091 cargo run'",
    url: 'http://127.0.0.1:8091/health',
    reuseExistingServer: false,
    timeout: 120_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
