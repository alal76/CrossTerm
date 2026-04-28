import { test, expect } from '@playwright/test';

test.describe('Tab & Pane Management', () => {
  // E2E-05: Sidebar collapse/expand
  // Steps: Click sidebar icon → collapsed → click again → expanded
  // Assertions: Width transitions correctly
  test('E2E-05: sidebar collapse and expand toggles width', async ({ page }) => {
    await page.goto('/');

    // The sidebar should be visible by default on wide viewports
    const sidebar = page.locator('nav');
    await expect(sidebar).toBeVisible();

    // The sidebar rail has icon buttons; the Sessions icon should be present
    const sessionsBtn = sidebar.locator('button[title="Sessions"]');
    await expect(sessionsBtn).toBeVisible();

    // Click Sessions to expand the sidebar panel (if collapsed, it opens; if open on same mode, it collapses)
    await sessionsBtn.click();
    await page.waitForTimeout(300);

    // Look for the "Sessions" heading in the content panel as indication it's expanded
    const contentHeading = sidebar.getByText('Sessions').first();

    // Check if sidebar is expanded (has content panel with heading)
    const isExpanded = await contentHeading.isVisible();

    if (isExpanded) {
      // Click the collapse button (ChevronLeft icon in the content panel header)
      const collapseBtn = sidebar.locator('button[title="Collapse Sidebar"]');
      await collapseBtn.click();
      await page.waitForTimeout(400);

      // After collapse, the content panel should be hidden (only the 48px rail remains)
      // The sidebar width should be narrow (w-12 = 48px)
      const sidebarBox = await sidebar.boundingBox();
      expect(sidebarBox?.width).toBeLessThanOrEqual(60);
    }

    // Click the Sessions icon again to re-expand
    await sessionsBtn.click();
    await page.waitForTimeout(400);

    // Sidebar should now be expanded with content visible
    await expect(sidebar.getByText('Sessions').first()).toBeVisible();
  });

  // E2E-06: Bottom panel
  // Steps: Ctrl+J → panel visible → switch modes → close
  // Assertions: Panel shows/hides correctly
  test('E2E-06: bottom panel toggle and mode switching', async ({ page }) => {
    await page.goto('/');

    // Bottom panel should be hidden initially
    const bottomPanel = page.locator('aside[aria-label="Bottom Panel"]');
    await expect(bottomPanel).not.toBeVisible();

    // Press Ctrl+J to toggle the bottom panel open
    await page.keyboard.press('Control+j');
    await page.waitForTimeout(300);

    // Bottom panel should now be visible
    await expect(bottomPanel).toBeVisible();

    // Verify panel mode tabs are visible (SFTP, Snippets, Audit Log, Search)
    await expect(bottomPanel.getByText('SFTP Browser')).toBeVisible();
    await expect(bottomPanel.getByText('Snippet Manager')).toBeVisible();
    await expect(bottomPanel.getByText('Audit Log')).toBeVisible();
    await expect(bottomPanel.getByText('Search Results')).toBeVisible();

    // Switch to Snippet Manager mode
    await bottomPanel.getByText('Snippet Manager').click();
    await page.waitForTimeout(200);

    // The Snippets empty state should be visible
    await expect(bottomPanel.getByText('No snippets yet')).toBeVisible();

    // Switch to Audit Log mode
    await bottomPanel.getByText('Audit Log').click();
    await page.waitForTimeout(200);

    // Close the panel with X button
    await bottomPanel.locator('button').last().click();
    await page.waitForTimeout(300);

    // Panel should be hidden
    await expect(bottomPanel).not.toBeVisible();
  });
});
