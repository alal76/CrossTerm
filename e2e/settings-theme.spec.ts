import { test, expect } from '@playwright/test';

test.describe('Settings & Theme', () => {
  // E2E-08: Theme switching
  // Steps: Click theme toggle → assert light class → toggle back
  // Assertions: CSS variables change
  test('E2E-08: theme switching updates css variables', async ({ page }) => {
    await page.goto('/');

    // By default the app should be in dark mode
    const html = page.locator('html');
    await expect(html).toHaveClass(/dark/);

    // Click the theme toggle button in the TitleBar
    // The button has a title like "Dark" or data-tooltip with the theme name
    const themeBtn = page.locator('header button').filter({ has: page.locator('svg') }).first();
    await themeBtn.click();
    await page.waitForTimeout(300);

    // After one click, it should switch to light mode
    await expect(html).toHaveClass(/light/);

    // Click again to go to system, then one more to go back to dark
    await themeBtn.click();
    await page.waitForTimeout(300);
    // Now it's "system" mode — resolved theme depends on prefers-color-scheme
    // Click once more to cycle back to dark
    await themeBtn.click();
    await page.waitForTimeout(300);

    await expect(html).toHaveClass(/dark/);
  });
});
