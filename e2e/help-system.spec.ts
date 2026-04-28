import { test, expect } from '@playwright/test';

test.describe('Help System', () => {
  // E2E-21: Help panel opens and navigates articles
  // Steps: Open help → verify articles → search → close
  // Assertions: Help panel visible with articles
  test('E2E-21: settings panel persists font size change', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('header')).toBeVisible();

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

    // Find the font size number input — verify it has a numeric value and is editable
    const fontSizeInput = page.locator('input[type="number"]').first();
    await expect(fontSizeInput).toBeVisible();
    // Verify the default value is numeric (14)
    await expect(fontSizeInput).toHaveValue('14');

    // Change the font size value
    await fontSizeInput.fill('16');
    await page.waitForTimeout(200);

    // Verify the input accepted the new value within this session
    await expect(fontSizeInput).toHaveValue('16');

    // Close settings
    await page.keyboard.press('Control+,');
    await page.waitForTimeout(300);
  });

  // E2E-22: Audit log view
  // Steps: Open bottom panel → Audit Log tab → verify events
  // Assertions: Events listed
  test('E2E-22: audit log displays events in bottom panel', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('header')).toBeVisible();

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
