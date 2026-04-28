import { test, expect } from '@playwright/test';

test.describe('Notifications', () => {
  // E2E-14: Middle-click close tab
  // Steps: Middle-click a tab
  // Assertions: Tab closes
  test('E2E-14: middle-click on tab closes it', async ({ page }) => {
    await page.goto('/');
    // Wait for the app to be ready before sending keyboard shortcuts
    await expect(page.locator('header')).toBeVisible();

    // Open two tabs
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    const tabs = page.locator('[role="tablist"] [role="tab"]');
    await expect(tabs).toHaveCount(2);

    // Middle-click on the first tab (button: 'middle' = button 1)
    await tabs.first().click({ button: 'middle' });
    await page.waitForTimeout(300);

    // Verify the tab was closed — only 1 tab remains
    await expect(tabs).toHaveCount(1);
  });
});
