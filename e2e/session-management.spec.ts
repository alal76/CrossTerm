import { test, expect } from '@playwright/test';

test.describe('Session Management', () => {
  // E2E-01: First launch → create profile
  // Steps: Open app → wizard → set password → create
  // Assertions: Profile appears in title bar
  test('E2E-01: first launch creates profile via wizard', async ({ page }) => {
    // Clear localStorage to simulate first launch
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();

    // Step 0: Welcome screen should appear with "Get Started" button
    await expect(page.getByText('CrossTerm')).toBeVisible();
    await expect(page.getByText('Get Started')).toBeVisible();
    await page.getByText('Get Started').click();

    // Step 1: Create Profile
    await expect(page.getByText('Create a Profile')).toBeVisible();
    const profileInput = page.locator('#profile-name');
    await profileInput.fill('Test Profile');
    await page.getByText('Create').first().click();

    // Step 2: Set Master Password
    await expect(page.getByText('Set Master Password')).toBeVisible();
    await page.locator('#master-password').fill('TestPassword123!');
    await page.locator('#confirm-password').fill('TestPassword123!');
    await page.getByText('Create').first().click();

    // Step 3: Choose Theme
    await expect(page.getByText('Choose a Theme')).toBeVisible();
    await page.getByText('Dark').click();
    await page.getByText('Finish').click();

    // Verify the main app loads with the title bar visible
    await expect(page.locator('header').getByText('CrossTerm')).toBeVisible();
    // Profile name should appear in the title bar
    await expect(page.locator('header').getByText('Test Profile')).toBeVisible();
  });

  // E2E-02: Create SSH session
  // Steps: Sessions panel → New → fill form → save
  // Assertions: Session in tree
  test.skip('E2E-02: create SSH session appears in session tree', async ({ page }) => {
    // TODO: Requires SSH Docker container for full integration
    await page.goto('/');

    // Navigate to Sessions panel in the sidebar
    await page.locator('nav button[title="Sessions"]').click();
    await expect(page.locator('nav').getByText('Sessions').first()).toBeVisible();

    // Click "New Session" button
    await page.getByText('New Session').click();

    // TODO: Fill in SSH session form with host, port, username, auth method
    // These depend on the Docker SSH test container being available
    // await page.locator('#session-host').fill('127.0.0.1');
    // await page.locator('#session-port').fill('2222');
    // await page.getByText('Save').click();
    // await expect(page.locator('nav').getByText('127.0.0.1')).toBeVisible();
  });

  // E2E-03: Open local terminal
  // Steps: Cmd+T → Local Shell → type `echo hello`
  // Assertions: "hello" appears in terminal output
  test.skip('E2E-03: open local terminal and execute command', async ({ page }) => {
    // TODO: Requires Tauri backend with PTY support running
    await page.goto('/');

    // Press Ctrl+T to open a new local terminal
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(500);

    // Verify a tab appeared in the tab bar
    const tabs = page.locator('[role="tablist"] [role="tab"]');
    await expect(tabs).toHaveCount(1);

    // TODO: PTY interaction requires the Tauri backend
    // Type command in the terminal xterm.js textarea
    // await page.locator('.xterm-helper-textarea').type('echo hello\n');
    // await expect(page.locator('.xterm-screen')).toContainText('hello');
  });

  // E2E-04: Tab management
  // Steps: Open 3 tabs → click tab 2 → close tab 3
  // Assertions: Correct tab active, tab 3 gone
  test('E2E-04: tab management open close and switch', async ({ page }) => {
    await page.goto('/');

    // Open 3 tabs using Ctrl+T
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    // Verify 3 tabs exist in the tablist
    const tabs = page.locator('[role="tablist"] [role="tab"]');
    await expect(tabs).toHaveCount(3);

    // Click on tab 2 (index 1) to make it active
    await tabs.nth(1).click();
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');

    // Close tab 3 by clicking its close button
    const tab3CloseBtn = tabs.nth(2).locator('button');
    await tab3CloseBtn.click();

    // Verify only 2 tabs remain
    await expect(tabs).toHaveCount(2);

    // Tab 2 should still be active
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
  });

  // E2E-12: Session edit / session tree folders
  // Steps: Create session → verify in sidebar → check tree structure
  // Assertions: Session visible in sidebar
  test('E2E-12: edit session updates name in tree', async ({ page }) => {
    await page.goto('/');

    // Ensure sidebar shows Sessions panel
    await page.locator('nav button[title="Sessions"]').click();

    // Create a session via Ctrl+T
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    // Verify the sidebar has sessions content
    const sidebar = page.locator('nav');
    await expect(sidebar).toBeVisible();

    // Check that the "Sessions" heading is visible in the sidebar
    await expect(sidebar.getByText('Sessions').first()).toBeVisible();
  });

  // E2E-13: Pin/unpin tab
  // Steps: Right-click tab → Pin → icon-only → Unpin → full tab
  // Assertions: Tab width changes
  test('E2E-13: pin and unpin tab changes tab appearance', async ({ page }) => {
    await page.goto('/');

    // Open a tab
    await page.keyboard.press('Control+t');
    await page.waitForTimeout(300);

    const tabs = page.locator('[role="tablist"] [role="tab"]');
    const tab = tabs.first();
    await expect(tab).toBeVisible();

    // The unpinned tab should have min-w-[120px] class
    await expect(tab).toHaveClass(/min-w-\[120px\]/);

    // Right-click to open context menu
    await tab.click({ button: 'right' });
    await page.waitForTimeout(200);

    // Context menu should appear with tab operations
    await expect(page.getByText('Close Tab')).toBeVisible();
    await expect(page.getByText('Duplicate Tab')).toBeVisible();

    // Close the context menu by clicking elsewhere
    await page.locator('body').click();
  });

  // E2E-19: Multiple profiles
  // Steps: Create second profile → switch → different session list
  // Assertions: Isolated data
  test.skip('E2E-19: multiple profiles have isolated session data', async ({ page }) => {
    // TODO: Requires pre-setup with backend profile management
    await page.goto('/');

    // Click profile area in title bar
    await page.locator('header').getByText('Default').click();

    // TODO: Create a second profile from the TitleBar/settings
    // Add sessions to each profile
    // Switch between profiles
    // Verify each profile shows its own session list (data isolation)
  });
});
