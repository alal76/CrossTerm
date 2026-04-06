import { test, expect } from '@playwright/test';

test.describe('Port Forwarding', () => {
  // E2E-16: Port forwarding (placeholder for SSH port forward E2E)
  // Steps: Connect to SSH → configure port forward → verify tunnel
  // Assertions: Data flows through tunnel
  test.skip('E2E-16: ssh port forwarding creates tunnel', async ({ page }) => {
    // TODO: Requires SSH Docker container with port forwarding support
    await page.goto('/');

    // Connect to SSH session via Quick Connect
    await page.keyboard.press('Control+Shift+n');
    await page.waitForTimeout(300);

    // TODO: Connect to Docker SSH container
    // const quickConnect = page.locator('[role="dialog"]');
    // await quickConnect.locator('input').fill('ssh testuser@127.0.0.1:2222');
    // await page.keyboard.press('Enter');
    // await page.waitForTimeout(2000);

    // Open the Tunnels sidebar panel
    const sidebar = page.locator('nav');
    await sidebar.locator('button[title="Tunnels"]').click();
    await page.waitForTimeout(200);
    await expect(sidebar).toBeVisible();

    // TODO: Configure a local port forward (e.g., local:8080 → remote:80)
    // Verify the port forward appears in the active forwards list
    // Verify data can flow through the tunnel using fetch
    // await expect(sidebar.getByText('8080')).toBeVisible();
  });
});
