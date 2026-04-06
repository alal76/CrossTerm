import { test, expect } from '@playwright/test';

test.describe('Terminal', () => {
  // E2E-15: Window resize → sidebar collapse (terminal-specific)
  // Steps: Resize window < 900px
  // Assertions: Sidebar auto-collapses
  test.skip('E2E-15: window resize below 900px auto-collapses sidebar', async ({ page }) => {
    // NOTE: Requires Tauri window management to test real window resize
    await page.goto('/');

    // Set wide viewport
    await page.setViewportSize({ width: 1200, height: 800 });
    await page.waitForTimeout(500);

    // Sidebar should be expanded
    const sidebar = page.locator('nav');
    await expect(sidebar).toBeVisible();

    // Open a terminal tab
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    // Resize below 900px
    await page.setViewportSize({ width: 800, height: 600 });
    await page.waitForTimeout(500);

    // Sidebar should auto-collapse (width ≤ 60px rail only)
    const sidebarBox = await sidebar.boundingBox();
    if (sidebarBox) {
      expect(sidebarBox.width).toBeLessThanOrEqual(60);
    }

    // Terminal tab should still be visible and functional
    const tabs = page.locator('[role="tablist"] [role="tab"]');
    await expect(tabs).toHaveCount(1);
  });
});
