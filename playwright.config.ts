import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  retries: 1,
  use: {
    baseURL: 'http://localhost:1420', // Tauri dev server
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    storageState: 'e2e/storage-state.json',
  },
  webServer: {
    command: 'npm run dev',
    port: 1420,
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
