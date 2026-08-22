import { chromium, type FullConfig } from '@playwright/test';

// Matches the shape src/stores/appStore.ts persists under this key. Seeding it
// before first navigation skips the first-launch setup wizard, which every
// spec in this suite assumes is already done.
const APP_STORE_STATE = JSON.stringify({
  state: {
    activeProfileId: null,
    profiles: [],
    firstLaunchComplete: true,
    theme: 'dark',
  },
  version: 0,
});

/**
 * Runs once before the whole suite. Boots a real browser, seeds first-launch
 * state, then drives the actual "What's New" dismiss button instead of
 * hardcoding a dismissed-version string in a static storageState fixture —
 * a prior fix (ed1b005) did exactly that, and it went stale the moment
 * package.json's version next bumped (which happens on effectively every
 * release), silently reintroducing this same suite-wide blocker. Clicking
 * the real button captures whatever version is actually running, so this
 * never goes stale again.
 */
export default async function globalSetup(config: FullConfig) {
  const baseURL = config.projects[0]?.use?.baseURL ?? 'http://localhost:1420';
  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.addInitScript((value) => {
    window.localStorage.setItem('crossterm-app-store', value);
  }, APP_STORE_STATE);

  await page.goto(baseURL);

  const dontShowAgain = page.getByRole('button', { name: "Don't show again for this version" });
  try {
    await dontShowAgain.click({ timeout: 10_000 });
  } catch {
    // Panel didn't appear - nothing to dismiss.
  }

  await context.storageState({ path: 'e2e/storage-state.json' });
  await browser.close();
}
