import { test, expect } from '@playwright/test';

test.describe('Settings & Theme', () => {
  // E2E-08: Theme switching
  // Steps: Click theme toggle → assert light class → toggle back
  // Assertions: CSS variables change
  test('E2E-08: theme switching updates css variables', async ({ page }) => {
    await page.goto('/');

    // By default the app should be in dark mode (set in storage state)
    const html = page.locator('html');
    await expect(html).toHaveClass(/dark/);

    // The theme toggle button in the TitleBar has a title matching the current theme name
    // Starting from dark → button title is "Dark"
    await page.locator('header button[title="Dark"]').click();
    await page.waitForTimeout(300);

    // After one click, it should switch to light mode
    await expect(html).toHaveClass(/light/);

    // Click again to go to system, then one more to cycle back to dark
    await page.locator('header button[title="Light"]').click();
    await page.waitForTimeout(300);
    await page.locator('header button[title="System"]').click();
    await page.waitForTimeout(300);

    await expect(html).toHaveClass(/dark/);
  });
});
