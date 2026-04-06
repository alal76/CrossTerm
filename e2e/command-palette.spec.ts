import { test, expect } from '@playwright/test';

test.describe('Command Palette', () => {
  // E2E-10: Command palette
  // Steps: Cmd+Shift+P → type "toggle" → select "Toggle Sidebar"
  // Assertions: Sidebar toggles
  test('E2E-10: command palette executes toggle sidebar action', async ({ page }) => {
    await page.goto('/');

    // Ensure the sidebar is visible before toggling
    const sidebar = page.locator('nav');
    await expect(sidebar).toBeVisible();

    // Open the command palette with Ctrl+Shift+P
    await page.keyboard.press('Control+Shift+p');
    await page.waitForTimeout(300);

    // The command palette dialog should appear
    const paletteDialog = page.locator('[role="dialog"]');
    await expect(paletteDialog).toBeVisible();

    // Type "toggle" to filter commands
    const paletteInput = paletteDialog.locator('input');
    await paletteInput.fill('Toggle Sidebar');
    await page.waitForTimeout(200);

    // The "Toggle Sidebar" option should be visible in the filtered list
    await expect(paletteDialog.getByText('Toggle Sidebar')).toBeVisible();

    // Click on "Toggle Sidebar" to execute it
    await paletteDialog.getByText('Toggle Sidebar').click();
    await page.waitForTimeout(400);

    // The command palette should close
    await expect(paletteDialog).not.toBeVisible();

    // Verify the sidebar state changed (collapsed to only the 48px rail)
    const sidebarBox = await sidebar.boundingBox();
    expect(sidebarBox?.width).toBeLessThanOrEqual(60);
  });

  // E2E-11: Quick Connect
  // Steps: Cmd+Shift+N → type `ssh user@host` → enter
  // Assertions: Tab created with SSH type
  test.skip('E2E-11: quick connect creates ssh tab from uri', async ({ page }) => {
    // NOTE: Requires SSH server/Docker for actual connection
    await page.goto('/');

    // Open Quick Connect with Ctrl+Shift+N
    await page.keyboard.press('Control+Shift+n');
    await page.waitForTimeout(300);

    // Quick Connect dialog should appear
    const quickConnect = page.locator('[role="dialog"]');
    await expect(quickConnect).toBeVisible();

    // Type an SSH URI
    const input = quickConnect.locator('input');
    await input.fill('ssh root@192.168.1.100');

    // Press Enter to connect
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    // NOTE: Verify a new tab is created with SSH connection type
    // This requires the Tauri SSH backend to be running
    // const tabs = page.locator('[role="tablist"] [role="tab"]');
    // await expect(tabs.first()).toBeVisible();
  });
});
