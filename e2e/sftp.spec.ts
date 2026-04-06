import { test, expect } from '@playwright/test';

test.describe('SFTP Browser', () => {
  // E2E-07: SFTP browser
  // Steps: Connect to SSH session → open SFTP panel → browse
  // Assertions: Files/directories listed
  test.skip('E2E-07: sftp browser lists remote files after ssh connect', async ({ page }) => {
    // NOTE: Requires SSH Docker container with SFTP support
    await page.goto('/');

    // Open a Quick Connect SSH session
    await page.keyboard.press('Control+Shift+n');
    await page.waitForTimeout(300);

    // NOTE: Connect to SSH session (Docker test container at 127.0.0.1:2222)
    // const quickConnect = page.locator('[role="dialog"]');
    // const input = quickConnect.locator('input');
    // await input.fill('ssh testuser@127.0.0.1:2222');
    // await page.keyboard.press('Enter');
    // await page.waitForTimeout(2000);

    // Open the bottom panel with SFTP mode
    await page.keyboard.press('Control+j');
    await page.waitForTimeout(300);

    const bottomPanel = page.locator('aside[aria-label="Bottom Panel"]');
    await expect(bottomPanel).toBeVisible();

    // Switch to SFTP Browser tab
    await bottomPanel.getByText('SFTP Browser').click();
    await page.waitForTimeout(200);

    // NOTE: Once SSH is connected, the SFTP browser should show remote files
    // Verify files and directories are listed
    // await expect(bottomPanel.getByText('Name')).toBeVisible();
    // await expect(bottomPanel.getByText('Size')).toBeVisible();
  });
});
