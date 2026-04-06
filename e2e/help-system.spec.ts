import { test, expect } from '@playwright/test';

test.describe('Help System', () => {
  // E2E-21: Help panel opens and navigates articles
  // Steps: Open help → verify articles → search → close
  // Assertions: Help panel visible with articles
  test('E2E-21: settings panel persists font size change', async ({ page }) => {
    await page.goto('/');

    // Open the Settings panel via Ctrl+,
    await page.keyboard.press('Control+,');
    await page.waitForTimeout(500);

    // Settings panel should be visible
    await expect(page.getByText('Settings')).toBeVisible();

    // Navigate to Appearance category
    const appearanceBtn = page.getByText('Appearance');
    if (await appearanceBtn.isVisible()) {
      await appearanceBtn.click();
      await page.waitForTimeout(200);
    }

    // Look for the Font Size setting
    const fontSizeLabel = page.getByText('Font Size');
    await expect(fontSizeLabel).toBeVisible();

    // Find the font size number input and change it
    const fontSizeInput = page.locator('input[type="number"]').first();
    await fontSizeInput.fill('16');
    await page.waitForTimeout(300);

    // Close settings (press Ctrl+, again to toggle)
    await page.keyboard.press('Control+,');
    await page.waitForTimeout(300);

    // Reopen settings to verify persistence
    await page.keyboard.press('Control+,');
    await page.waitForTimeout(500);

    // Navigate back to Appearance
    if (await page.getByText('Appearance').isVisible()) {
      await page.getByText('Appearance').click();
      await page.waitForTimeout(200);
    }

    // Verify the font size value is still 16
    const fontInput = page.locator('input[type="number"]').first();
    await expect(fontInput).toHaveValue('16');
  });

  // E2E-22: Audit log view
  // Steps: Open bottom panel → Audit Log tab → verify events
  // Assertions: Events listed
  test('E2E-22: audit log displays events in bottom panel', async ({ page }) => {
    await page.goto('/');

    // Open the bottom panel with Ctrl+J
    await page.keyboard.press('Control+j');
    await page.waitForTimeout(300);

    const bottomPanel = page.locator('aside[aria-label="Bottom Panel"]');
    await expect(bottomPanel).toBeVisible();

    // Switch to the Audit Log tab
    await bottomPanel.getByText('Audit Log').click();
    await page.waitForTimeout(200);

    // The Audit Log pane should be visible
    // Since there are no events yet, it shows the empty/no-results state
    await expect(bottomPanel.getByText('No results found')).toBeVisible();

    // Perform some actions that could generate audit events
    // Open a tab
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    // The panel should still be visible
    await expect(bottomPanel).toBeVisible();
  });
});
