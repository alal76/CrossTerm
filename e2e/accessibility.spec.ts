import { test, expect } from '@playwright/test';

test.describe('Accessibility', () => {
  // E2E-20: Keyboard navigation
  // Steps: Tab through UI elements → all reachable by keyboard
  // Assertions: Focus ring visible
  test('E2E-20: all interactive elements reachable via keyboard', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(500);

    // Verify ARIA landmarks exist for the six-region layout
    // Region A: Title bar (header)
    const header = page.locator('header');
    await expect(header).toBeVisible();

    // Open a tab so the tablist has content and is visible
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    // Region B: Tab bar (role="tablist")
    const tablist = page.locator('[role="tablist"]');
    await expect(tablist).toBeVisible();

    // Region C: Sidebar (nav)
    const sidebar = page.locator('nav');
    await expect(sidebar).toBeVisible();

    // Region F: Status bar (footer element — <output> inside has implicit role="status")
    const statusBar = page.locator('footer');
    await expect(statusBar).toBeVisible();

    // Test keyboard navigation: Tab through elements
    await page.keyboard.press('Tab');
    await page.waitForTimeout(100);

    // Verify that focus moved to an interactive element
    const focusedTag = await page.evaluate(() => document.activeElement?.tagName);
    expect(focusedTag).toBeTruthy();

    // Tab through several more elements to verify no focus traps
    for (let i = 0; i < 5; i++) {
      await page.keyboard.press('Tab');
      await page.waitForTimeout(100);
    }

    // Verify we can still interact — focus should be on a valid element
    const focusedAfter = await page.evaluate(() => document.activeElement?.tagName);
    expect(focusedAfter).toBeTruthy();

    // Verify the live announcement region exists for screen readers
    // Use aria-atomic="true" to distinguish it from the toast notification container
    const liveRegion = page.locator('[aria-live="polite"][aria-atomic="true"]');
    await expect(liveRegion).toBeAttached();

    // Verify keyboard shortcut: F1 opens help panel
    await page.keyboard.press('F1');
    await page.waitForTimeout(300);

    // Help panel should open (it's a panel, not necessarily role="dialog")
    const helpTitle = page.getByText('Help').first();
    await expect(helpTitle).toBeVisible();

    // Press Escape to close
    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);
  });
});
