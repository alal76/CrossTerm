import { test, expect } from '@playwright/test';

test.describe('Responsive Layout', () => {
  // E2E-17: Window resize → sidebar collapse
  // Steps: Resize window < 900px
  // Assertions: Sidebar auto-collapses
  test('E2E-17: responsive layout collapses sidebar on narrow viewport', async ({ page }) => {
    // Start with a wide viewport
    await page.setViewportSize({ width: 1200, height: 800 });
    await page.goto('/');
    await page.waitForTimeout(500);

    // Sidebar should be visible and expanded on wide viewport
    const sidebar = page.locator('nav');
    await expect(sidebar).toBeVisible();

    // On a wide viewport, the sidebar content panel should be present
    const sidebarBox = await sidebar.boundingBox();
    const wideWidth = sidebarBox?.width ?? 0;

    // Resize to a narrow viewport (< 900px triggers "medium" breakpoint → auto-collapse)
    await page.setViewportSize({ width: 800, height: 600 });
    await page.waitForTimeout(500);

    // Sidebar should auto-collapse: either hidden entirely (compact) or just the rail
    const narrowSidebar = page.locator('nav');
    const narrowBox = await narrowSidebar.boundingBox();

    if (narrowBox) {
      // If still visible, it should be the rail only (≤60px)
      expect(narrowBox.width).toBeLessThan(wideWidth);
    }

    // Resize to very narrow (compact breakpoint < 640px) — sidebar hidden entirely
    await page.setViewportSize({ width: 500, height: 600 });
    await page.waitForTimeout(500);

    // On compact, the sidebar nav is not rendered; bottom nav appears instead
    // The bottom navigation bar should be visible for compact layout
    await expect(page.getByText('New Tab')).toBeVisible();

    // Resize back to wide
    await page.setViewportSize({ width: 1200, height: 800 });
    await page.waitForTimeout(500);
    await expect(page.locator('nav').first()).toBeVisible();
  });
});
